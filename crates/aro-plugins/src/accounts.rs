use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginAccount {
    pub id: String,
    pub plugin_id: String,
    pub account_identifier: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    pub auth_method: String,
    pub is_default: bool,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePluginAccountInput {
    pub plugin_id: String,
    pub account_identifier: String,
    pub label: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    pub auth_method: String,
    #[serde(default)]
    pub is_default: Option<bool>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginAccountStore {
    db_path: PathBuf,
}

impl PluginAccountStore {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let store = Self { db_path };
        store.init_db()?;
        Ok(store)
    }

    fn open_conn(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path).map_err(|e| {
            anyhow!(
                "Failed to open plugin accounts database at {}: {e}",
                self.db_path.display()
            )
        })?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| {
                anyhow!(
                    "Failed to set busy timeout on plugin accounts database: {e}"
                )
            })?;
        Ok(conn)
    }

    pub fn init_db(&self) -> Result<()> {
        let conn = self.open_conn()?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS plugin_accounts (
                 id TEXT PRIMARY KEY,
                 plugin_id TEXT NOT NULL,
                 account_identifier TEXT NOT NULL,
                 label TEXT NOT NULL,
                 email TEXT,
                 display_name TEXT,
                 avatar_url TEXT,
                 auth_method TEXT NOT NULL,
                 is_default INTEGER NOT NULL DEFAULT 0,
                 status TEXT NOT NULL DEFAULT 'active',
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL,
                 last_used_at TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_plugin_accounts_plugin_id ON plugin_accounts(plugin_id);
             CREATE INDEX IF NOT EXISTS idx_plugin_accounts_default ON plugin_accounts(plugin_id, is_default);
            ",
        ).map_err(|e| anyhow!("Failed to initialize plugin accounts schema: {e}"))?;
        Ok(())
    }

    pub fn list(&self, plugin_id: Option<&str>) -> Result<Vec<PluginAccount>> {
        let conn = self.open_conn()?;
        let mut results = Vec::new();

        if let Some(pid) = plugin_id {
            let mut stmt = conn.prepare(
                "SELECT id, plugin_id, account_identifier, label, email, display_name, avatar_url,
                        auth_method, is_default, status, created_at, updated_at, last_used_at
                 FROM plugin_accounts
                 WHERE plugin_id = ?1
                 ORDER BY is_default DESC, created_at ASC",
            )?;
            let rows = stmt.query_map(params![pid], Self::row_to_account)?;
            for row in rows {
                results.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, plugin_id, account_identifier, label, email, display_name, avatar_url,
                        auth_method, is_default, status, created_at, updated_at, last_used_at
                 FROM plugin_accounts
                 ORDER BY plugin_id ASC, is_default DESC, created_at ASC",
            )?;
            let rows = stmt.query_map([], Self::row_to_account)?;
            for row in rows {
                results.push(row?);
            }
        }

        Ok(results)
    }

    pub fn get(&self, id: &str) -> Result<Option<PluginAccount>> {
        let conn = self.open_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, plugin_id, account_identifier, label, email, display_name, avatar_url,
                    auth_method, is_default, status, created_at, updated_at, last_used_at
             FROM plugin_accounts
             WHERE id = ?1",
        )?;
        let account = stmt
            .query_row(params![id], Self::row_to_account)
            .optional()?;
        Ok(account)
    }

    pub fn create(&self, input: CreatePluginAccountInput) -> Result<PluginAccount> {
        let trimmed_ident = input.account_identifier.trim();
        if trimmed_ident.is_empty() {
            return Err(anyhow!("Account identifier cannot be empty"));
        }
        let label = if input.label.trim().is_empty() {
            trimmed_ident.to_string()
        } else {
            input.label.trim().to_string()
        };

        let mut conn = self.open_conn()?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plugin_accounts WHERE plugin_id = ?1",
            params![input.plugin_id],
            |row| row.get(0),
        )?;

        let is_default = if count == 0 {
            true
        } else {
            input.is_default.unwrap_or(false)
        };

        let tx = conn.transaction()?;
        if is_default && count > 0 {
            tx.execute(
                "UPDATE plugin_accounts SET is_default = 0, updated_at = ?2 WHERE plugin_id = ?1",
                params![input.plugin_id, now],
            )?;
        }

        let status = input.status.unwrap_or_else(|| "active".to_string());
        let is_default_int = if is_default { 1 } else { 0 };

        tx.execute(
            "INSERT INTO plugin_accounts (
                id, plugin_id, account_identifier, label, email, display_name, avatar_url,
                auth_method, is_default, status, created_at, updated_at, last_used_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL)",
            params![
                id,
                input.plugin_id,
                trimmed_ident,
                label,
                input.email,
                input.display_name,
                input.avatar_url,
                input.auth_method,
                is_default_int,
                status,
                now,
                now,
            ],
        )?;

        tx.commit()?;

        Ok(PluginAccount {
            id,
            plugin_id: input.plugin_id,
            account_identifier: trimmed_ident.to_string(),
            label,
            email: input.email,
            display_name: input.display_name,
            avatar_url: input.avatar_url,
            auth_method: input.auth_method,
            is_default,
            status,
            created_at: now.clone(),
            updated_at: now,
            last_used_at: None,
        })
    }

    pub fn set_default(&self, id: &str) -> Result<PluginAccount> {
        let mut conn = self.open_conn()?;
        let account = self
            .get(id)?
            .ok_or_else(|| anyhow!("Account not found with id: {id}"))?;
        let now = Utc::now().to_rfc3339();

        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE plugin_accounts SET is_default = 0, updated_at = ?2 WHERE plugin_id = ?1",
            params![account.plugin_id, now],
        )?;
        tx.execute(
            "UPDATE plugin_accounts SET is_default = 1, updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        tx.commit()?;

        let mut updated = account;
        updated.is_default = true;
        updated.updated_at = now;
        Ok(updated)
    }

    pub fn update_label(&self, id: &str, label: &str) -> Result<PluginAccount> {
        if label.trim().is_empty() {
            return Err(anyhow!("Account label cannot be empty"));
        }
        let conn = self.open_conn()?;
        let account = self
            .get(id)?
            .ok_or_else(|| anyhow!("Account not found with id: {id}"))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE plugin_accounts SET label = ?1, updated_at = ?2 WHERE id = ?3",
            params![label.trim(), now, id],
        )?;

        let mut updated = account;
        updated.label = label.trim().to_string();
        updated.updated_at = now;
        Ok(updated)
    }

    pub fn update_status(&self, id: &str, status: &str) -> Result<PluginAccount> {
        let conn = self.open_conn()?;
        let account = self
            .get(id)?
            .ok_or_else(|| anyhow!("Account not found with id: {id}"))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE plugin_accounts SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now, id],
        )?;

        let mut updated = account;
        updated.status = status.to_string();
        updated.updated_at = now;
        Ok(updated)
    }

    pub fn update_last_used(&self, id: &str) -> Result<()> {
        let conn = self.open_conn()?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE plugin_accounts SET last_used_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let mut conn = self.open_conn()?;
        let account = self
            .get(id)?
            .ok_or_else(|| anyhow!("Account not found with id: {id}"))?;
        let was_default = account.is_default;
        let plugin_id = account.plugin_id.clone();

        let tx = conn.transaction()?;
        tx.execute("DELETE FROM plugin_accounts WHERE id = ?1", params![id])?;

        if was_default {
            let next_id: Option<String> = tx
                .query_row(
                    "SELECT id FROM plugin_accounts WHERE plugin_id = ?1 ORDER BY created_at ASC LIMIT 1",
                    params![plugin_id],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(next) = next_id {
                let now = Utc::now().to_rfc3339();
                tx.execute(
                    "UPDATE plugin_accounts SET is_default = 1, updated_at = ?2 WHERE id = ?1",
                    params![next, now],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn delete_by_plugin(&self, plugin_id: &str) -> Result<()> {
        let conn = self.open_conn()?;
        conn.execute(
            "DELETE FROM plugin_accounts WHERE plugin_id = ?1",
            params![plugin_id],
        )?;
        Ok(())
    }

    fn row_to_account(row: &rusqlite::Row) -> rusqlite::Result<PluginAccount> {
        let is_default_int: i64 = row.get(8)?;
        Ok(PluginAccount {
            id: row.get(0)?,
            plugin_id: row.get(1)?,
            account_identifier: row.get(2)?,
            label: row.get(3)?,
            email: row.get(4)?,
            display_name: row.get(5)?,
            avatar_url: row.get(6)?,
            auth_method: row.get(7)?,
            is_default: is_default_int != 0,
            status: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
            last_used_at: row.get(12)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_account_lifecycle_and_multi_accounts() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("accounts.sqlite3");
        let store = PluginAccountStore::new(db_path).unwrap();

        // 1. Create first account for google-workspace (should automatically be default)
        let acc1 = store
            .create(CreatePluginAccountInput {
                plugin_id: "google-workspace".to_string(),
                account_identifier: "personal@gmail.com".to_string(),
                label: "Personnel".to_string(),
                email: Some("personal@gmail.com".to_string()),
                display_name: Some("Alice Personal".to_string()),
                avatar_url: None,
                auth_method: "oauth2".to_string(),
                is_default: None,
                status: None,
            })
            .unwrap();

        assert!(acc1.is_default, "First account must be default");
        assert_eq!(acc1.label, "Personnel");
        assert_eq!(acc1.status, "active");

        // 2. Create second account for google-workspace without is_default
        let acc2 = store
            .create(CreatePluginAccountInput {
                plugin_id: "google-workspace".to_string(),
                account_identifier: "work@company.com".to_string(),
                label: "Travail".to_string(),
                email: Some("work@company.com".to_string()),
                display_name: Some("Alice Work".to_string()),
                avatar_url: None,
                auth_method: "oauth2".to_string(),
                is_default: Some(false),
                status: None,
            })
            .unwrap();

        assert!(!acc2.is_default);

        // Verify listing for google-workspace
        let list = store.list(Some("google-workspace")).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, acc1.id, "Default account listed first");

        // 3. Set acc2 as default
        let updated_acc2 = store.set_default(&acc2.id).unwrap();
        assert!(updated_acc2.is_default);

        let updated_acc1 = store.get(&acc1.id).unwrap().unwrap();
        assert!(!updated_acc1.is_default, "Previous default must now be false");

        // 4. Update label
        let relabeled = store
            .update_label(&acc1.id, "Perso Secondaire")
            .unwrap();
        assert_eq!(relabeled.label, "Perso Secondaire");

        // 5. Update status
        let status_updated = store.update_status(&acc1.id, "expired").unwrap();
        assert_eq!(status_updated.status, "expired");

        // 6. Delete default account (acc2) -> acc1 should become default
        store.delete(&acc2.id).unwrap();
        let remaining = store.list(Some("google-workspace")).unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].is_default, "Remaining account should become default");
        assert_eq!(remaining[0].id, acc1.id);

        // 7. Delete by plugin
        store.delete_by_plugin("google-workspace").unwrap();
        let empty = store.list(Some("google-workspace")).unwrap();
        assert!(empty.is_empty());
    }
}
