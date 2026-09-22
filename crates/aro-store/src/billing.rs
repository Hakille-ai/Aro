use super::{map_sqlx, AroStore, TenantContext};
use aro_core::{AroError, AroResult, BillingAccount, MAX_CREDIT_MICROS};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingLedgerEntry {
    pub id: Uuid,
    pub kind: String,
    pub amount_micros: i64,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone)]
pub struct BillingCheckoutIntent {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub actor_id: Uuid,
    pub kind: String,
    pub quantity: i32,
    pub amount_cents: i64,
    pub session_id: Option<String>,
}
#[derive(Debug)]
pub enum BillingReservation {
    New(Uuid),
    Replay(Value),
    Pending,
}

fn invalid(message: &str) -> AroError {
    AroError::Configuration(message.into())
}
fn account_from_row(
    row: &sqlx::postgres::PgRow,
    month_spent_micros: i64,
) -> AroResult<BillingAccount> {
    let plan: String = row.try_get("plan").map_err(map_sqlx)?;
    Ok(BillingAccount {
        plan: serde_json::from_value(Value::String(plan))
            .map_err(|_| invalid("unknown billing plan"))?,
        status: row.try_get("status").map_err(map_sqlx)?,
        seats: row.try_get("seats").map_err(map_sqlx)?,
        valid_until: row.try_get("valid_until").map_err(map_sqlx)?,
        cancel_at_period_end: row.try_get("cancel_at_period_end").map_err(map_sqlx)?,
        balance_micros: row.try_get("balance_micros").map_err(map_sqlx)?,
        reserved_micros: row.try_get("reserved_micros").map_err(map_sqlx)?,
        monthly_limit_micros: row.try_get("monthly_limit_micros").map_err(map_sqlx)?,
        per_request_limit_micros: row.try_get("per_request_limit_micros").map_err(map_sqlx)?,
        month_spent_micros,
    })
}

impl AroStore {
    /// Offline operator action, not a tenant-admin API. The audit table has no aro_app grant.
    pub async fn billing_grant_enterprise(
        &self,
        org: Uuid,
        seats: i32,
        until: DateTime<Utc>,
        reference: &str,
    ) -> AroResult<()> {
        if !(1..=100000).contains(&seats)
            || until <= Utc::now()
            || reference.trim().len() < 3
            || reference.len() > 200
        {
            return Err(invalid("invalid contract grant"));
        }
        let mut tx = self.billing_lock(org).await?;
        let details = serde_json::json!({"seats":seats,"validUntil":until});
        let old:Option<Value>=sqlx::query_scalar("SELECT details FROM billing_operator_actions WHERE organization_id=$1 AND reference=$2").bind(org).bind(reference).fetch_optional(&mut *tx).await.map_err(map_sqlx)?;
        if let Some(old) = old {
            return if old == details {
                Ok(())
            } else {
                Err(invalid("operator_reference_conflict"))
            };
        }
        let has_subscription:bool=sqlx::query_scalar("SELECT stripe_subscription_id IS NOT NULL AND status NOT IN ('inactive','canceled','incomplete_expired') FROM billing_accounts WHERE organization_id=$1").bind(org).fetch_one(&mut *tx).await.map_err(map_sqlx)?;
        if has_subscription {
            return Err(invalid(
                "cancel current Stripe subscription before contract migration",
            ));
        }
        let members:i64=sqlx::query_scalar("SELECT count(*) FROM memberships WHERE organization_id=$1 AND status='active' AND deleted_at IS NULL").bind(org).fetch_one(&mut *tx).await.map_err(map_sqlx)?;
        if members > i64::from(seats) {
            return Err(invalid("contract seats below active members"));
        }
        sqlx::query("INSERT INTO billing_operator_actions(id,organization_id,reference,action,details) VALUES ($1,$2,$3,'enterprise_grant',$4)").bind(Uuid::new_v4()).bind(org).bind(reference).bind(details).execute(&mut *tx).await.map_err(map_sqlx)?;
        sqlx::query("UPDATE billing_accounts SET plan='enterprise',status='active',seats=$2,valid_until=$3,cancel_at_period_end=false,stripe_subscription_id=NULL,updated_at=now() WHERE organization_id=$1").bind(org).bind(seats).bind(until).execute(&mut *tx).await.map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)
    }
    /// Call within the membership activation transaction. Uses the same lock as payment updates.
    pub async fn billing_check_seat(
        tx: &mut Transaction<'_, Postgres>,
        org: Uuid,
    ) -> AroResult<()> {
        sqlx::query("SELECT set_config('aro.organization_id',$1,true)")
            .bind(org.to_string())
            .execute(&mut **tx)
            .await
            .map_err(map_sqlx)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,57201))")
            .bind(org.to_string())
            .execute(&mut **tx)
            .await
            .map_err(map_sqlx)?;
        let row = sqlx::query("SELECT * FROM billing_accounts WHERE organization_id=$1")
            .bind(org)
            .fetch_optional(&mut **tx)
            .await
            .map_err(map_sqlx)?;
        let account = match row {
            Some(row) => account_from_row(&row, 0)?,
            None => BillingAccount::default(),
        };
        if !account.entitlements(Utc::now()).team_administration {
            return Err(AroError::Security(
                "active Business or Enterprise license required".into(),
            ));
        }
        let members:i64=sqlx::query_scalar("SELECT count(*) FROM memberships WHERE organization_id=$1 AND status='active' AND deleted_at IS NULL").bind(org).fetch_one(&mut **tx).await.map_err(map_sqlx)?;
        if members >= i64::from(account.seats) {
            return Err(AroError::Security(
                "no paid seat available; increase subscription seats".into(),
            ));
        }
        Ok(())
    }
    /// Serializes changes for an organization, including provider reconciliation across replicas.
    /// Only call with authenticated tenant scope or a previously verified checkout intent.
    pub async fn billing_lock(
        &self,
        organization_id: Uuid,
    ) -> AroResult<Transaction<'_, Postgres>> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query("SELECT set_config('aro.organization_id', $1, true)")
            .bind(organization_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 57201))")
            .bind(organization_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        sqlx::query(
            "INSERT INTO billing_accounts (organization_id) VALUES ($1) ON CONFLICT DO NOTHING",
        )
        .bind(organization_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        Ok(tx)
    }
    pub async fn billing_account(&self, tenant: TenantContext) -> AroResult<BillingAccount> {
        let mut tx = self.begin_tenant_tx(tenant).await?;
        let row = sqlx::query("SELECT * FROM billing_accounts WHERE organization_id=$1")
            .bind(tenant.organization_id())
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let spent: i64 = sqlx::query_scalar("SELECT COALESCE(-SUM(amount_micros),0)::bigint FROM billing_ledger WHERE organization_id=$1 AND kind='charge' AND created_at >= date_trunc('month', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'")
            .bind(tenant.organization_id()).fetch_one(&mut *tx).await.map_err(map_sqlx)?;
        match row {
            Some(row) => account_from_row(&row, spent),
            None => Ok(BillingAccount::default()),
        }
    }
    pub async fn billing_ledger(
        &self,
        tenant: TenantContext,
    ) -> AroResult<Vec<BillingLedgerEntry>> {
        self.ensure_org_admin(tenant.actor_id(), tenant.organization_id())
            .await?;
        let mut tx = self.begin_tenant_tx(tenant).await?;
        let rows = sqlx::query("SELECT id, kind, amount_micros, created_at FROM billing_ledger WHERE organization_id=$1 ORDER BY created_at DESC, id DESC LIMIT 100").bind(tenant.organization_id()).fetch_all(&mut *tx).await.map_err(map_sqlx)?;
        rows.iter()
            .map(|r| {
                Ok(BillingLedgerEntry {
                    id: r.try_get("id").map_err(map_sqlx)?,
                    kind: r.try_get("kind").map_err(map_sqlx)?,
                    amount_micros: r.try_get("amount_micros").map_err(map_sqlx)?,
                    created_at: r.try_get("created_at").map_err(map_sqlx)?,
                })
            })
            .collect()
    }
    pub async fn billing_set_limits(
        &self,
        tenant: TenantContext,
        monthly: i64,
        per_request: i64,
    ) -> AroResult<()> {
        self.ensure_org_admin(tenant.actor_id(), tenant.organization_id())
            .await?;
        if !(0..=MAX_CREDIT_MICROS).contains(&monthly) || !(0..=monthly).contains(&per_request) {
            return Err(invalid("invalid billing limits"));
        }
        let mut tx = self.billing_lock(tenant.organization_id()).await?;
        sqlx::query("UPDATE billing_accounts SET monthly_limit_micros=$2, per_request_limit_micros=$3, updated_at=now() WHERE organization_id=$1").bind(tenant.organization_id()).bind(monthly).bind(per_request).execute(&mut *tx).await.map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)
    }
    pub async fn billing_create_intent(
        &self,
        tenant: TenantContext,
        id: Uuid,
        kind: &str,
        quantity: i32,
        amount_cents: i64,
    ) -> AroResult<BillingCheckoutIntent> {
        self.ensure_org_admin(tenant.actor_id(), tenant.organization_id())
            .await?;
        if !matches!(kind, "cloud" | "business" | "credit") || !(1..=100000).contains(&quantity) {
            return Err(invalid("invalid checkout"));
        }
        let mut tx = self.billing_lock(tenant.organization_id()).await?;
        let current: (String, Option<String>) = sqlx::query_as(
            "SELECT status,stripe_subscription_id FROM billing_accounts WHERE organization_id=$1",
        )
        .bind(tenant.organization_id())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let enterprise: bool = sqlx::query_scalar(
            "SELECT plan='enterprise' FROM billing_accounts WHERE organization_id=$1",
        )
        .bind(tenant.organization_id())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if kind != "credit" && enterprise {
            return Err(invalid("enterprise_contract_contact_support"));
        }
        if kind != "credit"
            && current.1.is_some()
            && !matches!(
                current.0.as_str(),
                "canceled" | "inactive" | "incomplete_expired"
            )
        {
            return Err(invalid("subscription_exists_use_portal"));
        }
        let other: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM billing_checkout_intents WHERE organization_id=$1 AND kind <> 'credit' AND id <> $2 AND created_at > now()-interval '24 hours')").bind(tenant.organization_id()).bind(id).fetch_one(&mut *tx).await.map_err(map_sqlx)?;
        if kind != "credit" && other {
            return Err(invalid("checkout_already_pending"));
        }
        sqlx::query("INSERT INTO billing_checkout_intents (id, organization_id,actor_id,kind,quantity,amount_cents) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING").bind(id).bind(tenant.organization_id()).bind(tenant.actor_id()).bind(kind).bind(quantity).bind(amount_cents).execute(&mut *tx).await.map_err(map_sqlx)?;
        let row = sqlx::query("SELECT * FROM billing_checkout_intents WHERE id=$1 AND organization_id=$2 AND actor_id=$3").bind(id).bind(tenant.organization_id()).bind(tenant.actor_id()).fetch_optional(&mut *tx).await.map_err(map_sqlx)?.ok_or_else(|| invalid("checkout_key_conflict"))?;
        let stored_kind: String = row.try_get("kind").map_err(map_sqlx)?;
        let created: DateTime<Utc> = row.try_get("created_at").map_err(map_sqlx)?;
        let attached: Option<String> = row.try_get("stripe_session_id").map_err(map_sqlx)?;
        if attached.is_none() && created < Utc::now() - chrono::Duration::hours(23) {
            return Err(invalid("checkout_reconciliation_required_before_retry"));
        }
        let stored_qty: i32 = row.try_get("quantity").map_err(map_sqlx)?;
        let stored_amount: i64 = row.try_get("amount_cents").map_err(map_sqlx)?;
        if stored_kind != kind || stored_qty != quantity || stored_amount != amount_cents {
            return Err(invalid("checkout_key_conflict"));
        }
        let intent = BillingCheckoutIntent {
            id,
            organization_id: tenant.organization_id(),
            actor_id: tenant.actor_id(),
            kind: kind.into(),
            quantity,
            amount_cents,
            session_id: row.try_get("stripe_session_id").map_err(map_sqlx)?,
        };
        tx.commit().await.map_err(map_sqlx)?;
        Ok(intent)
    }
    pub async fn billing_intent(&self, id: Uuid) -> AroResult<Option<BillingCheckoutIntent>> {
        let row = sqlx::query("SELECT * FROM billing_checkout_intents WHERE id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx)?;
        row.map(|r| {
            Ok(BillingCheckoutIntent {
                id,
                organization_id: r.try_get("organization_id").map_err(map_sqlx)?,
                actor_id: r.try_get("actor_id").map_err(map_sqlx)?,
                kind: r.try_get("kind").map_err(map_sqlx)?,
                quantity: r.try_get("quantity").map_err(map_sqlx)?,
                amount_cents: r.try_get("amount_cents").map_err(map_sqlx)?,
                session_id: r.try_get("stripe_session_id").map_err(map_sqlx)?,
            })
        })
        .transpose()
    }
    pub async fn billing_attach_session(
        &self,
        intent: &BillingCheckoutIntent,
        session: &str,
    ) -> AroResult<()> {
        let affected = sqlx::query("UPDATE billing_checkout_intents SET stripe_session_id=$2 WHERE id=$1 AND (stripe_session_id IS NULL OR stripe_session_id=$2)").bind(intent.id).bind(session).execute(&self.pool).await.map_err(map_sqlx)?.rows_affected();
        if affected != 1 {
            return Err(invalid("checkout_session_conflict"));
        }
        Ok(())
    }
    pub async fn billing_customer(&self, tenant: TenantContext) -> AroResult<Option<String>> {
        self.ensure_org_admin(tenant.actor_id(), tenant.organization_id())
            .await?;
        let mut tx = self.begin_tenant_tx(tenant).await?;
        Ok(sqlx::query_scalar::<_, Option<String>>(
            "SELECT stripe_customer_id FROM billing_accounts WHERE organization_id=$1",
        )
        .bind(tenant.organization_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .flatten())
    }
    pub async fn billing_reserve(
        &self,
        tenant: TenantContext,
        key: Uuid,
        hash: &str,
        amount: i64,
    ) -> AroResult<BillingReservation> {
        self.ensure_org_access(tenant.actor_id(), tenant.organization_id())
            .await?;
        let mut tx = self.billing_lock(tenant.organization_id()).await?;
        if let Some(row) = sqlx::query("SELECT request_hash,status,result FROM billing_reservations WHERE organization_id=$1 AND actor_id=$2 AND request_key=$3").bind(tenant.organization_id()).bind(tenant.actor_id()).bind(key).fetch_optional(&mut *tx).await.map_err(map_sqlx)? {
            let stored: String = row.try_get("request_hash").map_err(map_sqlx)?;
            if stored != hash { return Err(invalid("idempotency_key_conflict")); }
            return match row.try_get::<String,_>("status").map_err(map_sqlx)?.as_str() {
                "completed" => match row.try_get::<Option<Value>,_>("result").map_err(map_sqlx)? { Some(result)=>Ok(BillingReservation::Replay(result)),None=>Err(invalid("request_already_charged_response_expired")) },
                "released" => Err(invalid("request_failed_use_new_key")),
                _ => Ok(BillingReservation::Pending),
            };
        }
        let row = sqlx::query("SELECT * FROM billing_accounts WHERE organization_id=$1")
            .bind(tenant.organization_id())
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let spent: i64 = sqlx::query_scalar("SELECT COALESCE(-SUM(amount_micros),0)::bigint FROM billing_ledger WHERE organization_id=$1 AND kind='charge' AND created_at >= date_trunc('month', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'").bind(tenant.organization_id()).fetch_one(&mut *tx).await.map_err(map_sqlx)?;
        account_from_row(&row, spent)?
            .check_reservation(amount)
            .map_err(invalid)?;
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO billing_reservations (id,organization_id,actor_id,request_key,request_hash,reserved_micros,status) VALUES ($1,$2,$3,$4,$5,$6,'reserved')").bind(id).bind(tenant.organization_id()).bind(tenant.actor_id()).bind(key).bind(hash).bind(amount).execute(&mut *tx).await.map_err(map_sqlx)?;
        sqlx::query("UPDATE billing_accounts SET reserved_micros=reserved_micros+$2 WHERE organization_id=$1").bind(tenant.organization_id()).bind(amount).execute(&mut *tx).await.map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(BillingReservation::New(id))
    }
    /// An ambiguous provider outcome stays reserved. Never automatically refund a timed-out call.
    pub async fn billing_settle(
        &self,
        org: Uuid,
        id: Uuid,
        charge: Option<i64>,
        result: Option<&Value>,
        uncertain: bool,
    ) -> AroResult<()> {
        let mut tx = self.billing_lock(org).await?;
        let row = sqlx::query("SELECT status,reserved_micros FROM billing_reservations WHERE id=$1 AND organization_id=$2 FOR UPDATE").bind(id).bind(org).fetch_optional(&mut *tx).await.map_err(map_sqlx)?.ok_or_else(|| invalid("reservation_not_found"))?;
        let status: String = row.try_get("status").map_err(map_sqlx)?;
        if matches!(status.as_str(), "completed" | "released") {
            return Ok(());
        }
        if uncertain {
            sqlx::query(
                "UPDATE billing_reservations SET status='uncertain',updated_at=now() WHERE id=$1",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        } else {
            let reserved: i64 = row.try_get("reserved_micros").map_err(map_sqlx)?;
            let amount = charge.unwrap_or(0);
            if amount < 0 || amount > reserved || (charge.is_some() && result.is_none()) {
                return Err(invalid("invalid_settlement"));
            }
            sqlx::query("UPDATE billing_accounts SET reserved_micros=reserved_micros-$2,balance_micros=balance_micros-$3,updated_at=now() WHERE organization_id=$1").bind(org).bind(reserved).bind(amount).execute(&mut *tx).await.map_err(map_sqlx)?;
            sqlx::query("UPDATE billing_reservations SET status=$2,charged_micros=$3,result=$4,updated_at=now() WHERE id=$1").bind(id).bind(if charge.is_some() {"completed"} else {"released"}).bind(amount).bind(result).execute(&mut *tx).await.map_err(map_sqlx)?;
            if charge.is_some() {
                sqlx::query("INSERT INTO billing_ledger (id,organization_id,reference,kind,amount_micros) VALUES ($1,$2,$3,'charge',$4)").bind(Uuid::new_v4()).bind(org).bind(format!("compute:{id}")).bind(-amount).execute(&mut *tx).await.map_err(map_sqlx)?;
            }
        }
        tx.commit().await.map_err(map_sqlx)
    }
}
