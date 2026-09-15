use aro_agent::built_in_tool_descriptors;
use aro_core::ToolStatus;
use aro_store::{AroStore, TenantContext};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn tool_registry_versions_aliases_and_tenants_are_isolated() -> TestResult {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping tool registry integration test: DATABASE_URL is not set");
        return Ok(());
    };
    let store = AroStore::connect(&database_url, 3).await?;
    let user_id = Uuid::new_v4();
    let organization_id = Uuid::new_v4();
    let other_organization_id = Uuid::new_v4();
    let email = format!("tool-registry-{user_id}@example.test");

    sqlx::query(
        "INSERT INTO users (id, email, name, password_hash) VALUES ($1, $2, 'Tool registry test', 'test')",
    )
    .bind(user_id)
    .bind(email)
    .execute(store.pool())
    .await?;
    for (id, name) in [
        (organization_id, "Tool registry tenant"),
        (other_organization_id, "Tool registry other tenant"),
    ] {
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(id)
            .bind(name)
            .execute(store.pool())
            .await?;
        sqlx::query(
            r#"
            INSERT INTO memberships (user_id, organization_id, role, status)
            VALUES ($1, $2, 'owner', 'active')
            "#,
        )
        .bind(user_id)
        .bind(id)
        .execute(store.pool())
        .await?;
    }

    let context = TenantContext::new(user_id, organization_id)?;
    let other_context = TenantContext::new(user_id, other_organization_id)?;
    let mut descriptor = built_in_tool_descriptors().remove(0);
    descriptor.id = "organization.test.registry".to_string();
    descriptor.version = "1.0.0".to_string();
    descriptor.aliases = vec!["organization.test.alias".to_string()];

    let first = store.upsert_tool_descriptor(context, &descriptor).await?;
    assert_eq!(first.descriptor_hash.len(), 64);
    assert_eq!(
        store
            .get_current_tool_descriptor(context, "organization.test.alias")
            .await?
            .expect("alias resolves")
            .descriptor
            .version,
        "1.0.0"
    );
    assert!(store
        .list_current_tool_descriptors(other_context)
        .await?
        .is_empty());

    descriptor.version = "1.1.0".to_string();
    descriptor.status = ToolStatus::Disabled;
    let second = store.upsert_tool_descriptor(context, &descriptor).await?;
    assert_ne!(first.record_id, second.record_id);
    let current = store.list_current_tool_descriptors(context).await?;
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].descriptor.version, "1.1.0");
    assert_eq!(current[0].descriptor.status, ToolStatus::Disabled);
    assert_eq!(
        store
            .get_current_tool_descriptor(context, "organization.test.alias")
            .await?
            .expect("alias follows current version")
            .record_id,
        second.record_id
    );

    let versions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM tool_registry_entries WHERE organization_id = $1 AND tool_id = $2",
    )
    .bind(organization_id)
    .bind(&descriptor.id)
    .fetch_one(store.pool())
    .await?;
    assert_eq!(versions, 2);

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(store.pool())
        .await?;
    sqlx::query("DELETE FROM organizations WHERE id = ANY($1)")
        .bind(vec![organization_id, other_organization_id])
        .execute(store.pool())
        .await?;
    Ok(())
}
