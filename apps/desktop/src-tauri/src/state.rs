use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

use aro_core::{
    AppSettings, AroError, AroResult, AuthSession, PermissionProfile, SendMessageResponse,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::AssistantEngine;
use chrono::{Duration, Utc};
use directories::ProjectDirs;
use tauri::async_runtime::Mutex;

use crate::api_client::{
    clear_refresh_token, load_provider_api_key, load_refresh_token, save_refresh_token,
    CloudApiClient, LocalResultRequest, OrganizationCreateRequest,
};

pub struct AppState {
    pub engine: AssistantEngine,
    pub cloud: CloudApiClient,
    pub paths: AppPaths,
    settings: Mutex<AppSettings>,
    cloud_session: Mutex<Option<AuthSession>>,
    /// Conversations avec une génération en cours : garde anti double-envoi
    /// (le frontend peut émettre deux invokes quasi simultanés).
    inflight_conversations: Mutex<HashSet<uuid::Uuid>>,
}

impl AppState {
    /// Réserve une génération pour la conversation (`false` si déjà en cours).
    pub async fn try_begin_send(&self, conversation_id: uuid::Uuid) -> bool {
        self.inflight_conversations
            .lock()
            .await
            .insert(conversation_id)
    }

    pub async fn finish_send(&self, conversation_id: &uuid::Uuid) {
        self.inflight_conversations
            .lock()
            .await
            .remove(conversation_id);
    }

    pub async fn settings(&self) -> AppSettings {
        let mut settings = self.settings.lock().await.clone();
        let keyring_id = format!("search-{}", settings.search.provider);
        settings.search.api_key = load_provider_api_key(&keyring_id).ok().flatten();
        settings.search.auth_configured = settings.search.api_key.is_some();
        settings
    }

    pub async fn replace_settings(&self, mut settings: AppSettings) -> AroResult<()> {
        settings.search.api_key = None;
        save_settings(&self.paths.settings_file, &settings)?;
        *self.settings.lock().await = settings;
        Ok(())
    }

    pub async fn set_cloud_session(&self, session: AuthSession) -> AroResult<AuthSession> {
        let mut guard = self.cloud_session.lock().await;
        // The server may already have consumed a predecessor token. Install the live successor
        // in memory before touching the fallible OS keyring so a persistence failure can never
        // leave the revoked predecessor as the application's active credential.
        install_cloud_session_locked(&mut guard, session).await
    }

    pub async fn clear_cloud_session(&self) -> AroResult<()> {
        let clear_result = clear_refresh_token();
        *self.cloud_session.lock().await = None;
        clear_result
    }

    pub async fn refresh_cloud_session_from_keyring(&self) -> AroResult<Option<AuthSession>> {
        let mut session_guard = self.cloud_session.lock().await;
        self.ensure_cloud_session_locked(&mut session_guard).await
    }

    async fn ensure_cloud_session_locked(
        &self,
        session_guard: &mut Option<AuthSession>,
    ) -> AroResult<Option<AuthSession>> {
        let token_to_refresh = if let Some(session) = &*session_guard {
            if session.expires_at > Utc::now() + Duration::seconds(45) {
                return Ok(Some(session.clone()));
            }
            Some(session.refresh_token.clone())
        } else {
            None
        };

        if let Some(token) = token_to_refresh {
            return self.refresh_with_token_locked(session_guard, token).await;
        }

        let Some(refresh_token) = load_refresh_token()? else {
            return Ok(None);
        };
        self.refresh_with_token_locked(session_guard, refresh_token)
            .await
    }

    pub async fn switch_cloud_organization(&self, organization_id: &str) -> AroResult<AuthSession> {
        let mut guard = self.cloud_session.lock().await;
        let session = self
            .ensure_cloud_session_locked(&mut guard)
            .await?
            .ok_or_else(|| AroError::Security("cloud session required".to_string()))?;
        let switched = self
            .cloud
            .switch_organization(
                &session.access_token,
                &session.refresh_token,
                organization_id,
            )
            .await?;
        install_cloud_session_locked(&mut guard, switched).await
    }

    pub async fn create_and_switch_cloud_organization(
        &self,
        request: &OrganizationCreateRequest,
    ) -> AroResult<AuthSession> {
        let mut guard = self.cloud_session.lock().await;
        let session = self
            .ensure_cloud_session_locked(&mut guard)
            .await?
            .ok_or_else(|| AroError::Security("cloud session required".to_string()))?;
        let created = self
            .cloud
            .create_organization(&session.access_token, request)
            .await?;
        let switched = self
            .cloud
            .switch_organization(
                &session.access_token,
                &session.refresh_token,
                &created.organization.id.to_string(),
            )
            .await?;
        install_cloud_session_locked(&mut guard, switched).await
    }

    pub async fn logout_cloud_session(&self) -> AroResult<()> {
        let mut guard = self.cloud_session.lock().await;
        let refresh_token = if let Some(session) = guard.as_ref() {
            Some(session.refresh_token.clone())
        } else {
            load_refresh_token()?
        };
        let remote_result = if let Some(refresh_token) = refresh_token {
            self.cloud.logout(refresh_token).await
        } else {
            Ok(())
        };
        let clear_result = clear_refresh_token();
        *guard = None;
        clear_result?;
        remote_result
    }

    async fn refresh_with_token_locked(
        &self,
        guard: &mut Option<AuthSession>,
        refresh_token: String,
    ) -> AroResult<Option<AuthSession>> {
        match self.cloud.refresh(refresh_token).await {
            Ok(session) => {
                let session = install_cloud_session_locked(guard, session).await?;
                Ok(Some(session))
            }
            Err(err) => {
                if should_clear_refresh_token(&err) {
                    let _ = clear_refresh_token();
                    *guard = None;
                }
                Err(err)
            }
        }
    }

    pub async fn persist_local_result(&self, response: &SendMessageResponse) -> AroResult<()> {
        let Some(session) = self.refresh_cloud_session_from_keyring().await? else {
            return Ok(());
        };
        self.cloud
            .store_local_result(
                &session.access_token,
                &LocalResultRequest {
                    conversation: response.conversation.clone(),
                    user_message: response.user_message.clone(),
                    assistant_message: response.assistant_message.clone(),
                },
            )
            .await
    }
}

async fn install_cloud_session_locked(
    guard: &mut Option<AuthSession>,
    session: AuthSession,
) -> AroResult<AuthSession> {
    *guard = Some(session.clone());
    if let Err(error) = persist_rotated_refresh_token(&session.refresh_token).await {
        // The predecessor may already be revoked server-side. Remove any stale keyring value,
        // retain the live in-memory successor, and surface the durability risk.
        let _ = clear_refresh_token();
        return Err(error);
    }
    Ok(session)
}

async fn persist_rotated_refresh_token(refresh_token: &str) -> AroResult<()> {
    let mut last_error = None;
    for delay_ms in [0_u64, 50, 200] {
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        match save_refresh_token(refresh_token) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AroError::Configuration("could not persist rotated cloud session".to_string())
    }))
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub settings_file: PathBuf,
    pub database_file: PathBuf,
    pub scratch_dir: PathBuf,
}

pub fn load_or_create_state() -> AroResult<AppState> {
    let paths = resolve_paths()?;
    fs::create_dir_all(&paths.data_dir).map_err(|err| AroError::Configuration(err.to_string()))?;
    fs::create_dir_all(&paths.scratch_dir)
        .map_err(|err| AroError::Configuration(err.to_string()))?;

    let mut settings = load_settings(&paths.settings_file)?;
    // P0 migration: historical search credentials were serialized in settings.json. Treat them
    // as compromised, erase the local copy immediately, and require explicit secure re-entry.
    if settings.search.api_key.take().is_some() {
        settings.search.auth_configured = false;
        save_settings(&paths.settings_file, &settings)?;
    }
    let store = SqliteMemoryStore::new(&paths.database_file)?;
    if let Ok(trusted_root) = env::var("ARO_TRUSTED_WORKSPACE_ROOT") {
        if !trusted_root.trim().is_empty() {
            let _ = store
                .upsert_permission_profile(&PermissionProfile::trusted_workspace(trusted_root));
        }
    }
    let plugins_dir = paths.data_dir.join("plugins");
    let skill_registry = std::sync::Arc::new(aro_skills::SkillRegistry::new());
    let plugin_manager =
        std::sync::Arc::new(aro_plugins::PluginManager::new(plugins_dir, skill_registry));
    let engine = AssistantEngine::with_plugin_manager(store, plugin_manager);
    let api_base_url =
        env::var("ARO_API_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8710".to_string());

    Ok(AppState {
        engine,
        cloud: CloudApiClient::try_new(api_base_url)?,
        paths,
        settings: Mutex::new(settings),
        cloud_session: Mutex::new(None),
        inflight_conversations: Mutex::new(HashSet::new()),
    })
}

fn resolve_paths() -> AroResult<AppPaths> {
    let project_dirs = ProjectDirs::from("com", "ARO", "ARO").ok_or_else(|| {
        AroError::Configuration("could not resolve platform app data directory".to_string())
    })?;
    let data_dir = project_dirs.data_dir().to_path_buf();
    Ok(AppPaths {
        settings_file: data_dir.join("settings.json"),
        database_file: data_dir.join("aro.sqlite3"),
        scratch_dir: data_dir.join("scratch"),
        data_dir,
    })
}

fn load_settings(path: &Path) -> AroResult<AppSettings> {
    if !path.exists() {
        let settings = AppSettings::default();
        save_settings(path, &settings)?;
        return Ok(settings);
    }

    let raw = fs::read_to_string(path).map_err(|err| AroError::Configuration(err.to_string()))?;
    let normalized = raw.trim_start_matches('\u{feff}');
    if normalized.trim().is_empty() {
        let settings = AppSettings::default();
        save_settings(path, &settings)?;
        return Ok(settings);
    }

    serde_json::from_str(normalized).map_err(|err| AroError::Configuration(err.to_string()))
}

fn save_settings(path: &Path, settings: &AppSettings) -> AroResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| AroError::Configuration(err.to_string()))?;
    }

    let raw = serde_json::to_string_pretty(settings)
        .map_err(|err| AroError::Configuration(err.to_string()))?;
    fs::write(path, raw).map_err(|err| AroError::Configuration(err.to_string()))
}

fn should_clear_refresh_token(error: &AroError) -> bool {
    matches!(error, AroError::Security(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_settings_accepts_utf8_bom() {
        let path =
            std::env::temp_dir().join(format!("aro-settings-bom-{}.json", uuid::Uuid::new_v4()));
        let json = serde_json::to_string_pretty(&AppSettings::default()).expect("settings json");
        fs::write(&path, format!("\u{feff}{json}")).expect("write settings");

        let settings = load_settings(&path).expect("load settings");
        assert!(settings.retain_history);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn refresh_failures_only_clear_token_for_auth_errors() {
        assert!(should_clear_refresh_token(&AroError::Security(
            "refresh token expired".to_string()
        )));
        assert!(!should_clear_refresh_token(&AroError::RuntimeUnavailable(
            "connection refused".to_string()
        )));
        assert!(!should_clear_refresh_token(&AroError::Configuration(
            "keyring unavailable".to_string()
        )));
    }
}
