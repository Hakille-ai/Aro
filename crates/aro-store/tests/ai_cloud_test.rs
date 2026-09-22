//! Sovereign-AI proof for server-side cloud generation (default-DENY).
//!
//! What this demonstrates:
//! - a fresh organization has cloud generation DISABLED and no keys;
//! - consent is explicit (enabled + provider list + residency + admin actor);
//! - provider keys round-trip encrypted and are only readable through the
//!   dedicated generation-path method, never through status listings;
//! - status listings and consent rows contain NO key material;
//! - revocation is immediate; tenants are isolated from each other;
//! - malformed ids, empty keys and short secrets keys are rejected.

use aro_store::{AroStore, NewUserWithOrg};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const TEST_SECRETS_KEY: &str = "test-secrets-key-0123456789abcdef-test";

async fn connect() -> Result<Option<AroStore>, Box<dyn std::error::Error>> {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping AI cloud integration test: DATABASE_URL is not set");
        return Ok(None);
    };
    Ok(Some(AroStore::connect(&database_url, 5).await?))
}

async fn principal(
    store: &AroStore,
    label: &str,
) -> Result<aro_store::AuthPrincipal, Box<dyn std::error::Error>> {
    let unique = Uuid::new_v4();
    Ok(store
        .create_user_with_org(NewUserWithOrg {
            email: format!("{label}-{unique}@aro.local"),
            name: format!("{label} User"),
            role_title: None,
            avatar_color: None,
            password_hash: "argon2-test-hash".to_string(),
            organization_name: format!("{label} Org {unique}"),
            organization_domain: None,
            organization_description: None,
        })
        .await?)
}

#[tokio::test]
async fn fresh_org_denies_server_cloud_by_default() -> TestResult {
    let Some(store) = connect().await? else {
        return Ok(());
    };
    let owner = principal(&store, "aicloud-fresh").await?;
    let org = owner.active_organization.id;

    let consent = store.get_org_ai_cloud_consent(org).await?;
    assert!(!consent.enabled);
    assert!(consent.provider_ids.is_empty());
    assert!(store.list_org_provider_key_status(org).await?.is_empty());
    assert!(
        store
            .get_org_provider_key(org, "openai", TEST_SECRETS_KEY)
            .await?
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn consent_and_key_roundtrip_without_leaks() -> TestResult {
    let Some(store) = connect().await? else {
        return Ok(());
    };
    let owner = principal(&store, "aicloud-optin").await?;
    let org = owner.active_organization.id;
    let secret = "sk-test-server-cloud-key-001";

    let consent = store
        .set_org_ai_cloud_consent(org, true, &["openai".to_string()], Some("EU"), owner.user.id)
        .await?;
    assert!(consent.enabled);
    assert_eq!(consent.provider_ids, vec!["openai".to_string()]);
    assert_eq!(consent.data_residency.as_deref(), Some("EU"));
    assert_eq!(consent.accepted_by, Some(owner.user.id));
    assert!(consent.accepted_at.is_some());

    store
        .set_org_provider_key(org, "openai", secret, TEST_SECRETS_KEY)
        .await?;
    assert_eq!(
        store
            .get_org_provider_key(org, "openai", TEST_SECRETS_KEY)
            .await?
            .as_deref(),
        Some(secret)
    );

    // Status listings and consent rows: presence flags only, zero key material.
    let statuses = store.list_org_provider_key_status(org).await?;
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].provider_id, "openai");
    assert!(statuses[0].configured);
    for blob in [
        serde_json::to_string(&statuses)?,
        serde_json::to_string(&consent)?,
    ] {
        assert!(!blob.contains(secret), "key material leaked: {blob}");
        assert!(!blob.to_lowercase().contains("apikey") || blob.contains("providerId"));
    }

    // Revocation is immediate.
    assert!(store.delete_org_provider_key(org, "openai").await?);
    assert!(
        store
            .get_org_provider_key(org, "openai", TEST_SECRETS_KEY)
            .await?
            .is_none()
    );
    assert!(!store.delete_org_provider_key(org, "openai").await?);

    // Disabling consent keeps keys stored but unusable by policy (callers
    // check `enabled` first); keys stay encrypted at rest either way.
    store
        .set_org_ai_cloud_consent(org, false, &[], None, owner.user.id)
        .await?;
    assert!(!store.get_org_ai_cloud_consent(org).await?.enabled);
    Ok(())
}

#[tokio::test]
async fn tenants_are_isolated_and_inputs_validated() -> TestResult {
    let Some(store) = connect().await? else {
        return Ok(());
    };
    let alpha = principal(&store, "aicloud-alpha").await?;
    let beta = principal(&store, "aicloud-beta").await?;

    store
        .set_org_ai_cloud_consent(
            alpha.active_organization.id,
            true,
            &["mistral".to_string()],
            None,
            alpha.user.id,
        )
        .await?;
    store
        .set_org_provider_key(
            alpha.active_organization.id,
            "mistral",
            "sk-alpha-secret",
            TEST_SECRETS_KEY,
        )
        .await?;

    // Beta sees nothing of alpha's cloud state.
    assert!(!store
        .get_org_ai_cloud_consent(beta.active_organization.id)
        .await?
        .enabled);
    assert!(store
        .list_org_provider_key_status(beta.active_organization.id)
        .await?
        .is_empty());
    assert!(store
        .get_org_provider_key(beta.active_organization.id, "mistral", TEST_SECRETS_KEY)
        .await?
        .is_none());

    // Validation: bad ids, empty keys, short secrets keys.
    assert!(store
        .set_org_provider_key(alpha.active_organization.id, "UPPER", "k", TEST_SECRETS_KEY)
        .await
        .is_err());
    assert!(store
        .set_org_provider_key(alpha.active_organization.id, "../x", "k", TEST_SECRETS_KEY)
        .await
        .is_err());
    assert!(store
        .set_org_provider_key(alpha.active_organization.id, "mistral", "   ", TEST_SECRETS_KEY)
        .await
        .is_err());
    assert!(store
        .set_org_provider_key(alpha.active_organization.id, "mistral", "k", "short")
        .await
        .is_err());
    assert!(store
        .get_org_provider_key(alpha.active_organization.id, "mistral", "short")
        .await
        .is_err());
    assert!(store
        .set_org_ai_cloud_consent(
            alpha.active_organization.id,
            true,
            &["bad id!".to_string()],
            None,
            alpha.user.id
        )
        .await
        .is_err());
    Ok(())
}
