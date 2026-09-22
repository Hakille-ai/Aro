//! Tenant-isolation proof for private data (conversations/messages/memories).
//!
//! What this demonstrates (app layer, enforced today):
//! - a user from another organization cannot read, list, write or resolve
//!   another tenant's conversations, messages or memories;
//! - a member of the SAME organization cannot see another user's
//!   owner-scoped memories.
//!
//! What this deliberately does NOT enable: Postgres Row-Level Security on
//! these tables. `202607020011_private_data_rls_policies.sql` ships the
//! policies but defers `ENABLE ROW LEVEL SECURITY` to an explicit phase-2
//! gate migration (direct `pool` reads in this crate do not all set the
//! `aro.*` session vars yet). The `rls_phase_gate` test below pins that
//! state: flipping it on must be a conscious migration + full-suite run,
//! not a silent side effect.

use aro_core::{AssistantMode, ChatMessage, MessageRole};
use aro_store::{AroStore, NewUserWithOrg, PersistedCollection, TenantContext};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn connect() -> Result<Option<AroStore>, Box<dyn std::error::Error>> {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping RLS integration test: DATABASE_URL is not set");
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

fn tenant(user_id: Uuid, org_id: Uuid) -> TenantContext {
    TenantContext::new(user_id, org_id).expect("non-nil test tenant context")
}

#[tokio::test]
async fn cross_tenant_private_data_is_inaccessible() -> TestResult {
    let Some(store) = connect().await? else {
        return Ok(());
    };
    let owner = principal(&store, "rls-owner").await?;
    let outsider = principal(&store, "rls-outsider").await?;
    let owner_ctx = tenant(owner.user.id, owner.active_organization.id);
    let outsider_ctx = tenant(outsider.user.id, outsider.active_organization.id);

    // Owner seeds private data.
    let conversation = store
        .create_conversation(owner_ctx, "RLS proof".to_string(), AssistantMode::Chat)
        .await?;
    let message = ChatMessage::new(conversation.id, MessageRole::User, "rls secret");
    store.add_message(owner_ctx, &message).await?;
    let memory = store
        .create_collection_item(
            owner.user.id,
            owner.active_organization.id,
            PersistedCollection::Memories,
            serde_json::json!({ "content": "rls owner fact" }),
            None,
        )
        .await?;
    let memory_id: Uuid = serde_json::from_value(
        memory
            .get("id")
            .cloned()
            .ok_or("memory response has no id")?,
    )?;

    // Outsider (other org): every door stays shut.
    assert!(store
        .list_messages(outsider_ctx, conversation.id)
        .await
        .is_err());
    assert!(store
        .add_message(
            outsider_ctx,
            &ChatMessage::new(conversation.id, MessageRole::User, "intrusion")
        )
        .await
        .is_err());
    assert!(store
        .get_collection_item(
            outsider.user.id,
            outsider.active_organization.id,
            PersistedCollection::Memories,
            memory_id,
        )
        .await?
        .is_none());
    assert!(store
        .list_memories(outsider.user.id, outsider.active_organization.id)
        .await?
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn same_org_member_cannot_read_owner_memories() -> TestResult {
    let Some(store) = connect().await? else {
        return Ok(());
    };
    let owner = principal(&store, "rls-mem-owner").await?;
    let member = principal(&store, "rls-mem-member").await?;

    // Member joins the OWNER's organization as a plain member.
    sqlx::query(
        r#"
        INSERT INTO memberships (user_id, organization_id, role, status)
        VALUES ($1, $2, 'member', 'active')
        ON CONFLICT (user_id, organization_id)
        DO UPDATE SET role = excluded.role, status = excluded.status, deleted_at = NULL
        "#,
    )
    .bind(member.user.id)
    .bind(owner.active_organization.id)
    .execute(store.pool())
    .await?;

    let memory = store
        .create_collection_item(
            owner.user.id,
            owner.active_organization.id,
            PersistedCollection::Memories,
            serde_json::json!({ "content": "owner-only fact" }),
            None,
        )
        .await?;
    let memory_id: Uuid = serde_json::from_value(
        memory
            .get("id")
            .cloned()
            .ok_or("memory response has no id")?,
    )?;

    // Owner-scoped reads exclude the member…
    assert!(store
        .list_memories(member.user.id, owner.active_organization.id)
        .await?
        .is_empty());
    assert!(store
        .get_collection_item(
            member.user.id,
            owner.active_organization.id,
            PersistedCollection::Memories,
            memory_id,
        )
        .await?
        .is_none());
    // …while the owner still sees their own row.
    assert_eq!(
        store
            .list_memories(owner.user.id, owner.active_organization.id)
            .await?
            .len(),
        1
    );
    Ok(())
}

/// Phase gate pin: RLS is ENFORCED on private tables since
/// `202607290001_private_data_rls_enforce.sql`. If this test fails because
/// the flags flipped back, treat it as a security regression, not a test
/// bug: re-disabling requires the same review bar as enabling did.
#[tokio::test]
async fn rls_phase_gate_private_tables_enforced() -> TestResult {
    let Some(store) = connect().await? else {
        return Ok(());
    };
    let rows: Vec<(String, bool)> = sqlx::query_as(
        "SELECT relname, relrowsecurity FROM pg_class WHERE relname IN ('conversations','messages','memories')",
    )
    .fetch_all(store.pool())
    .await?;
    assert_eq!(rows.len(), 3, "private tables must exist");
    for (table, enforced) in rows {
        assert!(
            enforced,
            "RLS not enforced on {table}: the phase-2 gate migration must stay applied"
        );
    }
    Ok(())
}
