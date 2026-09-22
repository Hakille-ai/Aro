//! Database-level proof that the RLS policies on
//! conversations/messages/memories actually enforce tenant/owner isolation
//! for a least-privilege role.
//!
//! Why this file exists: every other test connects as the migration owner
//! (or superuser), which PostgreSQL exempts from RLS — those suites prove
//! the SQL is valid, never that it enforces. Here we connect as
//! `aro_rls_probe` (non-owner, NOSUPERUSER, NOBYPASSRLS — the production
//! `aro_app` posture) and assert: no context → nothing; foreign context →
//! nothing; own context → own rows; cross-tenant writes → rejected.
//!
//! The role and grants are idempotent so parallel test binaries sharing one
//! scratch database never fight over them. Skipped without `DATABASE_URL`.

use aro_core::{AssistantMode, ChatMessage, MessageRole};
use aro_store::{AroStore, NewUserWithOrg, PersistedCollection, TenantContext};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PROBE_ROLE: &str = "aro_rls_probe";
const PROBE_PASSWORD: &str = "rls-probe-test-pw-01";

fn restricted_url(database_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let (scheme, rest) = database_url
        .split_once("://")
        .ok_or("DATABASE_URL has no scheme")?;
    let after_authority = rest.split_once('@').map(|(_, tail)| tail).unwrap_or(rest);
    Ok(format!("{scheme}://{PROBE_ROLE}:{PROBE_PASSWORD}@{after_authority}"))
}

/// Serializes probe-role setup: parallel tests sharing one scratch database
/// would otherwise concurrently `ALTER` the same `pg_authid` row.
static PROBE_ROLE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn ensure_probe_role(store: &AroStore) -> TestResult {
    let _guard = PROBE_ROLE_LOCK.lock().await;
    sqlx::query(&format!(
        "DO $$ BEGIN
           IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{PROBE_ROLE}') THEN
             CREATE ROLE {PROBE_ROLE} WITH LOGIN PASSWORD '{PROBE_PASSWORD}';
           END IF;
         END $$;"
    ))
    .execute(store.pool())
    .await?;
    sqlx::query(&format!(
        "ALTER ROLE {PROBE_ROLE} WITH LOGIN PASSWORD '{PROBE_PASSWORD}'"
    ))
    .execute(store.pool())
    .await?;
    for grant in [
        "GRANT USAGE ON SCHEMA public TO aro_rls_probe",
        "GRANT SELECT, INSERT, UPDATE, DELETE ON conversations TO aro_rls_probe",
        "GRANT SELECT, INSERT, UPDATE, DELETE ON messages TO aro_rls_probe",
        "GRANT SELECT, INSERT, UPDATE, DELETE ON memories TO aro_rls_probe",
        "GRANT SELECT ON memberships TO aro_rls_probe",
        "GRANT SELECT ON users TO aro_rls_probe",
        "GRANT SELECT ON organizations TO aro_rls_probe",
        "GRANT EXECUTE ON FUNCTION public.aro_private_context_uuid(TEXT) TO aro_rls_probe",
    ] {
        sqlx::query(grant).execute(store.pool()).await?;
    }
    Ok(())
}

struct Fixture {
    probe: sqlx::PgPool,
    owner: aro_store::AuthPrincipal,
    outsider: aro_store::AuthPrincipal,
}

async fn fixture() -> Result<Option<Fixture>, Box<dyn std::error::Error>> {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping RLS enforcement test: DATABASE_URL is not set");
        return Ok(None);
    };
    let store = AroStore::connect(&database_url, 5).await?;
    ensure_probe_role(&store).await?;

    let unique = Uuid::new_v4();
    let mk = |label: &str| NewUserWithOrg {
        email: format!("{label}-{unique}@aro.local"),
        name: format!("{label} User"),
        role_title: None,
        avatar_color: None,
        password_hash: "argon2-test-hash".to_string(),
        organization_name: format!("{label} Org {unique}"),
        organization_domain: None,
        organization_description: None,
    };
    let owner = store.create_user_with_org(mk("rls-enf-owner")).await?;
    let outsider = store.create_user_with_org(mk("rls-enf-outsider")).await?;
    let owner_ctx =
        TenantContext::new(owner.user.id, owner.active_organization.id).expect("tenant ctx");
    let conversation = store
        .create_conversation(owner_ctx, "RLS enforcement".to_string(), AssistantMode::Chat)
        .await?;
    store
        .add_message(
            owner_ctx,
            &ChatMessage::new(conversation.id, MessageRole::User, "rls probe secret"),
        )
        .await?;
    store
        .create_collection_item(
            owner.user.id,
            owner.active_organization.id,
            PersistedCollection::Memories,
            serde_json::json!({ "content": "rls probe fact" }),
            None,
        )
        .await?;

    let probe = sqlx::PgPool::connect(&restricted_url(&database_url)?).await?;
    Ok(Some(Fixture {
        probe,
        owner,
        outsider,
    }))
}

async fn count(
    pool: &sqlx::PgPool,
    actor: Option<(Uuid, Uuid)>,
    table: &str,
) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    if let Some((actor_id, org_id)) = actor {
        sqlx::query("SELECT set_config('aro.actor_id', $1, true), set_config('aro.organization_id', $2, true)")
            .bind(actor_id.to_string())
            .bind(org_id.to_string())
            .execute(&mut *tx)
            .await?;
    }
    let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*)::bigint FROM {table}"))
        .fetch_one(&mut *tx)
        .await?;
    tx.rollback().await?;
    Ok(n)
}

fn owner_ids(f: &Fixture) -> (Uuid, Uuid) {
    (f.owner.user.id, f.owner.active_organization.id)
}

fn outsider_ids(f: &Fixture) -> (Uuid, Uuid) {
    (f.outsider.user.id, f.outsider.active_organization.id)
}

#[tokio::test]
async fn rls_hides_everything_without_session_context() -> TestResult {
    let Some(f) = fixture().await? else {
        return Ok(());
    };
    // Seed at least one row per table so "0" is meaningful.
    assert!(count(&f.probe, Some(owner_ids(&f)), "conversations").await? >= 1);
    for table in ["conversations", "messages", "memories"] {
        assert_eq!(
            count(&f.probe, None, table).await?,
            0,
            "{table} visible without context"
        );
    }
    Ok(())
}

#[tokio::test]
async fn rls_shows_only_own_rows_to_a_tenant() -> TestResult {
    let Some(f) = fixture().await? else {
        return Ok(());
    };
    // Owner sees at least their own rows…
    assert!(count(&f.probe, Some(owner_ids(&f)), "conversations").await? >= 1);
    assert!(count(&f.probe, Some(owner_ids(&f)), "messages").await? >= 1);
    assert!(count(&f.probe, Some(owner_ids(&f)), "memories").await? >= 1);
    // …while a foreign tenant sees none of them.
    for table in ["conversations", "messages", "memories"] {
        assert_eq!(
            count(&f.probe, Some(outsider_ids(&f)), table).await?,
            0,
            "{table} leaked across tenants"
        );
    }
    // Unknown actor (no membership at all) sees nothing either.
    let ghost = (Uuid::new_v4(), Uuid::new_v4());
    for table in ["conversations", "messages", "memories"] {
        assert_eq!(count(&f.probe, Some(ghost), table).await?, 0);
    }
    Ok(())
}

#[tokio::test]
async fn rls_rejects_cross_tenant_writes() -> TestResult {
    let Some(f) = fixture().await? else {
        return Ok(());
    };
    let (actor, _) = owner_ids(&f);
    let (_, foreign_org) = outsider_ids(&f);
    let mut tx = f.probe.begin().await?;
    sqlx::query("SELECT set_config('aro.actor_id', $1, true), set_config('aro.organization_id', $2, true)")
        .bind(actor.to_string())
        .bind(f.owner.active_organization.id.to_string())
        .execute(&mut *tx)
        .await?;
    // Forged organization on INSERT must fail the WITH CHECK policy.
    // A savepoint contains the expected failure: Postgres aborts the whole
    // transaction on error, so the legitimate insert below needs a clean
    // slate.
    sqlx::query("SAVEPOINT rls_probe_bad_insert")
        .execute(&mut *tx)
        .await?;
    let bad: Result<_, sqlx::Error> = sqlx::query(
        "INSERT INTO conversations (organization_id, owner_user_id, title, mode) VALUES ($1, $2, 'x', 'chat')",
    )
    .bind(foreign_org)
    .bind(actor)
    .execute(&mut *tx)
    .await;
    assert!(bad.is_err(), "cross-tenant insert was not rejected");
    sqlx::query("ROLLBACK TO SAVEPOINT rls_probe_bad_insert")
        .execute(&mut *tx)
        .await?;
    // A legitimate insert works under the same context.
    sqlx::query(
        "INSERT INTO conversations (organization_id, owner_user_id, title, mode) VALUES ($1, $2, 'probe ok', 'chat')",
    )
    .bind(f.owner.active_organization.id)
    .bind(actor)
    .execute(&mut *tx)
    .await?;
    tx.rollback().await?;
    Ok(())
}
