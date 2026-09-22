//! Run only against a disposable database: DATABASE_URL=... cargo test -p aro-store --test billing
use aro_store::{AroStore, BillingReservation, NewUserWithOrg, TenantContext};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires ARO_BILLING_TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
async fn concurrent_reservations_replay_settlement_and_tenant_isolation(
) -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("ARO_BILLING_TEST_DATABASE_URL")
        .expect("set ARO_BILLING_TEST_DATABASE_URL to a disposable database");
    let store = AroStore::connect(&url, 8).await?;
    store.migrate().await?;
    let mut tenants = Vec::new();
    for _ in 0..2 {
        let id = Uuid::new_v4();
        let owner = store
            .create_user_with_org(NewUserWithOrg {
                email: format!("billing-{id}@example.test"),
                name: "Billing test".into(),
                role_title: None,
                avatar_color: None,
                password_hash: "fixture-not-a-password".into(),
                organization_name: format!("Billing {id}"),
                organization_domain: None,
                organization_description: None,
            })
            .await?;
        tenants.push(TenantContext::new(
            owner.user.id,
            owner.active_organization.id,
        )?);
    }
    let a = tenants[0];
    let b = tenants[1];
    store.billing_set_limits(a, 100, 100).await?;
    let mut tx = store.billing_lock(a.organization_id()).await?;
    sqlx::query("UPDATE billing_accounts SET balance_micros=100 WHERE organization_id=$1")
        .bind(a.organization_id())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let key1 = Uuid::new_v4();
    let key2 = Uuid::new_v4();
    let (first, second) = tokio::join!(
        store.billing_reserve(a, key1, "one", 70),
        store.billing_reserve(a, key2, "two", 70)
    );
    assert_ne!(
        first.is_ok(),
        second.is_ok(),
        "two concurrent calls must not spend the same balance"
    );
    let (reservation, key, hash) = match (first, second) {
        (Ok(BillingReservation::New(id)), Err(_)) => (id, key1, "one"),
        (Err(_), Ok(BillingReservation::New(id))) => (id, key2, "two"),
        other => panic!("unexpected reservation results: {other:?}"),
    };
    assert!(matches!(
        store.billing_reserve(a, key, hash, 70).await?,
        BillingReservation::Pending
    ));
    assert!(store
        .billing_reserve(a, key, "changed-payload", 70)
        .await
        .is_err());
    assert!(store
        .billing_settle(
            b.organization_id(),
            reservation,
            Some(40),
            Some(&json!({"ok":true})),
            false
        )
        .await
        .is_err());
    store
        .billing_settle(a.organization_id(), reservation, None, None, true)
        .await?;
    assert_eq!(store.billing_account(a).await?.reserved_micros, 70);
    let result = json!({"choices":[{"message":{"content":"paid once"}}]});
    store
        .billing_settle(
            a.organization_id(),
            reservation,
            Some(40),
            Some(&result),
            false,
        )
        .await?;
    store
        .billing_settle(
            a.organization_id(),
            reservation,
            Some(40),
            Some(&result),
            false,
        )
        .await?;
    let account = store.billing_account(a).await?;
    assert_eq!(
        (
            account.balance_micros,
            account.reserved_micros,
            account.month_spent_micros
        ),
        (60, 0, 40)
    );
    assert_eq!(store.billing_ledger(a).await?.len(), 1);
    assert!(
        matches!(store.billing_reserve(a,key,hash,70).await?,BillingReservation::Replay(value) if value==result)
    );
    assert_eq!(store.billing_account(b).await?.balance_micros, 0);
    let forged = TenantContext::new(b.actor_id(), a.organization_id())?;
    assert!(store.billing_account(forged).await.is_err());
    assert!(store.billing_set_limits(forged, 100, 100).await.is_err());
    let mut tx = store.billing_lock(a.organization_id()).await?;
    assert!(AroStore::billing_check_seat(&mut tx, a.organization_id())
        .await
        .is_err());
    sqlx::query("UPDATE billing_accounts SET plan='business',status='active',valid_until=now()+interval '1 day',seats=1 WHERE organization_id=$1").bind(a.organization_id()).execute(&mut *tx).await?;
    assert!(AroStore::billing_check_seat(&mut tx, a.organization_id())
        .await
        .is_err());
    sqlx::query("UPDATE billing_accounts SET seats=2 WHERE organization_id=$1")
        .bind(a.organization_id())
        .execute(&mut *tx)
        .await?;
    AroStore::billing_check_seat(&mut tx, a.organization_id()).await?;
    tx.rollback().await?;
    let until = chrono::Utc::now() + chrono::Duration::days(365);
    store
        .billing_grant_enterprise(a.organization_id(), 5, until, "contract-fixture")
        .await?;
    store
        .billing_grant_enterprise(a.organization_id(), 5, until, "contract-fixture")
        .await?;
    assert!(store
        .billing_grant_enterprise(a.organization_id(), 6, until, "contract-fixture")
        .await
        .is_err());
    assert!(
        store
            .billing_account(a)
            .await?
            .entitlements(chrono::Utc::now())
            .commercial_use
    );
    let mut tx = store.billing_lock(a.organization_id()).await?;
    sqlx::query("UPDATE billing_reservations SET result=NULL WHERE id=$1")
        .bind(reservation)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    assert!(
        store.billing_reserve(a, key, hash, 70).await.is_err(),
        "purged responses must never execute again"
    );
    assert_eq!(store.billing_account(a).await?.balance_micros, 60);
    store.billing_set_limits(b, 10, 10).await?;
    let mut tx = store.pool().begin().await?;
    // Disposable non-owner role, created and rolled back in this transaction only.
    // Do not change cluster-wide production role definitions to run this test.
    let role = format!("billing_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(
        "CREATE ROLE {role} NOLOGIN NOSUPERUSER NOBYPASSRLS"
    ))
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!("GRANT SELECT ON billing_accounts TO {role}"))
        .execute(&mut *tx)
        .await?;
    sqlx::query(&format!(
        "GRANT EXECUTE ON FUNCTION public.aro_private_context_uuid(text) TO {role}"
    ))
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!("SET LOCAL ROLE {role}"))
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT set_config('aro.organization_id',$1,true)")
        .bind(a.organization_id().to_string())
        .execute(&mut *tx)
        .await?;
    let other_accounts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM billing_accounts WHERE organization_id=$1")
            .bind(b.organization_id())
            .fetch_one(&mut *tx)
            .await?;
    assert_eq!(
        other_accounts, 0,
        "non-owner application role cannot read another tenant's balance"
    );
    let operator_access: bool = sqlx::query_scalar(
        "SELECT has_table_privilege(current_user,'billing_operator_actions','INSERT')",
    )
    .fetch_one(&mut *tx)
    .await?;
    assert!(
        !operator_access,
        "HTTP application role cannot grant contracts"
    );
    tx.rollback().await?;
    Ok(())
}
