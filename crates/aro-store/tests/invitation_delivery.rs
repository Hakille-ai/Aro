use std::error::Error;

use aro_core::MembershipRole;
use aro_store::{AroStore, NewOrganizationMember, NewUserWithOrg};
use chrono::{Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

const TEST_SECRETS_KEY: &str = "aro-invitation-delivery-test-key-at-least-32-bytes";
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

async fn issue_invitation(
    store: &AroStore,
    actor_id: Uuid,
    organization_id: Uuid,
    label: &str,
) -> TestResult<(Uuid, String)> {
    let bearer = token();
    let issued = store
        .issue_organization_invitation(
            actor_id,
            organization_id,
            NewOrganizationMember {
                email: format!("{label}-{}@aro.local", Uuid::new_v4()),
                name: format!("{label} Invitee"),
                role: MembershipRole::Member,
            },
            bearer.clone(),
            Utc::now() + Duration::hours(1),
            TEST_SECRETS_KEY,
        )
        .await?;
    // Put this test job ahead of unrelated pending fixtures in a shared integration database.
    // Production ordering remains unchanged; only the freshly-created row is touched.
    sqlx::query(
        "UPDATE organization_invitations SET delivery_available_at = '1970-01-01 00:00:00+00' WHERE id = $1",
    )
    .bind(issued.id)
    .execute(store.pool())
    .await?;
    Ok((issued.id, bearer))
}

#[tokio::test]
async fn invitation_delivery_is_lease_fenced_across_retry_acceptance_and_revocation() -> TestResult
{
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping invitation delivery integration test: DATABASE_URL is not set");
        return Ok(());
    };
    let store = AroStore::connect(&database_url, 10).await?;
    let worker_role_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker')")
            .fetch_one(store.pool())
            .await?;
    if worker_role_exists {
        let worker_privileges: (bool, bool, bool, bool, bool, bool) = sqlx::query_as(
            r#"
            SELECT
              has_column_privilege('aro_worker', 'organization_invitations', 'delivery_status', 'SELECT'),
              has_column_privilege('aro_worker', 'organization_invitations', 'delivery_status', 'UPDATE'),
              has_column_privilege('aro_worker', 'organizations', 'name', 'SELECT'),
              has_column_privilege('aro_worker', 'organization_invitations', 'token_hash', 'SELECT'),
              has_column_privilege('aro_worker', 'organization_invitations', 'accepted_at', 'UPDATE'),
              has_column_privilege('aro_worker', 'organizations', 'description', 'SELECT')
            "#,
        )
        .fetch_one(store.pool())
        .await?;
        assert_eq!(
            worker_privileges,
            (true, true, true, false, false, false),
            "the delivery worker must have only its reviewed column-level privileges"
        );
    }
    let unique = Uuid::new_v4();
    let owner = store
        .create_user_with_org(NewUserWithOrg {
            email: format!("invitation-delivery-owner-{unique}@aro.local"),
            name: "Invitation Delivery Owner".to_string(),
            role_title: None,
            avatar_color: None,
            password_hash: "argon2-test-hash".to_string(),
            organization_name: format!("Invitation Delivery Org {unique}"),
            organization_domain: None,
            organization_description: None,
        })
        .await?;
    let actor_id = owner.user.id;
    let organization_id = owner.active_organization.id;

    // Only one worker can own a live lease. Expiry permits takeover, while the old token remains
    // fenced out even after another worker has claimed the same invitation.
    let (retry_id, retry_bearer) =
        issue_invitation(&store, actor_id, organization_id, "delivery-retry").await?;
    let first_lease = Uuid::new_v4();
    let first = store
        .claim_invitation_deliveries(1, first_lease, 60, 4, TEST_SECRETS_KEY)
        .await?;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].id, retry_id);
    assert_eq!(first[0].token, retry_bearer);
    assert_eq!(first[0].attempts, 1);
    let live_lease: bool = sqlx::query_scalar(
        r#"
        SELECT delivery_status = 'processing'
           AND delivery_lease_token = $2
           AND delivery_lease_until > now()
        FROM organization_invitations
        WHERE id = $1
        "#,
    )
    .bind(retry_id)
    .bind(first_lease)
    .fetch_one(store.pool())
    .await?;
    assert!(live_lease);
    assert!(
        !store
            .acknowledge_invitation_delivery(retry_id, Uuid::new_v4())
            .await?
    );
    assert!(
        !store
            .release_invitation_delivery_for_retry(retry_id, Uuid::new_v4(), 4, 1, "smtp_timeout",)
            .await?
    );

    sqlx::query(
        "UPDATE organization_invitations SET delivery_lease_until = now() - interval '1 second' WHERE id = $1",
    )
    .bind(retry_id)
    .execute(store.pool())
    .await?;
    assert!(
        !store
            .acknowledge_invitation_delivery(retry_id, first_lease)
            .await?
    );
    let successor_lease = Uuid::new_v4();
    let successor = store
        .claim_invitation_deliveries(1, successor_lease, 60, 4, TEST_SECRETS_KEY)
        .await?;
    assert_eq!(successor.len(), 1);
    assert_eq!(successor[0].id, retry_id);
    assert_eq!(successor[0].attempts, 2);
    assert!(
        !store
            .acknowledge_invitation_delivery(retry_id, first_lease)
            .await?
    );
    assert!(
        store
            .release_invitation_delivery_for_retry(retry_id, successor_lease, 4, 1, "smtp_timeout",)
            .await?
    );
    let retry_state = sqlx::query(
        r#"
        SELECT delivery_status, delivery_attempts, delivery_lease_token, delivery_last_error,
               delivery_token_encrypted IS NOT NULL AS has_delivery_token
        FROM organization_invitations
        WHERE id = $1
        "#,
    )
    .bind(retry_id)
    .fetch_one(store.pool())
    .await?;
    assert_eq!(retry_state.get::<String, _>("delivery_status"), "pending");
    assert_eq!(retry_state.get::<i32, _>("delivery_attempts"), 2);
    assert_eq!(
        retry_state.get::<Option<Uuid>, _>("delivery_lease_token"),
        None
    );
    assert_eq!(
        retry_state.get::<Option<String>, _>("delivery_last_error"),
        Some("smtp_timeout".to_string())
    );
    assert!(retry_state.get::<bool, _>("has_delivery_token"));

    sqlx::query(
        "UPDATE organization_invitations SET delivery_available_at = '1970-01-01 00:00:00+00' WHERE id = $1",
    )
    .bind(retry_id)
    .execute(store.pool())
    .await?;
    let final_lease = Uuid::new_v4();
    let final_claim = store
        .claim_invitation_deliveries(1, final_lease, 60, 4, TEST_SECRETS_KEY)
        .await?;
    assert_eq!(final_claim.len(), 1);
    assert_eq!(final_claim[0].id, retry_id);
    assert!(
        store
            .acknowledge_invitation_delivery(retry_id, final_lease)
            .await?
    );
    assert!(
        !store
            .acknowledge_invitation_delivery(retry_id, final_lease)
            .await?
    );
    let delivered = sqlx::query(
        r#"
        SELECT delivery_status, delivered_at IS NOT NULL AS was_delivered,
               delivery_token_encrypted IS NULL AS token_destroyed,
               delivery_lease_token IS NULL AS lease_destroyed
        FROM organization_invitations
        WHERE id = $1
        "#,
    )
    .bind(retry_id)
    .fetch_one(store.pool())
    .await?;
    assert_eq!(delivered.get::<String, _>("delivery_status"), "delivered");
    assert!(delivered.get::<bool, _>("was_delivered"));
    assert!(delivered.get::<bool, _>("token_destroyed"));
    assert!(delivered.get::<bool, _>("lease_destroyed"));

    // Destroying the worker's encrypted copy after relay acknowledgement does not destroy the
    // one-way token hash needed by the recipient to accept the link.
    store
        .accept_new_organization_invitation(&retry_bearer, "argon2-invitee-hash".to_string())
        .await?;
    let accepted: bool = sqlx::query_scalar(
        "SELECT accepted_at IS NOT NULL FROM organization_invitations WHERE id = $1",
    )
    .bind(retry_id)
    .fetch_one(store.pool())
    .await?;
    assert!(accepted);

    // Acceptance racing relay acknowledgement is monotonic: acceptance wins permanently and a
    // late worker cannot recreate its lease or encrypted bearer credential.
    let (accept_id, accept_bearer) =
        issue_invitation(&store, actor_id, organization_id, "delivery-accept-race").await?;
    let accept_lease = Uuid::new_v4();
    let accept_claim = store
        .claim_invitation_deliveries(1, accept_lease, 60, 4, TEST_SECRETS_KEY)
        .await?;
    assert_eq!(accept_claim.len(), 1);
    assert_eq!(accept_claim[0].id, accept_id);
    let (accept_result, acknowledge_result) = tokio::join!(
        store.accept_new_organization_invitation(
            &accept_bearer,
            "argon2-accept-race-hash".to_string()
        ),
        store.acknowledge_invitation_delivery(accept_id, accept_lease),
    );
    accept_result?;
    let _acknowledged_before_acceptance = acknowledge_result?;
    let accepted_state = sqlx::query(
        r#"
        SELECT accepted_at IS NOT NULL AS accepted, delivery_status,
               delivery_token_encrypted IS NULL AS token_destroyed,
               delivery_lease_token IS NULL AS lease_destroyed
        FROM organization_invitations
        WHERE id = $1
        "#,
    )
    .bind(accept_id)
    .fetch_one(store.pool())
    .await?;
    assert!(accepted_state.get::<bool, _>("accepted"));
    assert_eq!(
        accepted_state.get::<String, _>("delivery_status"),
        "delivered"
    );
    assert!(accepted_state.get::<bool, _>("token_destroyed"));
    assert!(accepted_state.get::<bool, _>("lease_destroyed"));
    assert!(
        !store
            .release_invitation_delivery_for_retry(accept_id, accept_lease, 4, 1, "smtp_timeout",)
            .await?
    );

    // Administrative revocation has the same monotonic fencing guarantee.
    let (revoke_id, _) =
        issue_invitation(&store, actor_id, organization_id, "delivery-revoke-race").await?;
    let revoke_lease = Uuid::new_v4();
    let revoke_claim = store
        .claim_invitation_deliveries(1, revoke_lease, 60, 4, TEST_SECRETS_KEY)
        .await?;
    assert_eq!(revoke_claim.len(), 1);
    assert_eq!(revoke_claim[0].id, revoke_id);
    let (revoke_result, acknowledge_result) = tokio::join!(
        store.revoke_organization_invitation(actor_id, organization_id, revoke_id),
        store.acknowledge_invitation_delivery(revoke_id, revoke_lease),
    );
    revoke_result?;
    let _acknowledged_before_revocation = acknowledge_result?;
    let revoked_state = sqlx::query(
        r#"
        SELECT revoked_at IS NOT NULL AS revoked, delivery_status,
               delivery_token_encrypted IS NULL AS token_destroyed,
               delivery_lease_token IS NULL AS lease_destroyed,
               delivery_last_error
        FROM organization_invitations
        WHERE id = $1
        "#,
    )
    .bind(revoke_id)
    .fetch_one(store.pool())
    .await?;
    assert!(revoked_state.get::<bool, _>("revoked"));
    assert_eq!(revoked_state.get::<String, _>("delivery_status"), "failed");
    assert!(revoked_state.get::<bool, _>("token_destroyed"));
    assert!(revoked_state.get::<bool, _>("lease_destroyed"));
    assert_eq!(
        revoked_state.get::<Option<String>, _>("delivery_last_error"),
        Some("invitation_revoked".to_string())
    );

    // A permanent failure exhausts immediately and destroys only the worker-decryptable copy.
    let (failed_id, _) =
        issue_invitation(&store, actor_id, organization_id, "delivery-permanent").await?;
    let failed_lease = Uuid::new_v4();
    let failed_claim = store
        .claim_invitation_deliveries(1, failed_lease, 60, 4, TEST_SECRETS_KEY)
        .await?;
    assert_eq!(failed_claim.len(), 1);
    assert_eq!(failed_claim[0].id, failed_id);
    assert!(
        store
            .release_invitation_delivery_for_retry(
                failed_id,
                failed_lease,
                1,
                30,
                "invalid_recipient",
            )
            .await?
    );
    let failed_state = sqlx::query(
        r#"
        SELECT delivery_status, delivery_token_encrypted IS NULL AS token_destroyed,
               delivery_last_error
        FROM organization_invitations
        WHERE id = $1
        "#,
    )
    .bind(failed_id)
    .fetch_one(store.pool())
    .await?;
    assert_eq!(failed_state.get::<String, _>("delivery_status"), "failed");
    assert!(failed_state.get::<bool, _>("token_destroyed"));
    assert_eq!(
        failed_state.get::<Option<String>, _>("delivery_last_error"),
        Some("invalid_recipient".to_string())
    );

    // Expired invitations and exhausted jobs are terminally scrubbed. A live lease is never
    // stolen merely because its claim consumed the last allowed attempt.
    // Drain any existing expired invitations from previous runs in the shared database.
    let _ = store.fail_expired_invitation_deliveries(4).await?;

    let (expired_id, _) =
        issue_invitation(&store, actor_id, organization_id, "delivery-expired").await?;
    sqlx::query(
        "UPDATE organization_invitations SET expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(expired_id)
    .execute(store.pool())
    .await?;
    assert_eq!(store.fail_expired_invitation_deliveries(4).await?, 1);
    let expired_state: (String, bool, Option<String>) = sqlx::query_as(
        r#"
        SELECT delivery_status, delivery_token_encrypted IS NULL, delivery_last_error
        FROM organization_invitations
        WHERE id = $1
        "#,
    )
    .bind(expired_id)
    .fetch_one(store.pool())
    .await?;
    assert_eq!(
        expired_state,
        (
            "failed".to_string(),
            true,
            Some("invitation_expired".to_string())
        )
    );

    let (exhausted_id, _) =
        issue_invitation(&store, actor_id, organization_id, "delivery-exhausted").await?;
    let exhausted_lease = Uuid::new_v4();
    let exhausted_claim = store
        .claim_invitation_deliveries(1, exhausted_lease, 60, 1, TEST_SECRETS_KEY)
        .await?;
    assert_eq!(exhausted_claim.len(), 1);
    assert_eq!(exhausted_claim[0].id, exhausted_id);
    assert_eq!(store.fail_expired_invitation_deliveries(1).await?, 0);
    let still_processing: bool = sqlx::query_scalar(
        "SELECT delivery_status = 'processing' FROM organization_invitations WHERE id = $1",
    )
    .bind(exhausted_id)
    .fetch_one(store.pool())
    .await?;
    assert!(still_processing);
    sqlx::query(
        "UPDATE organization_invitations SET delivery_lease_until = now() - interval '1 second' WHERE id = $1",
    )
    .bind(exhausted_id)
    .execute(store.pool())
    .await?;
    assert_eq!(store.fail_expired_invitation_deliveries(1).await?, 1);
    let exhausted_state: (String, bool, Option<String>) = sqlx::query_as(
        r#"
        SELECT delivery_status, delivery_token_encrypted IS NULL, delivery_last_error
        FROM organization_invitations
        WHERE id = $1
        "#,
    )
    .bind(exhausted_id)
    .fetch_one(store.pool())
    .await?;
    assert_eq!(
        exhausted_state,
        (
            "failed".to_string(),
            true,
            Some("delivery_attempts_exhausted".to_string())
        )
    );
    assert!(
        !store
            .acknowledge_invitation_delivery(exhausted_id, exhausted_lease)
            .await?
    );

    Ok(())
}
