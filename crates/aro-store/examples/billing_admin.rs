//! Offline operations with a dedicated database role. No secrets on the command line.
use aro_store::AroStore;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage="Usage: billing_admin grant-enterprise ORG_UUID SEATS VALID_UNTIL_RFC3339 CONTRACT_REFERENCE | pending ORG_UUID | reconcile ORG_UUID RESERVATION_UUID release|CHARGE_MICROS EVIDENCE_REFERENCE | purge-responses ORG_UUID RETENTION_DAYS";
    let command = args.first().ok_or(usage)?.as_str();
    if !matches!(
        (command, args.len()),
        ("grant-enterprise", 5) | ("pending", 2) | ("reconcile", 5) | ("purge-responses", 3)
    ) {
        return Err(usage.into());
    }
    let org: Uuid = args[1].parse()?;
    let store = AroStore::connect(&std::env::var("ARO_BILLING_ADMIN_DATABASE_URL")?, 2).await?;
    // Access to this table is deliberately unavailable to the HTTP application role.
    let authorized: bool = sqlx::query_scalar(
        "SELECT has_table_privilege(current_user,'billing_operator_actions','INSERT')",
    )
    .fetch_one(store.pool())
    .await?;
    if !authorized {
        return Err("a dedicated billing operator database role is required".into());
    }
    match command {
        "grant-enterprise" => {
            let until = DateTime::parse_from_rfc3339(&args[3])?.with_timezone(&Utc);
            store
                .billing_grant_enterprise(org, args[2].parse()?, until, &args[4])
                .await?;
            println!(
                "Enterprise contract recorded for {org}; reference {}",
                args[4]
            );
        }
        "pending" => {
            let mut tx = store.billing_lock(org).await?;
            let rows=sqlx::query("SELECT id,request_key,status,reserved_micros,created_at FROM billing_reservations WHERE organization_id=$1 AND status IN ('reserved','uncertain') ORDER BY created_at").bind(org).fetch_all(&mut *tx).await?;
            for row in rows {
                println!(
                    "{} request={} status={} held={} created={}",
                    row.get::<Uuid, _>("id"),
                    row.get::<Uuid, _>("request_key"),
                    row.get::<String, _>("status"),
                    row.get::<i64, _>("reserved_micros"),
                    row.get::<DateTime<Utc>, _>("created_at")
                );
            }
        }
        "reconcile" => {
            let id: Uuid = args[2].parse()?;
            if args[4].trim().len() < 3 || args[4].len() > 200 {
                return Err("supply a traceable provider/support evidence reference".into());
            }
            let mut tx = store.billing_lock(org).await?;
            let row=sqlx::query("SELECT updated_at,status FROM billing_reservations WHERE organization_id=$1 AND id=$2").bind(org).bind(id).fetch_one(&mut *tx).await?;
            let status: String = row.get("status");
            if matches!(status.as_str(), "completed" | "released") {
                return Err("already settled; inspect ledger before another action".into());
            }
            if row.get::<DateTime<Utc>, _>("updated_at")
                > Utc::now() - chrono::Duration::minutes(10)
            {
                return Err("wait at least ten minutes after last update to avoid racing an active provider call".into());
            }
            tx.rollback().await?;
            let charge = if args[3] == "release" {
                None
            } else {
                Some(args[3].parse::<i64>()?)
            };
            let result = serde_json::json!({"error":{"code":"operator_reconciled","message":"Original response unavailable; contact support with your request ID."},"reconciliation":{"reference":args[4],"chargedMicros":charge,"at":Utc::now()}});
            store
                .billing_settle(org, id, charge, Some(&result), false)
                .await?;
            println!(
                "Reservation {id} reconciled. Evidence reference retained with the reservation."
            );
        }
        "purge-responses" => {
            let days: i32 = args[2].parse()?;
            if !(1..=365).contains(&days) {
                return Err("retention must be 1–365 days".into());
            }
            let mut tx = store.billing_lock(org).await?;
            let count=sqlx::query("UPDATE billing_reservations SET result=NULL WHERE organization_id=$1 AND status='completed' AND result IS NOT NULL AND NOT (result ? 'reconciliation') AND updated_at < now()-make_interval(days => $2)").bind(org).bind(days).execute(&mut *tx).await?.rows_affected();
            sqlx::query("INSERT INTO billing_operator_actions(id,organization_id,reference,action,details) VALUES($1,$2,$3,'purge_responses',$4)").bind(Uuid::new_v4()).bind(org).bind(format!("purge:{}",Uuid::new_v4())).bind(serde_json::json!({"retentionDays":days,"responses":count})).execute(&mut *tx).await?;
            tx.commit().await?;
            println!(
                "Purged {count} responses; retained idempotency tombstones and financial records."
            );
        }
        _ => unreachable!(),
    }
    Ok(())
}
