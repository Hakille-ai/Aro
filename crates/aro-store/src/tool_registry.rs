use aro_core::{AroError, AroResult, ToolDescriptor, ToolStatus};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::{map_sqlx, AroStore, TenantContext};

#[derive(Debug, Clone)]
pub struct StoredToolDescriptor {
    pub record_id: Uuid,
    pub descriptor: ToolDescriptor,
    pub descriptor_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AroStore {
    /// Publishes one validated descriptor version and atomically makes it current for its
    /// canonical id. Existing versions remain immutable history except for their current marker.
    pub async fn upsert_tool_descriptor(
        &self,
        context: TenantContext,
        descriptor: &ToolDescriptor,
    ) -> AroResult<StoredToolDescriptor> {
        descriptor.validate()?;
        let descriptor_value = serde_json::to_value(descriptor).map_err(map_descriptor_serde)?;
        let descriptor_hash = tool_descriptor_hash(&descriptor_value)?;
        let status = enum_name(&descriptor.status)?;
        let provenance_kind = enum_name(&descriptor.provenance.kind)?;
        let owner_kind = enum_name(&descriptor.owner.kind)?;
        let revoked_at = (descriptor.status == ToolStatus::Revoked).then(Utc::now);

        let mut tx = self.begin_tenant_tx(context).await?;
        let can_manage: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM memberships
              WHERE organization_id = $1
                AND user_id = $2
                AND role IN ('owner', 'admin')
                AND status = 'active'
                AND deleted_at IS NULL
            )
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if !can_manage {
            return Err(AroError::Security(
                "organization administrator permission is required to publish tools".to_string(),
            ));
        }
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "{}:tool:{}",
                context.organization_id(),
                descriptor.id
            ))
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;

        let alias_conflict = sqlx::query(
            r#"
            SELECT conflict_name, conflict_tool_id
            FROM (
              SELECT alias.alias AS conflict_name, entry.tool_id AS conflict_tool_id
              FROM tool_registry_aliases alias
              JOIN tool_registry_entries entry
                ON entry.organization_id = alias.organization_id
               AND entry.id = alias.entry_id
              WHERE alias.organization_id = $1
                AND alias.alias = $2
                AND entry.tool_id <> $2
              UNION ALL
              SELECT entry.tool_id AS conflict_name, entry.tool_id AS conflict_tool_id
              FROM tool_registry_entries entry
              WHERE entry.organization_id = $1
                AND entry.is_current
                AND entry.tool_id = ANY($3)
                AND entry.tool_id <> $2
              UNION ALL
              SELECT alias.alias AS conflict_name, entry.tool_id AS conflict_tool_id
              FROM tool_registry_aliases alias
              JOIN tool_registry_entries entry
                ON entry.organization_id = alias.organization_id
               AND entry.id = alias.entry_id
              WHERE alias.organization_id = $1
                AND alias.alias = ANY($3)
                AND entry.tool_id <> $2
            ) conflicts
            LIMIT 1
            "#,
        )
        .bind(context.organization_id())
        .bind(&descriptor.id)
        .bind(&descriptor.aliases)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if let Some(conflict) = alias_conflict {
            return Err(AroError::Configuration(format!(
                "tool name `{}` already resolves to `{}`",
                conflict.get::<String, _>("conflict_name"),
                conflict.get::<String, _>("conflict_tool_id")
            )));
        }

        sqlx::query(
            r#"
            UPDATE tool_registry_entries
            SET is_current = false, updated_by_user_id = $3
            WHERE organization_id = $1 AND tool_id = $2 AND is_current
            "#,
        )
        .bind(context.organization_id())
        .bind(&descriptor.id)
        .bind(context.actor_id())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let row = sqlx::query(
            r#"
            INSERT INTO tool_registry_entries (
              organization_id, tool_id, tool_version, descriptor_schema_version, descriptor,
              descriptor_hash, status, provenance_kind, owner_kind, is_current,
              created_by_user_id, updated_by_user_id, revoked_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10, $10, $11)
            ON CONFLICT (organization_id, tool_id, tool_version) DO UPDATE
            SET descriptor_schema_version = EXCLUDED.descriptor_schema_version,
                descriptor = EXCLUDED.descriptor,
                descriptor_hash = EXCLUDED.descriptor_hash,
                status = EXCLUDED.status,
                provenance_kind = EXCLUDED.provenance_kind,
                owner_kind = EXCLUDED.owner_kind,
                is_current = true,
                updated_by_user_id = EXCLUDED.updated_by_user_id,
                revoked_at = EXCLUDED.revoked_at
            RETURNING id, descriptor, descriptor_hash, created_at, updated_at
            "#,
        )
        .bind(context.organization_id())
        .bind(&descriptor.id)
        .bind(&descriptor.version)
        .bind(i32::from(descriptor.schema_version))
        .bind(&descriptor_value)
        .bind(&descriptor_hash)
        .bind(status)
        .bind(provenance_kind)
        .bind(owner_kind)
        .bind(context.actor_id())
        .bind(revoked_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let record_id: Uuid = row.get("id");
        sqlx::query(
            r#"
            DELETE FROM tool_registry_aliases alias
            USING tool_registry_entries entry
            WHERE alias.organization_id = $1
              AND alias.organization_id = entry.organization_id
              AND alias.entry_id = entry.id
              AND entry.tool_id = $2
            "#,
        )
        .bind(context.organization_id())
        .bind(&descriptor.id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        for alias in &descriptor.aliases {
            sqlx::query(
                r#"
                INSERT INTO tool_registry_aliases (organization_id, alias, entry_id)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(context.organization_id())
            .bind(alias)
            .bind(record_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        }

        let stored = map_tool_descriptor_row(&row)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(stored)
    }

    pub async fn list_current_tool_descriptors(
        &self,
        context: TenantContext,
    ) -> AroResult<Vec<StoredToolDescriptor>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, descriptor, descriptor_hash, created_at, updated_at
            FROM tool_registry_entries
            WHERE organization_id = $1 AND is_current
            ORDER BY tool_id
            "#,
        )
        .bind(context.organization_id())
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let descriptors = rows
            .iter()
            .map(map_tool_descriptor_row)
            .collect::<AroResult<Vec<_>>>()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(descriptors)
    }

    pub async fn get_current_tool_descriptor(
        &self,
        context: TenantContext,
        tool_or_alias: &str,
    ) -> AroResult<Option<StoredToolDescriptor>> {
        if tool_or_alias.trim().is_empty() || tool_or_alias.len() > 200 {
            return Err(AroError::Configuration(
                "tool id or alias must contain between 1 and 200 bytes".to_string(),
            ));
        }
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            SELECT entry.id, entry.descriptor, entry.descriptor_hash,
                   entry.created_at, entry.updated_at,
                   (entry.tool_id = $2) AS direct_match
            FROM tool_registry_entries entry
            LEFT JOIN tool_registry_aliases alias
              ON alias.organization_id = entry.organization_id
             AND alias.entry_id = entry.id
            WHERE entry.organization_id = $1
              AND entry.is_current
              AND (entry.tool_id = $2 OR alias.alias = $2)
            ORDER BY direct_match DESC
            LIMIT 1
            "#,
        )
        .bind(context.organization_id())
        .bind(tool_or_alias)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let descriptor = row.as_ref().map(map_tool_descriptor_row).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(descriptor)
    }
}

fn map_tool_descriptor_row(row: &sqlx::postgres::PgRow) -> AroResult<StoredToolDescriptor> {
    let descriptor_value = row.get::<Value, _>("descriptor");
    let stored_hash = row.get::<String, _>("descriptor_hash");
    let actual_hash = tool_descriptor_hash(&descriptor_value)?;
    if actual_hash != stored_hash {
        return Err(AroError::Security(
            "persisted tool descriptor failed its integrity check".to_string(),
        ));
    }
    let descriptor: ToolDescriptor =
        serde_json::from_value(descriptor_value).map_err(map_descriptor_serde)?;
    descriptor.validate()?;
    Ok(StoredToolDescriptor {
        record_id: row.get("id"),
        descriptor,
        descriptor_hash: stored_hash,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn tool_descriptor_hash(descriptor: &Value) -> AroResult<String> {
    let canonical = serde_json::to_vec(descriptor).map_err(map_descriptor_serde)?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

fn enum_name<T: serde::Serialize>(value: &T) -> AroResult<String> {
    serde_json::to_value(value)
        .map_err(map_descriptor_serde)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| AroError::Unexpected("tool registry enum did not serialize as text".into()))
}

fn map_descriptor_serde(error: serde_json::Error) -> AroError {
    AroError::Unexpected(format!("tool descriptor serialization failed: {error}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn descriptor_hash_is_stable_and_content_sensitive() {
        let first = tool_descriptor_hash(&json!({"a": 1, "b": 2})).unwrap();
        let reordered = tool_descriptor_hash(&json!({"b": 2, "a": 1})).unwrap();
        let changed = tool_descriptor_hash(&json!({"a": 1, "b": 3})).unwrap();
        assert_eq!(first, reordered);
        assert_ne!(first, changed);
    }
}
