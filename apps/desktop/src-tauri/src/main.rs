mod api_client;
mod plugin_accounts;
mod state;
mod billing;
mod workspace_patch;
use workspace_patch::apply_unified_patch;

use std::collections::BTreeMap;

use api_client::{
    clear_provider_api_key, load_provider_api_key, save_provider_api_key, LoginRequest,
    MembershipCreateRequest, OrganizationCreateRequest, OrganizationInvitationPage,
    OrganizationInvitationReceipt, OrganizationPatch, RegisterRequest, UserPreferencesPatch,
    UserProfilePatch,
};
use aro_core::{
    AgentContextItem, AgentLaneView, AgentOrchestratorSnapshot, AgentRun, AgentRunPriority,
    AgentRunStartRequest, AgentRunStatus, AgentRunView, AppSettings, AroError, AroResult,
    AssistantMode, AuthSession, ChatMessage, Conversation, Episode, FileObject, Folder, LongTermMemory,
    Membership, MembershipRole, ModelProviderConnection, ModelProviderKind, ModelRef,
    NotificationFilter, NotificationItem, Organization,
    OrganizationMember, PermissionProfile, Project, PublicApiKey, RuntimeStatus,
    SendMessageRequest, SendMessageResponse, SyncStatus, SynthesisRequest, SynthesisResult,
    TranscriptionRequest, TranscriptionResult, User, UserPreferences, VoiceReadinessIssue,
    VoiceReadinessIssueCode, VoiceRuntimeKind, VoiceSettings, VoiceStatus,
    WakeWordDetectionRequest as CoreWakeWordDetectionRequest, WakeWordRuntimeKind,
};
use aro_runtime::{list_models_for_connection, ModelRouter};
use aro_vector::{MemoryIndexStatus, MemoryReindexReport};
use aro_voice::VoiceService;
use serde::{Deserialize, Serialize};
use state::{load_or_create_state, AppState};
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use url::Url;
use uuid::Uuid;

type CommandResult<T> = Result<T, String>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    settings: AppSettings,
    conversations: Vec<Conversation>,
    runtime: RuntimeStatus,
    voice_status: VoiceStatus,
    voice_model_status: VoiceModelsStatus,
    models: Vec<ModelRef>,
    model_providers: Vec<ModelProviderConnection>,
    current_user: Option<User>,
    active_organization: Option<Organization>,
    memberships: Vec<Membership>,
    preferences: Option<UserPreferences>,
    sync_status: SyncStatus,
    client_state: BTreeMap<String, serde_json::Value>,
    cloud_authenticated: bool,
    api_base_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudSessionView {
    user: User,
    active_organization: Organization,
    memberships: Vec<Membership>,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyCreateView {
    key: PublicApiKey,
    secret: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateConversationRequest {
    title: Option<String>,
    mode: AssistantMode,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelProviderSecretRequest {
    provider_id: String,
    api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelSelectRequest {
    provider_id: String,
    model_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileUploadCommandRequest {
    original_name: String,
    mime_type: String,
    bytes: Vec<u8>,
    sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
enum VoiceModelCapability {
    SpeechToText,
    TextToSpeech,
    WakeWord,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
enum VoiceModelRuntimeKind {
    Disabled,
    WhisperCpp,
    Piper,
    LocalModel,
}

impl From<&VoiceRuntimeKind> for VoiceModelRuntimeKind {
    fn from(runtime: &VoiceRuntimeKind) -> Self {
        match runtime {
            VoiceRuntimeKind::Disabled => Self::Disabled,
            VoiceRuntimeKind::WhisperCpp => Self::WhisperCpp,
            VoiceRuntimeKind::Piper => Self::Piper,
        }
    }
}

impl From<&WakeWordRuntimeKind> for VoiceModelRuntimeKind {
    fn from(runtime: &WakeWordRuntimeKind) -> Self {
        match runtime {
            WakeWordRuntimeKind::Disabled => Self::Disabled,
            WakeWordRuntimeKind::LocalModel => Self::LocalModel,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VoiceModelReadiness {
    capability: VoiceModelCapability,
    runtime: VoiceModelRuntimeKind,
    ready: bool,
    optional: bool,
    binary_path: Option<String>,
    model_path: Option<String>,
    detail: String,
    issues: Vec<VoiceReadinessIssue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VoiceModelsStatus {
    enabled: bool,
    ready: bool,
    voice_status: VoiceStatus,
    speech_to_text: VoiceModelReadiness,
    text_to_speech: VoiceModelReadiness,
    wake_word: VoiceModelReadiness,
    models: Vec<VoiceModelReadiness>,
    checked_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WakeWordDetectionRequest {
    audio_bytes: Vec<u8>,
    mime_type: String,
    language: Option<String>,
    variants: Option<Vec<String>>,
    allow_embedded_aro: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WakeWordDetectionResult {
    detected: bool,
    text: String,
    matched_variant: Option<String>,
    runtime_detail: String,
    score: Option<f32>,
    threshold: Option<f32>,
    checked_at: chrono::DateTime<chrono::Utc>,
}

#[tauri::command]
async fn app_bootstrap(state: State<'_, AppState>) -> CommandResult<BootstrapPayload> {
    if let Err(err) = state.engine.plugins().load_all().await {
        tracing::warn!(?err, "failed to load plugins during app bootstrap");
    }

    let mut settings = state.settings().await;
    let mut conversations = state.engine.conversations().map_err(to_command_error)?;
    let mut current_user = None;
    let mut active_organization = None;
    let mut memberships = Vec::new();
    let mut preferences = None;
    let mut sync_status = SyncStatus::default();
    let mut client_state = BTreeMap::new();
    let mut cloud_authenticated = false;

    match state.refresh_cloud_session_from_keyring().await {
        Ok(Some(session)) => {
            cloud_authenticated = true;
            current_user = Some(session.user.clone());
            active_organization = Some(session.active_organization.clone());
            memberships = session.memberships.clone();

            match state.cloud.bootstrap(&session.access_token).await {
                Ok(cloud_payload) => {
                    settings = cloud_payload.settings;
                    let mut merged_conversations =
                        Vec::with_capacity(cloud_payload.conversations.len());
                    for cloud_conv in &cloud_payload.conversations {
                        let (merged, needs_push) = merge_cloud_conversation(&state, cloud_conv);
                        let _ = state.engine.upsert_conversation(&merged);
                        if needs_push {
                            let _ = state
                                .cloud
                                .move_conversation(
                                    &session.access_token,
                                    &merged.id.to_string(),
                                    merged.project_id.map(|id| id.to_string()),
                                    merged.folder_id.map(|id| id.to_string()),
                                )
                                .await;
                        }
                        merged_conversations.push(merged);
                    }
                    // Le local reste la source de vérité affichée : il
                    // contient aussi les conversations jamais poussées.
                    conversations = match state.engine.conversations() {
                        Ok(local) => local,
                        Err(err) => {
                            tracing::warn!(error = %err, "local conversation list failed during bootstrap; showing merged cloud list");
                            merged_conversations
                        }
                    };
                    current_user = Some(cloud_payload.current_user);
                    active_organization = Some(cloud_payload.active_organization);
                    memberships = cloud_payload.memberships;
                    preferences = Some(cloud_payload.preferences);
                    sync_status = cloud_payload.sync_status;
                    client_state = cloud_payload.client_state;
                }
                Err(err) => {
                    sync_status.health = sync_health_for_cloud_error(&err);
                    if matches!(err, AroError::Security(_)) {
                        let _ = state.clear_cloud_session().await;
                        current_user = None;
                        active_organization = None;
                        memberships = Vec::new();
                        cloud_authenticated = false;
                    }
                }
            }
        }
        Ok(None) => {}
        Err(err) => {
            sync_status.health = sync_health_for_cloud_error(&err);
        }
    }

    let api_key = active_provider_api_key(&settings)?;
    let mut runtime = match ModelRouter::from_active(&settings.model, api_key) {
        Ok(provider) => state.engine.check_runtime(&provider).await,
        Err(err) => runtime_unavailable_from_settings(&settings, err),
    };
    let voice_model_status = voice_model_status_for_settings(settings.voice.clone()).await?;
    let voice_status = voice_model_status.voice_status.clone();
    runtime.set_voice_status(&voice_status);
    let models = model_options_for_settings(&settings).await;
    let settings = settings_with_auth_status(settings);
    let model_providers = settings.model.providers.clone();

    Ok(BootstrapPayload {
        settings,
        conversations,
        runtime,
        voice_status,
        voice_model_status,
        models,
        model_providers,
        current_user,
        active_organization,
        memberships,
        preferences,
        sync_status,
        client_state,
        cloud_authenticated,
        api_base_url: state.cloud.base_url().to_string(),
    })
}

#[tauri::command]
async fn cloud_session_get(state: State<'_, AppState>) -> CommandResult<Option<CloudSessionView>> {
    Ok(state
        .refresh_cloud_session_from_keyring()
        .await
        .ok()
        .flatten()
        .map(session_to_view))
}

#[tauri::command]
async fn cloud_auth_register(
    state: State<'_, AppState>,
    request: RegisterRequest,
) -> CommandResult<CloudSessionView> {
    let session = state
        .cloud
        .register(request)
        .await
        .map_err(to_command_error)?;
    let session = state
        .set_cloud_session(session)
        .await
        .map_err(to_command_error)?;
    Ok(session_to_view(session))
}

#[tauri::command]
async fn cloud_auth_login(
    state: State<'_, AppState>,
    request: LoginRequest,
) -> CommandResult<CloudSessionView> {
    let session = state.cloud.login(request).await.map_err(to_command_error)?;
    let session = state
        .set_cloud_session(session)
        .await
        .map_err(to_command_error)?;
    Ok(session_to_view(session))
}

#[tauri::command]
async fn cloud_invitation_accept(
    state: State<'_, AppState>,
    token: String,
    email: String,
    password: String,
) -> CommandResult<CloudSessionView> {
    let session = state
        .cloud
        .accept_invitation(token, email, password)
        .await
        .map_err(to_command_error)?;
    let session = state
        .set_cloud_session(session)
        .await
        .map_err(to_command_error)?;
    Ok(session_to_view(session))
}

#[tauri::command]
fn open_url(app_handle: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    validate_external_url(&url)?;
    app_handle
        .opener()
        .open_url(url, None::<String>)
        .map_err(|e| e.to_string())
}

fn validate_external_url(url: &str) -> Result<(), String> {
    if url.len() > 4096 {
        return Err("URL is too long".to_string());
    }
    let parsed = Url::parse(url).map_err(|_| "invalid URL".to_string())?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("only credential-free HTTPS URLs may be opened".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod external_url_tests {
    use super::validate_external_url;

    #[test]
    fn external_urls_are_https_and_credential_free() {
        assert!(validate_external_url("https://accounts.example.com/oauth?state=abc").is_ok());
        assert!(validate_external_url("http://accounts.example.com/oauth").is_err());
        assert!(validate_external_url("file:///C:/Windows/System32/cmd.exe").is_err());
        assert!(validate_external_url("https://user:secret@example.com/oauth").is_err());
    }

    #[test]
    fn safe_workspace_path_containment() {
        let temp_dir = std::env::temp_dir().join("aro_test_ws");
        let _ = std::fs::create_dir_all(&temp_dir);
        let safe = super::resolve_safe_workspace_path(&temp_dir, "src/main.rs");
        assert!(safe.is_ok());
        let safe_p = safe.unwrap();
        assert!(safe_p.to_string_lossy().contains("src"));

        // Path traversal should be rejected
        let traversal = super::resolve_safe_workspace_path(&temp_dir, "../../../secret.txt");
        assert!(traversal.is_err());

        // Empty path should be rejected
        let empty = super::resolve_safe_workspace_path(&temp_dir, "   ");
        assert!(empty.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_apply_unified_patch_logic() {
        // Direct replacement without hunks
        assert!(super::apply_unified_patch("old", "new").is_err());

        // Unified diff with additions and modifications
        let original = "line 1\nline 2\nline 3";
        let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,2 @@\n-line 2\n+modified line 2\n+extra line\n";
        let patched = super::apply_unified_patch(original, patch).unwrap();
        assert!(patched.contains("line 1"));
        assert!(patched.contains("modified line 2"));
        assert!(patched.contains("extra line"));
        assert!(patched.contains("line 3"));
        assert!(!patched.contains("\nline 2\n"));
    }

    #[test]
    fn test_apply_unified_patch_malformed_headers() {
        for patch in [
            "@@ -1,2 +1,2\n-a\n+b\n",
            "@@ -invalid,range +foo,bar @@\n+x\n",
            "@@ @@\n+x\n",
        ] {
            assert!(super::apply_unified_patch("a\nb\n", patch).is_err());
        }
    }

    #[test]
    fn test_apply_unified_patch_mismatching_line_counts() {
        for patch in [
            "@@ -1,10 +1,20 @@\n-a\n+b\n",
            "@@ -999,1 +999,2 @@\n+a\n+b\n",
            "@@ -2,5 +2,1 @@\n-b\n-c\n-d\n",
        ] {
            assert!(super::apply_unified_patch("a\nb\nc\n", patch).is_err());
        }
    }

    #[test]
    fn test_apply_unified_patch_missing_file_headers() {
        let original = "alpha\nbeta\ngamma";
        // No "--- a/..." or "+++ b/..." header, starts directly with hunk
        let raw_hunk = "@@ -2,1 +2,1 @@\n-beta\n+BETA\n";
        let res = super::apply_unified_patch(original, raw_hunk).unwrap();
        assert_eq!(res, "alpha\nBETA\ngamma");
    }

    #[test]
    fn test_apply_unified_patch_empty_and_creation() {
        // Applying patch to empty file (creating file)
        let empty_orig = "";
        let creation_patch =
            "--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+first line\n+second line\n";
        let res = super::apply_unified_patch(empty_orig, creation_patch).unwrap();
        assert_eq!(res, "first line\nsecond line");
    }

    #[test]
    fn test_apply_unified_patch_multi_hunk_offset() {
        let original = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10";
        // Hunk 1 inserts 2 lines at line 2.
        // Hunk 2 modifies line 8.
        let patch =
            "@@ -2,1 +2,3 @@\n-2\n+2_mod\n+2_extra1\n+2_extra2\n@@ -8,1 +10,1 @@\n-8\n+8_mod\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        println!("Multi-hunk patched result:\n{}", res);
        assert!(res.contains("2_mod"), "Hunk 1 should modify line 2");
        assert!(res.contains("2_extra1"), "Hunk 1 should insert extra line");
        assert!(res.contains("8_mod"), "Hunk 2 should modify line 8");
        assert!(
            !res.contains("\n8\n") && !res.ends_with("\n8"),
            "Original line 8 should be replaced"
        );
        assert!(res.contains("6"), "Line 6 should remain intact");
    }

    #[test]
    fn test_safe_workspace_path_nested_non_existent_creation() {
        let temp_dir = std::env::temp_dir().join(format!("aro_test_nested_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        // Non-existent deeply nested path inside workspace should succeed
        let res = super::resolve_safe_workspace_path(&temp_dir, "deep/nested/sub/folder/file.txt");
        assert!(res.is_ok());
        let p = res.unwrap();
        assert!(p.to_string_lossy().contains("deep"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_safe_workspace_path_rejects_junction_escape() {
        let base_temp = std::env::temp_dir().join(format!("aro_test_junc_{}", std::process::id()));
        let ws_root = base_temp.join("ws");
        let outside_target = base_temp.join("outside");
        let _ = std::fs::create_dir_all(&ws_root);
        let _ = std::fs::create_dir_all(&outside_target);

        let link_path = ws_root.join("ext_junction");
        let status = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link_path.to_str().unwrap(),
                outside_target.to_str().unwrap(),
            ])
            .output();

        if let Ok(out) = status {
            if out.status.success() {
                // Test 1: Existing file through junction
                let outside_file = outside_target.join("secret.txt");
                let _ = std::fs::write(&outside_file, "secret");
                assert!(
                    super::resolve_safe_workspace_path(&ws_root, "ext_junction/secret.txt")
                        .is_err()
                );

                // Test 2: New file in junction root
                assert!(
                    super::resolve_safe_workspace_path(&ws_root, "ext_junction/new.txt").is_err()
                );

                // Test 3: New file in non-existent subdirectory inside junction
                assert!(super::resolve_safe_workspace_path(
                    &ws_root,
                    "ext_junction/non_existent_folder/file.txt"
                )
                .is_err());
                assert!(super::resolve_safe_workspace_path(
                    &ws_root,
                    "ext_junction/a/b/c/payload.exe"
                )
                .is_err());
            }
        }

        let _ = std::fs::remove_dir_all(&base_temp);
    }

    #[test]
    fn test_apply_unified_patch_multi_hunk_deletions() {
        let original =
            "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10";
        // Hunk 1 deletes lines 2, 3, 4 (net -3)
        // Hunk 2 replaces line 8
        let patch = "@@ -2,3 +2,0 @@\n-line 2\n-line 3\n-line 4\n@@ -8,1 +5,1 @@\n-line 8\n+line 8 modified\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert!(!res.contains("line 2"));
        assert!(!res.contains("line 3"));
        assert!(!res.contains("line 4"));
        assert!(res.contains("line 1"));
        assert!(res.contains("line 5"));
        assert!(res.contains("line 6"));
        assert!(res.contains("line 7"));
        assert!(res.contains("line 8 modified"));
        assert!(!res.contains("\nline 8\n"));
        assert!(res.contains("line 9"));
        assert!(res.contains("line 10"));
    }

    #[test]
    fn test_apply_unified_patch_crlf_preservation() {
        let original = "line 1\r\nline 2\r\nline 3\r\nline 4\r\n";
        let patch = "@@ -2,1 +2,2 @@\n-line 2\n+line 2 mod\n+line 2 extra\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert!(res.contains("\r\n"));
        assert!(
            !res.replace("\r\n", "").contains('\n'),
            "Should not contain lone LF newlines"
        );
        assert!(res.ends_with("\r\n"), "Trailing CRLF must be preserved");
        assert!(res.contains("line 2 mod\r\nline 2 extra\r\n"));
    }

    #[test]
    fn test_apply_unified_patch_empty_and_whitespace() {
        let original = "const x = 42;\n";
        assert_eq!(super::apply_unified_patch(original, "").unwrap(), original);
        assert_eq!(
            super::apply_unified_patch(original, "   \n\t\n").unwrap(),
            original
        );
    }

    #[test]
    fn test_apply_unified_patch_header_only_no_hunks() {
        let original = "const x = 42;\n";
        let header_only = "diff --git a/test.ts b/test.ts\n--- a/test.ts\n+++ b/test.ts\n";
        assert_eq!(
            super::apply_unified_patch(original, header_only).unwrap(),
            original
        );
    }

    #[test]
    fn test_apply_unified_patch_4hunk_interleaved_mixed_shifts() {
        let original = "line 1\nline 2\nline 3\n\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\nline 11\n\nline 13\nline 14\nline 15\n";
        // Hunk 1: delete line 2, line 3 (-2 lines)
        // Hunk 2: context line 5, insert 3 lines (+3 lines)
        // Hunk 3: replace line 8 with blank line and new line (+1 line)
        // Hunk 4: delete line 13, 14, 15 and add 1 replacement line (-2 lines)
        let patch = "\
@@ -2,2 +2,0 @@
-line 2
-line 3
@@ -5,1 +3,4 @@
 line 5
+ins 5a
+ins 5b
+ins 5c
@@ -8,1 +10,2 @@
-line 8
+
+line 8 modified
@@ -13,3 +16,1 @@
-line 13
-line 14
-line 15
+line 13-15 replaced
";
        let res = super::apply_unified_patch(original, patch).unwrap();
        let expected = "line 1\n\nline 5\nins 5a\nins 5b\nins 5c\nline 6\nline 7\n\nline 8 modified\nline 9\nline 10\nline 11\n\nline 13-15 replaced\n";
        assert_eq!(
            res, expected,
            "All 4 hunks with mixed shifts must produce exact expected line sequence"
        );
    }

    #[test]
    fn test_apply_unified_patch_adjacent_hunks() {
        let original = "A\nB\nC\nD\n";
        // Hunk 1 replaces A, B with A1, A2, A3 (+1 shift)
        // Hunk 2 immediately touches C, D replacing them with CD (-1 shift)
        let patch = "\
@@ -1,2 +1,3 @@
-A
-B
+A1
+A2
+A3
@@ -3,2 +4,1 @@
-C
-D
+CD
";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert_eq!(res, "A1\nA2\nA3\nCD\n");
    }

    #[test]
    fn test_apply_unified_patch_blank_context_lines_without_leading_space() {
        let original = "section 1\n\nsection 2\n\nsection 3\n";
        // Context line 2 is an empty string with no leading space
        let patch = "@@ -1,3 +1,3 @@\n section 1\n\n-section 2\n+section 2 modified\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert_eq!(res, "section 1\n\nsection 2 modified\n\nsection 3\n");
    }

    #[test]
    fn test_apply_unified_patch_blank_line_modification() {
        let original = "before\n\nafter\n";
        let patch = "@@ -1,3 +1,3 @@\n before\n-\n+inserted middle\n after\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert_eq!(res, "before\ninserted middle\nafter\n");
    }

    #[test]
    fn test_apply_unified_patch_multiple_deletions_to_reduction() {
        let original = "1\n2\n3\n4\n5\n";
        let patch = "@@ -1,2 +1,0 @@\n-1\n-2\n@@ -4,2 +2,0 @@\n-4\n-5\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert_eq!(res, "3\n");
    }

    #[test]
    fn test_apply_unified_patch_crlf_multi_hunk_stress() {
        let original = "line 1\r\nline 2\r\nline 3\r\nline 4\r\nline 5\r\n";
        let patch = "@@ -2,1 +2,2 @@\n-line 2\n+line 2 mod\n+line 2 extra\n@@ -4,1 +5,2 @@\n-line 4\n+line 4 mod\n+line 4 extra\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert_eq!(res, "line 1\r\nline 2 mod\r\nline 2 extra\r\nline 3\r\nline 4 mod\r\nline 4 extra\r\nline 5\r\n");
        assert!(
            !res.replace("\r\n", "").contains('\n'),
            "No rogue bare LFs permitted"
        );
    }

    #[test]
    fn test_apply_unified_patch_various_header_only_formats() {
        let original = "fn main() {}\n";
        let h1 = "--- a/main.rs\n+++ b/main.rs\n";
        let h2 =
            "diff --git a/main.rs b/main.rs\nindex abc..def 100644\n--- a/main.rs\n+++ b/main.rs\n";
        let h3 = "+++ b/main.rs\n";
        assert_eq!(super::apply_unified_patch(original, h1).unwrap(), original);
        assert_eq!(super::apply_unified_patch(original, h2).unwrap(), original);
        assert_eq!(super::apply_unified_patch(original, h3).unwrap(), original);
    }

    #[test]
    fn test_apply_unified_patch_commit_preamble_before_hunks() {
        let original = "let a = 1;\nlet b = 2;\n";
        let patch = "commit abcdef1234567890\nAuthor: Dev <dev@example.com>\nDate: today\n\n    Message\n\n--- a/file.rs\n+++ b/file.rs\n@@ -1,2 +1,2 @@\n-let a = 1;\n+let a = 100;\n let b = 2;\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert_eq!(res, "let a = 100;\nlet b = 2;\n");
    }

    #[test]
    fn test_apply_unified_patch_no_trailing_newline() {
        let original = "single_line";
        let patch = "@@ -1,1 +1,2 @@\n-single_line\n+single_line_mod\n+new_line\n";
        let res = super::apply_unified_patch(original, patch).unwrap();
        assert_eq!(res, "single_line_mod\nnew_line");
    }

    #[test]
    fn test_apply_unified_patch_raw_content_replacement() {
        let original = "old file content";
        let new_code = "export const x = 999;\n";
        assert!(super::apply_unified_patch(original, new_code).is_err());
    }
}

#[tauri::command]
async fn integration_connect_start(
    state: State<'_, AppState>,
    provider_id: String,
    redirect_uri: Option<String>,
    scopes: Vec<String>,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_connect_start(&session.access_token, &provider_id, redirect_uri, scopes)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn integration_connect_status(
    state: State<'_, AppState>,
    oauth_state: String,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_connect_status(&session.access_token, &oauth_state)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn integration_api_key_connect(
    state: State<'_, AppState>,
    provider_id: String,
    secret: String,
    public_config: serde_json::Value,
    account: serde_json::Value,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_api_key_create(
            &session.access_token,
            &provider_id,
            secret,
            public_config,
            account,
        )
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn integration_catalog_v3_list(
    state: State<'_, AppState>,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_catalog_v3(&session.access_token)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn integration_installations_v3_list(
    state: State<'_, AppState>,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_installations_v3(&session.access_token)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn integration_credential_v3_create(
    state: State<'_, AppState>,
    provider_id: String,
    idempotency_key: String,
    request: serde_json::Value,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_credential_v3_create(
            &session.access_token,
            &provider_id,
            &idempotency_key,
            request,
        )
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn integration_authorization_v3_create(
    state: State<'_, AppState>,
    provider_id: String,
    idempotency_key: String,
    request: serde_json::Value,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_authorization_v3_create(
            &session.access_token,
            &provider_id,
            &idempotency_key,
            request,
        )
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn integration_authorization_v3_get(
    state: State<'_, AppState>,
    attempt_id: String,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_authorization_v3_get(&session.access_token, &attempt_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn integration_installation_v3_action(
    state: State<'_, AppState>,
    installation_id: String,
    action: String,
) -> CommandResult<serde_json::Value> {
    if !matches!(action.as_str(), "health-check" | "disconnect") {
        return Err("unsupported integration action".to_string());
    }
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .integration_installation_v3_action(&session.access_token, &installation_id, &action)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_auth_logout(state: State<'_, AppState>) -> CommandResult<()> {
    state.logout_cloud_session().await.map_err(to_command_error)
}

#[tauri::command]
async fn cloud_organizations_list(state: State<'_, AppState>) -> CommandResult<Vec<Organization>> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .list_organizations(&session.access_token)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_organization_create(
    state: State<'_, AppState>,
    request: OrganizationCreateRequest,
) -> CommandResult<CloudSessionView> {
    let switched = state
        .create_and_switch_cloud_organization(&request)
        .await
        .map_err(to_command_error)?;
    Ok(session_to_view(switched))
}

#[tauri::command]
async fn cloud_organization_switch(
    state: State<'_, AppState>,
    organization_id: String,
) -> CommandResult<CloudSessionView> {
    let switched = state
        .switch_cloud_organization(&organization_id)
        .await
        .map_err(to_command_error)?;
    Ok(session_to_view(switched))
}

#[tauri::command]
async fn cloud_state_set(
    state: State<'_, AppState>,
    key: String,
    value: serde_json::Value,
) -> CommandResult<()> {
    let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    else {
        return Ok(());
    };
    state
        .cloud
        .set_client_state(&session.access_token, &key, value)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_state_delete(state: State<'_, AppState>, key: String) -> CommandResult<()> {
    let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    else {
        return Ok(());
    };
    state
        .cloud
        .delete_client_state(&session.access_token, &key)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_collection_get(
    state: State<'_, AppState>,
    collection: String,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .get_collection(&session.access_token, &collection)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_collection_create(
    state: State<'_, AppState>,
    collection: String,
    payload: serde_json::Value,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .create_collection_item(&session.access_token, &collection, payload)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_collection_update(
    state: State<'_, AppState>,
    collection: String,
    item_id: String,
    payload: serde_json::Value,
) -> CommandResult<serde_json::Value> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .update_collection_item(&session.access_token, &collection, &item_id, payload)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_collection_delete(
    state: State<'_, AppState>,
    collection: String,
    item_id: String,
) -> CommandResult<()> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .delete_collection_item(&session.access_token, &collection, &item_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_user_profile_update(
    state: State<'_, AppState>,
    request: UserProfilePatch,
) -> CommandResult<User> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .update_user_profile(&session.access_token, &request)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_organization_update(
    state: State<'_, AppState>,
    organization_id: String,
    request: OrganizationPatch,
) -> CommandResult<Organization> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .update_organization(&session.access_token, &organization_id, &request)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_api_keys_list(state: State<'_, AppState>) -> CommandResult<Vec<PublicApiKey>> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .list_api_keys(&session.access_token)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_api_key_create(
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<ApiKeyCreateView> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    let response = state
        .cloud
        .create_api_key(&session.access_token, name)
        .await
        .map_err(to_command_error)?;
    Ok(ApiKeyCreateView {
        key: response.key,
        secret: response.secret,
    })
}

#[tauri::command]
async fn cloud_api_key_revoke(state: State<'_, AppState>, api_key_id: String) -> CommandResult<()> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .revoke_api_key(&session.access_token, &api_key_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_members_list(state: State<'_, AppState>) -> CommandResult<Vec<OrganizationMember>> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .list_members(&session.access_token)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_invitations_list(
    state: State<'_, AppState>,
    cursor: Option<String>,
    limit: i64,
    include_closed: bool,
) -> CommandResult<OrganizationInvitationPage> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .list_invitations(
            &session.access_token,
            cursor.as_deref(),
            limit,
            include_closed,
        )
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_invitation_revoke(
    state: State<'_, AppState>,
    invitation_id: String,
) -> CommandResult<()> {
    let invitation_id = Uuid::parse_str(&invitation_id)
        .map_err(|_| "invalid invitation id".to_string())?
        .to_string();
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .revoke_invitation(&session.access_token, &invitation_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_member_invite(
    state: State<'_, AppState>,
    request: MembershipCreateRequest,
) -> CommandResult<OrganizationInvitationReceipt> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .invite_member(&session.access_token, &request)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_member_role_update(
    state: State<'_, AppState>,
    membership_id: String,
    role: MembershipRole,
) -> CommandResult<OrganizationMember> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .update_member_role(&session.access_token, &membership_id, role)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn cloud_member_remove(
    state: State<'_, AppState>,
    membership_id: String,
) -> CommandResult<()> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .remove_member(&session.access_token, &membership_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn settings_get(state: State<'_, AppState>) -> CommandResult<AppSettings> {
    Ok(settings_with_auth_status(state.settings().await))
}

#[tauri::command]
async fn settings_update(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> CommandResult<AppSettings> {
    let mut settings = settings;
    if let Some(api_key) = settings.search.api_key.take() {
        if !api_key.trim().is_empty() {
            let keyring_id = format!("search-{}", settings.search.provider);
            save_provider_api_key(&keyring_id, &api_key).map_err(to_command_error)?;
            settings.search.auth_configured = true;
        }
    }
    settings.model = settings.model.normalized();
    validate_provider_endpoints(&settings.model).map_err(to_command_error)?;
    sanitize_provider_auth_status(&mut settings.model.providers);
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let updated = state
            .cloud
            .update_settings(&session.access_token, &settings)
            .await
            .map_err(to_command_error)?;
        state
            .replace_settings(updated.clone())
            .await
            .map_err(to_command_error)?;
        return Ok(settings_with_auth_status(state.settings().await));
    }
    state
        .replace_settings(settings.clone())
        .await
        .map_err(to_command_error)?;
    Ok(settings_with_auth_status(state.settings().await))
}

#[tauri::command]
async fn search_provider_set_api_key(
    state: State<'_, AppState>,
    provider_id: String,
    api_key: String,
) -> CommandResult<AppSettings> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("search provider API key is required".to_string());
    }
    let mut settings = state.settings().await;
    if settings.search.provider != provider_id {
        return Err("search provider does not match active settings".to_string());
    }
    save_provider_api_key(&format!("search-{provider_id}"), api_key).map_err(to_command_error)?;
    settings.search.api_key = None;
    settings.search.auth_configured = true;
    settings_update(state, settings).await
}

#[tauri::command]
async fn search_provider_clear_api_key(
    state: State<'_, AppState>,
    provider_id: String,
) -> CommandResult<AppSettings> {
    clear_provider_api_key(&format!("search-{provider_id}")).map_err(to_command_error)?;
    let mut settings = state.settings().await;
    settings.search.api_key = None;
    settings.search.auth_configured = false;
    settings_update(state, settings).await
}

#[tauri::command]
async fn preferences_update(
    state: State<'_, AppState>,
    preferences: UserPreferencesPatch,
) -> CommandResult<UserPreferences> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .update_preferences(&session.access_token, &preferences)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn runtime_check(state: State<'_, AppState>) -> CommandResult<RuntimeStatus> {
    let settings = state.settings().await;
    let mut runtime =
        match ModelRouter::from_active(&settings.model, active_provider_api_key(&settings)?) {
            Ok(provider) => state.engine.check_runtime(&provider).await,
            Err(err) => runtime_unavailable_from_settings(&settings, err),
        };
    let voice_status = voice_status_for_settings(settings.voice).await?;
    runtime.set_voice_status(&voice_status);
    Ok(runtime)
}

#[tauri::command]
async fn voice_status(state: State<'_, AppState>) -> CommandResult<VoiceStatus> {
    let settings = state.settings().await;
    voice_status_for_settings(settings.voice).await
}

#[tauri::command]
async fn voice_model_status(state: State<'_, AppState>) -> CommandResult<VoiceModelsStatus> {
    let settings = state.settings().await;
    voice_model_status_for_settings(settings.voice).await
}

#[tauri::command]
async fn model_list(state: State<'_, AppState>) -> CommandResult<Vec<ModelRef>> {
    let settings = state.settings().await;
    Ok(model_options_for_settings(&settings).await)
}

#[tauri::command]
async fn model_provider_list(
    state: State<'_, AppState>,
) -> CommandResult<Vec<ModelProviderConnection>> {
    Ok(with_auth_status(state.settings().await.model.providers))
}

#[tauri::command]
async fn model_provider_upsert(
    state: State<'_, AppState>,
    provider: ModelProviderConnection,
) -> CommandResult<AppSettings> {
    let mut settings = state.settings().await;
    let mut provider = provider;
    provider.auth_configured = false;
    settings.model.upsert_provider(provider);
    settings_update(state, settings).await
}

#[tauri::command]
async fn model_provider_delete(
    state: State<'_, AppState>,
    provider_id: String,
) -> CommandResult<AppSettings> {
    let mut settings = state.settings().await;
    let provider = settings
        .model
        .connection(&provider_id)
        .cloned()
        .ok_or_else(|| "model provider not found".to_string())?;
    if provider.kind.is_local() {
        return Err("local providers cannot be deleted".to_string());
    }
    clear_provider_api_key(&provider_id).map_err(to_command_error)?;
    settings
        .model
        .providers
        .retain(|provider| provider.id != provider_id);
    settings.model = settings.model.normalized();
    settings_update(state, settings).await
}

#[tauri::command]
async fn model_provider_set_api_key(
    state: State<'_, AppState>,
    request: ModelProviderSecretRequest,
) -> CommandResult<AppSettings> {
    let api_key = request.api_key.trim();
    if api_key.is_empty() {
        return Err("provider API key is required".to_string());
    }
    let mut settings = state.settings().await;
    if settings.model.connection(&request.provider_id).is_none() {
        return Err("model provider not found".to_string());
    }
    save_provider_api_key(&request.provider_id, api_key).map_err(to_command_error)?;
    if let Some(provider) = settings
        .model
        .providers
        .iter_mut()
        .find(|provider| provider.id == request.provider_id)
    {
        provider.auth_configured = true;
        provider.updated_at = chrono::Utc::now();
    }
    settings_update(state, settings).await
}

#[tauri::command]
async fn model_provider_clear_api_key(
    state: State<'_, AppState>,
    provider_id: String,
) -> CommandResult<AppSettings> {
    clear_provider_api_key(&provider_id).map_err(to_command_error)?;
    let mut settings = state.settings().await;
    if let Some(provider) = settings
        .model
        .providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
    {
        provider.auth_configured = false;
        provider.updated_at = chrono::Utc::now();
    }
    settings_update(state, settings).await
}

#[tauri::command]
async fn model_provider_test(
    state: State<'_, AppState>,
    provider_id: String,
) -> CommandResult<RuntimeStatus> {
    let settings = state.settings().await;
    let connection = settings
        .model
        .connection(&provider_id)
        .cloned()
        .ok_or_else(|| "model provider not found".to_string())?;
    let model = connection.models.first().cloned().unwrap_or_else(|| {
        ModelRef::new(
            connection.id.clone(),
            connection.kind.clone(),
            connection.kind.default_model_id(),
            connection.kind.default_model_id(),
        )
    });
    let mut runtime =
        match ModelRouter::from_model_ref(&settings.model, &model, provider_api_key(&provider_id)?)
        {
            Ok(provider) => state.engine.check_runtime(&provider).await,
            Err(err) => RuntimeStatus::unavailable(
                connection.kind.clone(),
                model.model_id.clone(),
                connection.endpoint.clone(),
                err.to_string(),
            ),
        };
    let voice_status = voice_status_for_settings(settings.voice).await?;
    runtime.set_voice_status(&voice_status);
    Ok(runtime)
}

#[tauri::command]
async fn model_catalog_refresh(
    state: State<'_, AppState>,
    provider_id: String,
) -> CommandResult<AppSettings> {
    let mut settings = state.settings().await;
    let Some(connection) = settings.model.connection(&provider_id).cloned() else {
        return Err("model provider not found".to_string());
    };
    let api_key = provider_api_key(&provider_id)?;
    let models = list_models_for_connection(&settings.model, &connection, api_key.as_deref())
        .await
        .map_err(to_command_error)?;
    if let Some(provider) = settings
        .model
        .providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
    {
        provider.models = models;
        provider.auth_configured = api_key.is_some() || !provider.kind.requires_api_key();
        provider.updated_at = chrono::Utc::now();
    }
    settings_update(state, settings).await
}

#[tauri::command]
async fn model_select(
    state: State<'_, AppState>,
    request: ModelSelectRequest,
) -> CommandResult<AppSettings> {
    let mut settings = state.settings().await;
    if !settings
        .model
        .select_model(&request.provider_id, &request.model_id)
    {
        return Err("model provider not found".to_string());
    }
    settings_update(state, settings).await
}

#[tauri::command]
async fn conversation_create(
    state: State<'_, AppState>,
    request: CreateConversationRequest,
) -> CommandResult<Conversation> {
    let project_id = request
        .project_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| Uuid::parse_str(s).ok());
    let folder_id = request
        .folder_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| Uuid::parse_str(s).ok());
    let mut conversation = state
        .engine
        .create_conversation(
            request
                .title
                .unwrap_or_else(|| "New conversation".to_string()),
            request.mode,
            project_id,
            folder_id,
        )
        .map_err(to_command_error)?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        match state
            .cloud
            .create_conversation(
                &session.access_token,
                conversation.title.clone(),
                conversation.mode.clone(),
                project_id.map(|id| id.to_string()),
                folder_id.map(|id| id.to_string()),
            )
            .await
        {
            Ok(cloud_conversation) => {
                let _ = state.engine.delete_conversation(conversation.id);
                if state
                    .engine
                    .upsert_conversation(&cloud_conversation)
                    .is_ok()
                {
                    conversation = cloud_conversation;
                    // Le cloud peut ignorer le placement : on le ré-applique en local.
                    if (project_id.is_some() || folder_id.is_some())
                        && (conversation.project_id.is_none() && conversation.folder_id.is_none())
                    {
                        if let Ok(moved) =
                            state
                                .engine
                                .move_conversation(conversation.id, project_id, folder_id)
                        {
                            conversation = moved;
                        }
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "Cloud conversation creation failed or was rate-limited; keeping local conversation"
                );
            }
        }
    }
    Ok(conversation)
}

/// Fusionne une conversation cloud avec la version locale : le placement
/// local (projet/dossier/root) n'est jamais écrasé par un cloud qui
/// l'ignore. Le desktop gagne en cas de conflit.
/// Retourne (conversation_fusionnée, push_back_requis).
fn merge_cloud_conversation(state: &AppState, cloud_conv: &Conversation) -> (Conversation, bool) {
    let merged = match state.engine.get_conversation(cloud_conv.id) {
        Ok(Some(local)) => {
            let mut merged = cloud_conv.clone();
            if local.project_id.is_some() || cloud_conv.project_id.is_none() {
                merged.project_id = local.project_id;
            }
            if local.folder_id.is_some() || cloud_conv.folder_id.is_none() {
                merged.folder_id = local.folder_id;
            }
            if local.root_path.is_some() || cloud_conv.root_path.is_none() {
                merged.root_path = local.root_path.clone();
            }
            if merged.title.is_empty() {
                merged.title = local.title.clone();
            }
            merged
        }
        _ => cloud_conv.clone(),
    };
    let needs_push =
        merged.project_id != cloud_conv.project_id || merged.folder_id != cloud_conv.folder_id;
    (merged, needs_push)
}

#[tauri::command]
async fn conversation_list(state: State<'_, AppState>) -> CommandResult<Vec<Conversation>> {
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if let Ok(cloud_conversations) = state.cloud.list_conversations(&session.access_token).await
        {
            for cloud_conv in &cloud_conversations {
                let (merged, needs_push) = merge_cloud_conversation(&state, cloud_conv);
                let _ = state.engine.upsert_conversation(&merged);
                if needs_push {
                    let _ = state
                        .cloud
                        .move_conversation(
                            &session.access_token,
                            &merged.id.to_string(),
                            merged.project_id.map(|id| id.to_string()),
                            merged.folder_id.map(|id| id.to_string()),
                        )
                        .await;
                }
            }
            // Le local est la source de vérité affichée : il contient aussi
            // les conversations jamais poussées vers le cloud.
            return state.engine.conversations().map_err(to_command_error);
        }
    }
    state.engine.conversations().map_err(to_command_error)
}

#[tauri::command]
async fn conversation_delete(
    state: State<'_, AppState>,
    conversation_id: String,
) -> CommandResult<()> {
    let id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let _ = state
            .cloud
            .delete_conversation(&session.access_token, &conversation_id)
            .await;
    }
    state
        .engine
        .delete_conversation(id)
        .map_err(to_command_error)
}

#[tauri::command]
async fn conversations_delete_empty(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    state
        .engine
        .delete_empty_conversations()
        .map(|ids| ids.into_iter().map(|id| id.to_string()).collect())
        .map_err(to_command_error)
}

#[tauri::command]
async fn conversation_export(
    state: State<'_, AppState>,
    conversation_id: String,
    format: Option<String>,
) -> CommandResult<Vec<u8>> {
    let id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    let conversation = state
        .engine
        .get_conversation(id)
        .map_err(to_command_error)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    let messages = state.engine.messages(id).map_err(to_command_error)?;
    let bytes = if format.as_deref() == Some("json") {
        serde_json::to_vec_pretty(&serde_json::json!({
            "conversation": conversation,
            "messages": messages,
        }))
        .map_err(to_command_error)?
    } else {
        let mut md = format!("# {}\n\n", conversation.title);
        for msg in &messages {
            let role = match msg.role {
                aro_core::MessageRole::User => "**User**",
                aro_core::MessageRole::Assistant => "**Assistant**",
                aro_core::MessageRole::System => "**System**",
            };
            md.push_str(&format!("### {}\n\n{}\n\n", role, msg.content));
        }
        md.into_bytes()
    };
    Ok(bytes)
}

#[tauri::command]
async fn conversation_update_title(
    state: State<'_, AppState>,
    conversation_id: String,
    title: String,
) -> CommandResult<Conversation> {
    let id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    let conversation = state
        .engine
        .update_conversation_title(id, title)
        .map_err(to_command_error)?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let _ = state
            .cloud
            .update_conversation_title(
                &session.access_token,
                &conversation_id,
                conversation.title.clone(),
            )
            .await;
    }
    Ok(conversation)
}

#[tauri::command]
async fn project_list(state: State<'_, AppState>) -> CommandResult<Vec<Project>> {
    // Maintenance : fusionne les doublons stricts (anciennes boucles de sync)
    // avant toute synchronisation, pour ne jamais les ré-afficher.
    let mut removed_ids = match state.engine.deduplicate_projects() {
        Ok(ids) => ids,
        Err(err) => {
            tracing::warn!(error = %err, "project deduplication failed; continuing without cleanup");
            Vec::new()
        }
    };
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        // Purge côté cloud les doublons supprimés en local : sinon le
        // prochain list les re-téléchargerait et les recréerait.
        for gone in removed_ids.drain(..) {
            let _ = state
                .cloud
                .delete_project(&session.access_token, &gone.to_string())
                .await;
        }
        if let Ok(cloud_projects) = state.cloud.list_projects(&session.access_token).await {
            let active_org_id = session.active_organization.id.to_string();
            for mut proj in cloud_projects.clone() {
                proj.organization_id = Some(active_org_id.clone());
                let _ = state.engine.save_project(&proj);
            }
            if let Ok(local_projects) = state.engine.projects() {
                for local in &local_projects {
                    if local.organization_id.as_deref() == Some(&active_org_id)
                        && !cloud_projects.iter().any(|c| c.id == local.id)
                    {
                        let _ = state
                            .cloud
                            .create_project(&session.access_token, local)
                            .await;
                    }
                }
            }
            return match state.engine.projects() {
                Ok(local) => Ok(local),
                Err(err) => {
                    tracing::warn!(error = %err, "local project list failed; showing cloud list");
                    let tagged_cloud = cloud_projects
                        .into_iter()
                        .map(|mut p| {
                            p.organization_id = Some(active_org_id.clone());
                            p
                        })
                        .collect();
                    Ok(tagged_cloud)
                }
            };
        }
    }
    state.engine.projects().map_err(to_command_error)
}

#[tauri::command]
async fn project_create(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
    instructions: Option<String>,
    root_path: Option<String>,
    color: Option<String>,
    icon: Option<String>,
    organization_id: Option<String>,
) -> CommandResult<Project> {
    let now = chrono::Utc::now();
    let project = Project {
        id: Uuid::new_v4(),
        name,
        description,
        instructions,
        root_path,
        color: color.unwrap_or_else(|| "#3b82f6".into()),
        icon: icon.unwrap_or_else(|| "folder-tree".into()),
        created_at: now,
        updated_at: now,
        organization_id,
    };
    state
        .engine
        .save_project(&project)
        .map_err(to_command_error)?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if project.organization_id.as_deref() == Some(&session.active_organization.id.to_string()) {
            let _ = state
                .cloud
                .create_project(&session.access_token, &project)
                .await;
        }
    }
    Ok(project)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn project_update(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    description: Option<String>,
    instructions: Option<String>,
    root_path: Option<String>,
    color: Option<String>,
    icon: Option<String>,
) -> CommandResult<Project> {
    let proj_id = Uuid::parse_str(&id).map_err(|err| err.to_string())?;
    let mut projects = state.engine.projects().map_err(to_command_error)?;
    let project = projects
        .iter_mut()
        .find(|p| p.id == proj_id)
        .ok_or_else(|| "Project not found".to_string())?;
    if let Some(n) = name {
        project.name = n;
    }
    if let Some(d) = description {
        project.description = Some(d);
    }
    if let Some(inst) = instructions {
        project.instructions = Some(inst);
    }
    if let Some(rp) = root_path {
        project.root_path = if rp.trim().is_empty() { None } else { Some(rp) };
    }
    if let Some(c) = color {
        project.color = c;
    }
    if let Some(i) = icon {
        project.icon = i;
    }
    project.updated_at = chrono::Utc::now();
    let updated = project.clone();
    state
        .engine
        .save_project(&updated)
        .map_err(to_command_error)?;
    Ok(updated)
}

#[tauri::command]
async fn project_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let proj_id = Uuid::parse_str(&id).map_err(|err| err.to_string())?;
    let res = state
        .engine
        .delete_project(proj_id)
        .map_err(to_command_error);
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let _ = state.cloud.delete_project(&session.access_token, &id).await;
    }
    res
}

#[tauri::command]
async fn conversation_search_content(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> CommandResult<Vec<ChatMessage>> {
    state
        .engine
        .search_messages(&query, limit.unwrap_or(20))
        .map_err(to_command_error)
}

#[tauri::command]
async fn folder_list(state: State<'_, AppState>) -> CommandResult<Vec<Folder>> {
    let mut removed_ids = match state.engine.deduplicate_folders() {
        Ok(ids) => ids,
        Err(err) => {
            tracing::warn!(error = %err, "folder deduplication failed; continuing without cleanup");
            Vec::new()
        }
    };
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        for gone in removed_ids.drain(..) {
            let _ = state
                .cloud
                .delete_folder(&session.access_token, &gone.to_string())
                .await;
        }
        if let Ok(cloud_folders) = state.cloud.list_folders(&session.access_token).await {
            let active_org_id = session.active_organization.id.to_string();
            for mut fold in cloud_folders.clone() {
                fold.organization_id = Some(active_org_id.clone());
                let _ = state.engine.save_folder(&fold);
            }
            if let Ok(local_folders) = state.engine.folders() {
                for local in &local_folders {
                    if local.organization_id.as_deref() == Some(&active_org_id)
                        && !cloud_folders.iter().any(|c| c.id == local.id)
                    {
                        let _ = state
                            .cloud
                            .create_folder(&session.access_token, local)
                            .await;
                    }
                }
            }
            return match state.engine.folders() {
                Ok(local) => Ok(local),
                Err(err) => {
                    tracing::warn!(error = %err, "local folder list failed; showing cloud list");
                    let tagged_cloud = cloud_folders
                        .into_iter()
                        .map(|mut f| {
                            f.organization_id = Some(active_org_id.clone());
                            f
                        })
                        .collect();
                    Ok(tagged_cloud)
                }
            };
        }
    }
    state.engine.folders().map_err(to_command_error)
}

#[tauri::command]
async fn folder_create(
    state: State<'_, AppState>,
    name: String,
    project_id: Option<String>,
    root_path: Option<String>,
    color: Option<String>,
    icon: Option<String>,
    organization_id: Option<String>,
) -> CommandResult<Folder> {
    let now = chrono::Utc::now();
    let proj_uuid = match project_id {
        Some(s) if !s.is_empty() => Some(Uuid::parse_str(&s).map_err(|err| err.to_string())?),
        _ => None,
    };
    let folder = Folder {
        id: Uuid::new_v4(),
        project_id: proj_uuid,
        name,
        root_path,
        color,
        icon: icon.unwrap_or_else(|| "folder".into()),
        created_at: now,
        updated_at: now,
        organization_id,
    };
    state
        .engine
        .save_folder(&folder)
        .map_err(to_command_error)?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if folder.organization_id.as_deref() == Some(&session.active_organization.id.to_string()) {
            let _ = state
                .cloud
                .create_folder(&session.access_token, &folder)
                .await;
        }
    }
    Ok(folder)
}

#[tauri::command]
async fn folder_update(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    project_id: Option<String>,
    root_path: Option<String>,
    color: Option<String>,
    icon: Option<String>,
) -> CommandResult<Folder> {
    let fold_id = Uuid::parse_str(&id).map_err(|err| err.to_string())?;
    let mut folders = state.engine.folders().map_err(to_command_error)?;
    let folder = folders
        .iter_mut()
        .find(|f| f.id == fold_id)
        .ok_or_else(|| "Folder not found".to_string())?;
    if let Some(n) = name {
        folder.name = n;
    }
    if let Some(pid) = project_id {
        folder.project_id = if pid.is_empty() {
            None
        } else {
            Uuid::parse_str(&pid).ok()
        };
    }
    if let Some(rp) = root_path {
        folder.root_path = if rp.trim().is_empty() { None } else { Some(rp) };
    }
    if let Some(c) = color {
        folder.color = Some(c);
    }
    if let Some(i) = icon {
        folder.icon = i;
    }
    folder.updated_at = chrono::Utc::now();
    let updated = folder.clone();
    state
        .engine
        .save_folder(&updated)
        .map_err(to_command_error)?;
    Ok(updated)
}

#[tauri::command]
async fn folder_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let fold_id = Uuid::parse_str(&id).map_err(|err| err.to_string())?;
    let res = state
        .engine
        .delete_folder(fold_id)
        .map_err(to_command_error);
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let _ = state.cloud.delete_folder(&session.access_token, &id).await;
    }
    res
}

#[tauri::command]
async fn conversation_move(
    state: State<'_, AppState>,
    conversation_id: String,
    project_id: Option<String>,
    folder_id: Option<String>,
) -> CommandResult<Conversation> {
    let conv_id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    let proj_uuid = match project_id {
        Some(s) if !s.is_empty() => Some(Uuid::parse_str(&s).map_err(|err| err.to_string())?),
        _ => None,
    };
    let fold_uuid = match folder_id {
        Some(s) if !s.is_empty() => Some(Uuid::parse_str(&s).map_err(|err| err.to_string())?),
        _ => None,
    };
    let moved = state
        .engine
        .move_conversation(conv_id, proj_uuid, fold_uuid)
        .map_err(to_command_error)?;
    // Le classement doit survivre au redémarrage : on le pousse vers le
    // cloud, sinon le prochain conversation_list l'écraserait.
    if let Ok(Some(session)) = state.refresh_cloud_session_from_keyring().await {
        let _ = state
            .cloud
            .move_conversation(
                &session.access_token,
                &conversation_id,
                proj_uuid.map(|id| id.to_string()),
                fold_uuid.map(|id| id.to_string()),
            )
            .await;
    }
    Ok(moved)
}

#[tauri::command]
async fn conversation_set_root_path(
    state: State<'_, AppState>,
    conversation_id: String,
    root_path: Option<String>,
) -> CommandResult<Conversation> {
    let conv_id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    let cleaned = root_path.and_then(|r| if r.trim().is_empty() { None } else { Some(r) });
    state
        .engine
        .set_conversation_root_path(conv_id, cleaned)
        .map_err(to_command_error)
}

#[tauri::command]
async fn conversation_effective_root_path(
    state: State<'_, AppState>,
    conversation_id: String,
) -> CommandResult<Option<String>> {
    let conv_id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    state
        .engine
        .resolve_effective_root_path(conv_id)
        .map_err(to_command_error)
}

#[tauri::command]
async fn workspace_tree_get(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    path: Option<String>,
) -> CommandResult<serde_json::Value> {
    let target_path = path.filter(|p| !p.trim().is_empty());

    let effective_path = match target_path {
        Some(p) => p,
        None => {
            if let Some(c_id) = conversation_id {
                if let Ok(conv_uuid) = Uuid::parse_str(&c_id) {
                    state
                        .engine
                        .resolve_effective_root_path(conv_uuid)
                        .unwrap_or(None)
                        .unwrap_or_else(|| ".".into())
                } else {
                    ".".into()
                }
            } else {
                ".".into()
            }
        }
    };

    // On tente d'abord le chemin tel quel, puis sa forme canonique
    // (noms courts 8.3, jonctions) : un dossier existant ne doit jamais
    // être rapporté comme "vide" à cause d'une simple forme de chemin.
    let raw_dir = std::path::PathBuf::from(&effective_path);
    let canonical_dir = std::fs::canonicalize(&raw_dir)
        .map(|p| strip_verbatim_prefix(&p))
        .ok();
    let probe_dir = canonical_dir
        .as_ref()
        .filter(|p| p.is_dir())
        .unwrap_or(&raw_dir);

    let mut entries_out = Vec::new();
    fn read_dir_tree(dir: &std::path::Path, entries: &mut Vec<serde_json::Value>, depth: usize) {
        if depth > 4 || entries.len() >= 300 {
            return;
        }
        if let Ok(dir_entries) = std::fs::read_dir(dir) {
            for entry in dir_entries.flatten() {
                let entry_path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                let is_dir = entry_path.is_dir();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                entries.push(serde_json::json!({
                    "name": name,
                    "path": entry_path.to_string_lossy(),
                    "isDir": is_dir,
                    "size": size
                }));

                if is_dir {
                    read_dir_tree(&entry_path, entries, depth + 1);
                }
            }
        }
    }

    read_dir_tree(probe_dir, &mut entries_out, 0);

    // Ne jamais confondre "illisible" et "vide" : si la lecture échoue, on
    // le signale explicitement pour que l'UI propose "Réessayer" au lieu
    // d'afficher "Dossier vide".
    let read_error = match std::fs::read_dir(probe_dir) {
        Ok(_) => None,
        Err(err) => Some(format!("Cannot read directory '{effective_path}': {err}")),
    };

    Ok(serde_json::json!({
        "rootPath": effective_path,
        "entries": entries_out,
        "count": entries_out.len(),
        "readError": read_error
    }))
}

fn strip_verbatim_prefix(path: &std::path::Path) -> std::path::PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        std::path::PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn resolve_effective_root_path(
    state: &AppState,
    conversation_id: Option<String>,
) -> std::path::PathBuf {
    let raw = if let Some(c_id) = conversation_id {
        if let Ok(conv_uuid) = Uuid::parse_str(&c_id) {
            state
                .engine
                .resolve_effective_root_path(conv_uuid)
                .ok()
                .flatten()
                .unwrap_or_else(|| ".".into())
        } else {
            ".".into()
        }
    } else {
        ".".into()
    };

    let p = std::path::PathBuf::from(raw);
    if let Ok(canon) = std::fs::canonicalize(&p) {
        strip_verbatim_prefix(&canon)
    } else if let Ok(cwd) = std::env::current_dir() {
        if p == std::path::Path::new(".") || p.as_os_str().is_empty() {
            strip_verbatim_prefix(&cwd)
        } else {
            strip_verbatim_prefix(&cwd.join(p))
        }
    } else {
        strip_verbatim_prefix(&p)
    }
}

fn resolve_safe_workspace_path(
    root: &std::path::Path,
    relative_or_absolute: &str,
) -> Result<std::path::PathBuf, String> {
    let raw_path = relative_or_absolute.trim();
    if raw_path.is_empty() {
        return Err("File path cannot be empty".to_string());
    }

    let cand_path = std::path::Path::new(raw_path);
    let combined = if cand_path.is_absolute() {
        cand_path.to_path_buf()
    } else {
        root.join(cand_path)
    };

    // Normalize path components to prevent .. traversal escaping root
    let mut normalized = std::path::PathBuf::new();
    for comp in combined.components() {
        match comp {
            std::path::Component::Prefix(_p) => normalized.push(comp.as_os_str()),
            std::path::Component::RootDir => normalized.push(comp.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "Access denied: Path traversal detected outside workspace root: {}",
                        raw_path
                    ));
                }
            }
            std::path::Component::Normal(c) => normalized.push(c),
        }
    }

    let clean_root = strip_verbatim_prefix(root);
    let clean_normalized = strip_verbatim_prefix(&normalized);

    // Resolve canonical root for comparing canonicalized paths (handles Windows 8.3 short names and casing)
    let canon_root = std::fs::canonicalize(root)
        .map(|p| strip_verbatim_prefix(&p))
        .unwrap_or_else(|_| clean_root.clone());

    // Check 1: Target file exists on disk
    if let Ok(canon) = std::fs::canonicalize(&clean_normalized) {
        let clean_canon = strip_verbatim_prefix(&canon);
        if !clean_canon.starts_with(&canon_root) && !clean_canon.starts_with(&clean_root) {
            return Err(format!(
                "Access denied: File path outside workspace root: {}",
                raw_path
            ));
        }
        return Ok(clean_canon);
    }

    // Check 2: Target does not exist yet.
    // Iteratively walk up the directory tree to find the closest existing ancestor.
    // Canonicalizing the existing ancestor resolves any junctions or symlinks in the path prefix.
    let mut cur = clean_normalized.as_path();
    let mut existing_ancestor = None;
    while let Some(parent) = cur.parent() {
        if parent.exists() {
            existing_ancestor = Some(parent);
            break;
        }
        cur = parent;
    }

    if let Some(ancestor) = existing_ancestor {
        if let Ok(canon_ancestor) = std::fs::canonicalize(ancestor) {
            let clean_ancestor = strip_verbatim_prefix(&canon_ancestor);
            if !clean_ancestor.starts_with(&canon_root) && !clean_ancestor.starts_with(&clean_root)
            {
                return Err(format!(
                    "Access denied: Parent directory outside workspace root: {}",
                    raw_path
                ));
            }
        } else {
            return Err(format!(
                "Access denied: Unable to resolve ancestor directory: {}",
                raw_path
            ));
        }
    } else {
        return Err(format!(
            "Access denied: Workspace root or parent directory outside workspace root: {}",
            raw_path
        ));
    }

    // Check 3: Lexical containment fallback
    if !clean_normalized.starts_with(&clean_root) && !clean_normalized.starts_with(&canon_root) {
        return Err(format!(
            "Access denied: File path outside workspace root: {}",
            raw_path
        ));
    }

    Ok(clean_normalized)
}

#[tauri::command]
async fn workspace_file_read(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    path: String,
) -> Result<String, String> {
    let root = resolve_effective_root_path(&state, conversation_id);
    let safe_path = resolve_safe_workspace_path(&root, &path)?;
    if !safe_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }
    std::fs::read_to_string(&safe_path).map_err(|e| format!("Failed to read file {}: {}", path, e))
}

#[tauri::command]
async fn workspace_file_write(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    path: String,
    content: String,
) -> Result<(), String> {
    let root = resolve_effective_root_path(&state, conversation_id);
    let safe_path = resolve_safe_workspace_path(&root, &path)?;
    if let Some(parent) = safe_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories for {}: {}", path, e))?;
    }
    std::fs::write(&safe_path, content.as_bytes())
        .map_err(|e| format!("Failed to write file {}: {}", path, e))?;
    Ok(())
}

#[tauri::command]
async fn workspace_diff_apply(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    path: String,
    diff_patch: String,
) -> Result<(), String> {
    if diff_patch.trim().is_empty() {
        return Ok(());
    }
    let root = resolve_effective_root_path(&state, conversation_id);
    let safe_path = resolve_safe_workspace_path(&root, &path)?;
    // Jamais de contenu vide silencieux : un fichier illisible traité comme
    // "" puis réécrit patché = perte de données. On échoue explicitement.
    let original = if safe_path.exists() {
        std::fs::read_to_string(&safe_path)
            .map_err(|e| format!("Failed to read file {}: {}", path, e))?
    } else {
        String::new()
    };
    let patched = apply_unified_patch(&original, &diff_patch)?;
    if let Some(parent) = safe_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories for {}: {}", path, e))?;
    }
    std::fs::write(&safe_path, patched.as_bytes())
        .map_err(|e| format!("Failed to write patched file {}: {}", path, e))?;
    Ok(())
}

#[tauri::command]
async fn workspace_git_diff(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    path: Option<String>,
) -> Result<String, String> {
    let root = resolve_effective_root_path(&state, conversation_id);
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "git", "diff"]);
        c
    } else {
        let mut c = std::process::Command::new("git");
        c.arg("diff");
        c
    };
    cmd.current_dir(&root);
    if let Some(p) = path {
        if !p.trim().is_empty() {
            let safe_path = resolve_safe_workspace_path(&root, &p)?;
            cmd.arg(safe_path);
        }
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute git diff: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !stderr.trim().is_empty() {
            return Err(format!("git diff error: {}", stderr));
        }
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

#[tauri::command]
async fn select_folder_dialog() -> CommandResult<Option<String>> {
    #[cfg(target_os = "windows")]
    {
        // Note: Windows PowerShell émet stdout dans l'encodage de la locale
        // (windows-1252 sur Windows FR). Sans ceci, un dossier contenant un
        // caractère non-ASCII (ex: Æ, é, ç) revenait en mojibake (U+FFFD) via
        // from_utf8_lossy, le root stocké ne correspondait à aucun dossier
        // réel et l'explorateur affichait "Dossier vide".
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.FolderBrowserDialog; $f.Description = 'Sélectionnez le dossier racine de travail'; if ($f.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $f.SelectedPath }",
            ])
            .output();
        if let Ok(out) = output {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
async fn notify_desktop_os(
    title: String,
    body: String,
    conversation_id: Option<String>,
) -> CommandResult<()> {
    #[cfg(target_os = "windows")]
    {
        let clean_title = title.replace('\'', "''");
        let clean_body = body.replace('\'', "''");
        let script = format!(
            "[void][System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms'); $n = New-Object System.Windows.Forms.NotifyIcon; $n.Icon = [System.Drawing.SystemIcons]::Information; $n.Visible = $true; $n.ShowBalloonTip(4000, '{clean_title}', '{clean_body}', [System.Windows.Forms.ToolTipIcon]::Info); Start-Sleep -Seconds 3; $n.Dispose();"
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let clean_title = title.replace('"', "\\\"").replace('\\', "\\\\");
        let clean_body = body.replace('"', "\\\"").replace('\\', "\\\\");
        let script = format!("display notification \"{clean_body}\" with title \"{clean_title}\"");
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("notify-send")
            .args(["--app-name=ARO", &title, &body])
            .spawn();
    }
    let _ = conversation_id;
    Ok(())
}

#[tauri::command]
async fn notification_list(
    state: State<'_, AppState>,
    filter: Option<NotificationFilter>,
) -> CommandResult<Vec<NotificationItem>> {
    let f = filter.unwrap_or_default();
    state.engine.list_notifications(&f).map_err(to_command_error)
}

#[tauri::command]
async fn notification_create(
    state: State<'_, AppState>,
    item: NotificationItem,
) -> CommandResult<NotificationItem> {
    let created = state.engine.create_notification(&item).map_err(to_command_error)?;
    let settings = state.settings().await;
    if settings.notification.desktop_notifications_enabled {
        let _ = notify_desktop_os(created.title.clone(), created.body.clone(), created.action_url.clone()).await;
    }
    Ok(created)
}

#[tauri::command]
async fn notification_mark_read(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<bool> {
    state.engine.mark_notification_as_read(&id).map_err(to_command_error)
}

#[tauri::command]
async fn notification_mark_all_read(
    state: State<'_, AppState>,
    organization_id: Option<Uuid>,
) -> CommandResult<u64> {
    state.engine.mark_all_notifications_as_read(organization_id).map_err(to_command_error)
}

#[tauri::command]
async fn notification_delete(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<bool> {
    state.engine.delete_notification(&id).map_err(to_command_error)
}

#[tauri::command]
async fn notification_clear_all(
    state: State<'_, AppState>,
    organization_id: Option<Uuid>,
) -> CommandResult<u64> {
    state.engine.clear_all_notifications(organization_id).map_err(to_command_error)
}

#[tauri::command]
async fn notification_unread_count(
    state: State<'_, AppState>,
    organization_id: Option<Uuid>,
) -> CommandResult<u64> {
    state.engine.get_unread_notification_count(organization_id).map_err(to_command_error)
}

#[tauri::command]
async fn notification_set_secret(
    state: State<'_, AppState>,
    secret_type: String,
    secret_value: String,
) -> CommandResult<AppSettings> {
    let key = secret_value.trim();
    if key.is_empty() {
        return Err("Secret value cannot be empty".to_string());
    }
    let keyring_id = match secret_type.as_str() {
        "smtp_password" | "smtpPassword" => "notification-smtp-password",
        "api_key" | "apiKey" => "notification-api-key",
        _ => return Err(format!("Unsupported notification secret type: {secret_type}")),
    };
    save_provider_api_key(keyring_id, key).map_err(to_command_error)?;
    let settings = state.settings().await;
    Ok(settings)
}

#[tauri::command]
async fn notification_clear_secret(
    state: State<'_, AppState>,
    secret_type: String,
) -> CommandResult<AppSettings> {
    let keyring_id = match secret_type.as_str() {
        "smtp_password" | "smtpPassword" => "notification-smtp-password",
        "api_key" | "apiKey" => "notification-api-key",
        _ => return Err(format!("Unsupported notification secret type: {secret_type}")),
    };
    clear_provider_api_key(keyring_id).map_err(to_command_error)?;
    let settings = state.settings().await;
    Ok(settings)
}

#[tauri::command]
async fn email_send_direct(
    state: State<'_, AppState>,
    to: String,
    subject: String,
    body: String,
    is_html: Option<bool>,
) -> CommandResult<serde_json::Value> {
    let settings = state.settings().await;
    let provider = settings.notification.email_provider.to_ascii_lowercase();

    match provider.as_str() {
        "resend" => {
            let api_key = settings.notification.api_key
                .or_else(|| std::env::var("RESEND_API_KEY").ok())
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| "Clé API Resend non configurée dans les Paramètres ARO".to_string())?;

            let from_str = settings.notification.smtp_from.as_deref().unwrap_or("onboarding@resend.dev");
            let client = reqwest::Client::new();
            let payload = serde_json::json!({
                "from": from_str,
                "to": [to.trim()],
                "subject": subject,
                "text": body,
                "html": if is_html.unwrap_or(false) { Some(body.as_str()) } else { None }
            });

            let resp = client.post("https://api.resend.com/emails")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&payload)
                .send()
                .await
                .map_err(|e| format!("Erreur réseau Resend: {e}"))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await.unwrap_or_default();
                return Err(format!("Erreur API Resend ({status}): {err_text}"));
            }

            let resp_json: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({ "success": true }));
            Ok(serde_json::json!({
                "success": true,
                "provider": "resend",
                "recipient": to,
                "detail": resp_json,
            }))
        }
        "sendgrid" => {
            let api_key = settings.notification.api_key
                .or_else(|| std::env::var("SENDGRID_API_KEY").ok())
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| "Clé API SendGrid non configurée dans les Paramètres ARO".to_string())?;

            let from_str = settings.notification.smtp_from.as_deref().unwrap_or("noreply@aro-ai.com");
            let client = reqwest::Client::new();
            let content_type = if is_html.unwrap_or(false) { "text/html" } else { "text/plain" };
            let payload = serde_json::json!({
                "personalizations": [{
                    "to": [{ "email": to.trim() }]
                }],
                "from": { "email": from_str },
                "subject": subject,
                "content": [{
                    "type": content_type,
                    "value": body
                }]
            });

            let resp = client.post("https://api.sendgrid.com/v3/mail/send")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&payload)
                .send()
                .await
                .map_err(|e| format!("Erreur réseau SendGrid: {e}"))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await.unwrap_or_default();
                return Err(format!("Erreur API SendGrid ({status}): {err_text}"));
            }

            Ok(serde_json::json!({
                "success": true,
                "provider": "sendgrid",
                "recipient": to,
                "detail": "Dispatched via SendGrid",
            }))
        }
        _ => {
            let host = settings.notification.smtp_host.as_deref().unwrap_or("").trim();
            if host.is_empty() {
                return Err("Serveur SMTP non configuré dans les Paramètres ARO".into());
            }
            let port = settings.notification.smtp_port.unwrap_or(587);
            let from_str = settings.notification.smtp_from.as_deref().unwrap_or("noreply@aro-ai.com");
            let tls_mode = settings.notification.smtp_tls_mode.as_deref().unwrap_or("starttls");
            
            use lettre::{
                message::{header::ContentType, Mailbox},
                transport::smtp::authentication::Credentials,
                AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
            };
            use std::time::Duration;

            let from: Mailbox = from_str.parse().map_err(|e| format!("Adresse expéditeur invalide: {e}"))?;
            let to_mb: Mailbox = to.trim().parse().map_err(|e| format!("Adresse destinataire invalide: {e}"))?;

            let builder = Message::builder()
                .from(from)
                .to(to_mb)
                .subject(subject);

            let message = if is_html.unwrap_or(false) {
                builder.header(ContentType::TEXT_HTML).body(body)
            } else {
                builder.header(ContentType::TEXT_PLAIN).body(body)
            }.map_err(|e| format!("Erreur de composition de message: {e}"))?;

            let transport_builder = if tls_mode.eq_ignore_ascii_case("tls") {
                AsyncSmtpTransport::<Tokio1Executor>::relay(host)
                    .map_err(|e| format!("Erreur de configuration SMTP relay: {e}"))?
                    .port(port)
                    .timeout(Some(Duration::from_secs(20)))
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                    .map_err(|e| format!("Erreur de configuration SMTP starttls: {e}"))?
                    .port(port)
                    .timeout(Some(Duration::from_secs(20)))
            };

            let mut transport_builder = transport_builder;
            if let (Some(username), Some(password)) = (&settings.notification.smtp_user, &settings.notification.smtp_password) {
                if !username.trim().is_empty() && !password.trim().is_empty() {
                    transport_builder = transport_builder.credentials(Credentials::new(username.clone(), password.clone()));
                }
            }

            let mailer = transport_builder.build();
            let response = mailer.send(message).await.map_err(|e| format!("Échec de l'envoi d'e-mail: {e}"))?;

            Ok(serde_json::json!({
                "success": true,
                "provider": "smtp",
                "recipient": to,
                "detail": format!("{:?}", response),
            }))
        }
    }
}

#[tauri::command]
async fn email_test_connection(
    state: State<'_, AppState>,
) -> CommandResult<serde_json::Value> {
    let settings = state.settings().await;
    let recipient = settings.notification.email_recipient.clone()
        .filter(|r| !r.trim().is_empty())
        .ok_or_else(|| "Veuillez configurer une adresse e-mail de destinataire dans les Paramètres".to_string())?;

    email_send_direct(
        state,
        recipient.clone(),
        "ARO - Test de Notification E-mail".to_string(),
        format!(
            "Bonjour,\n\nCeci est un e-mail de test généré par ARO Intelligence.\nVotre configuration SMTP ({}:{}) fonctionne à merveille !\n\nHorodatage : {}\n\nCordialement,\nARO Assistant",
            settings.notification.smtp_host.as_deref().unwrap_or("localhost"),
            settings.notification.smtp_port.unwrap_or(587),
            chrono::Utc::now().to_rfc3339()
        ),
        Some(false),
    ).await
}

#[tauri::command]
async fn auth_password_reset_request(
    state: State<'_, AppState>,
    email: String,
) -> CommandResult<serde_json::Value> {
    let trimmed = email.trim().to_string();
    if trimmed.is_empty() {
        return Err("Veuillez saisir votre adresse e-mail".into());
    }
    // Chemin réel : l'API émet et stocke le token (réponse neutre anti-énumération).
    // Hors-ligne, erreur honnête au lieu d'un faux succès.
    state
        .cloud
        .request_password_reset(trimmed.clone())
        .await
        .map_err(to_command_error)?;
    Ok(serde_json::json!({
        "success": true,
        "message": format!("Si un compte existe pour {}, un e-mail de réinitialisation vient d'être traité", trimmed),
    }))
}

#[tauri::command]
async fn auth_password_reset_confirm(
    state: State<'_, AppState>,
    token: String,
    new_password: String,
) -> CommandResult<serde_json::Value> {
    if token.trim().is_empty() || new_password.len() < 10 {
        return Err("Jeton invalide ou mot de passe trop court (10 caractères minimum)".into());
    }

    state
        .cloud
        .confirm_password_reset(token.trim().to_string(), new_password)
        .await
        .map_err(to_command_error)?;
    Ok(serde_json::json!({
        "success": true,
        "message": "Votre mot de passe a été réinitialisé avec succès ! Vous pouvez maintenant vous connecter."
    }))
}

#[tauri::command]
async fn message_list(
    state: State<'_, AppState>,
    conversation_id: String,
) -> CommandResult<Vec<ChatMessage>> {
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if let Ok(cloud_messages) = state
            .cloud
            .list_messages(&session.access_token, &conversation_id)
            .await
        {
            for msg in &cloud_messages {
                let _ = state.engine.upsert_message(msg);
            }
            return Ok(cloud_messages);
        }
    }
    let id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    state.engine.messages(id).map_err(to_command_error)
}

#[tauri::command]
async fn file_upload(
    state: State<'_, AppState>,
    request: FileUploadCommandRequest,
) -> CommandResult<FileObject> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "Connecte le cloud avant d'uploader un fichier.".to_string())?;
    state
        .cloud
        .upload_file(
            &session.access_token,
            request.original_name,
            request.mime_type,
            request.bytes,
            request.sha256,
        )
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn file_download(state: State<'_, AppState>, file_id: String) -> CommandResult<Vec<u8>> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "Connecte le cloud avant de télécharger un fichier.".to_string())?;
    state
        .cloud
        .download_file(&session.access_token, &file_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn message_send(
    state: State<'_, AppState>,
    request: SendMessageRequest,
) -> CommandResult<SendMessageResponse> {
    let settings = state.settings().await;
    let provider = router_for_request(&settings, &request).map_err(to_command_error)?;
    if let Some(conv_id) = request.conversation_id {
        if !state.try_begin_send(conv_id).await {
            return Err("generation already in progress for this conversation".to_string());
        }
        let response = state.engine.send_message(request, &provider).await;
        state.finish_send(&conv_id).await;
        let response = response.map_err(to_command_error)?;
        if let Err(err) = state.persist_local_result(&response).await {
            tracing::warn!(error = %err, "cloud sync of local result failed; local data kept");
        }
        return Ok(response);
    }
    let response = state
        .engine
        .send_message(request, &provider)
        .await
        .map_err(to_command_error)?;
    if let Err(err) = state.persist_local_result(&response).await {
        tracing::warn!(error = %err, "cloud sync of local result failed; local data kept");
    }
    Ok(response)
}

#[tauri::command]
async fn plans_list(
    state: State<'_, AppState>,
    conversation_id: String,
) -> CommandResult<Vec<aro_core::Plan>> {
    let uuid = uuid::Uuid::parse_str(&conversation_id).map_err(|e| e.to_string())?;
    state.engine.list_plans(uuid).map_err(to_command_error)
}

#[tauri::command]
async fn plan_create(state: State<'_, AppState>, plan: aro_core::Plan) -> CommandResult<()> {
    state.engine.create_plan(&plan).map_err(to_command_error)
}

#[tauri::command]
async fn plan_update(state: State<'_, AppState>, plan: aro_core::Plan) -> CommandResult<()> {
    state.engine.update_plan(&plan).map_err(to_command_error)
}

#[tauri::command]
async fn plan_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.engine.delete_plan(uuid).map_err(to_command_error)
}

#[tauri::command]
async fn memory_reset(state: State<'_, AppState>) -> CommandResult<()> {
    state.engine.reset_memory().await.map_err(to_command_error)
}

#[tauri::command]
async fn memory_list(state: State<'_, AppState>) -> CommandResult<Vec<LongTermMemory>> {
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if let Ok(memories) = state.cloud.list_memories(&session.access_token).await {
            for memory in &memories {
                let _ = state.engine.upsert_memory(memory).await;
            }
            return Ok(memories);
        }
    }
    state.engine.memories().map_err(to_command_error)
}

#[tauri::command]
async fn memory_episodes(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    limit: Option<usize>,
) -> CommandResult<Vec<Episode>> {
    let conv_uuid = match conversation_id {
        Some(id_str) if !id_str.trim().is_empty() => {
            Some(Uuid::parse_str(&id_str).map_err(|err| err.to_string())?)
        }
        _ => None,
    };
    state
        .engine
        .list_episodes(conv_uuid, limit)
        .map_err(to_command_error)
}

#[tauri::command]
async fn memory_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> CommandResult<Vec<LongTermMemory>> {
    let limit = limit.unwrap_or(20);
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if let Ok(memories) = state
            .cloud
            .search_memories(&session.access_token, &query, limit)
            .await
        {
            for memory in &memories {
                let _ = state.engine.upsert_memory(memory).await;
            }
            return Ok(memories);
        }
    }
    state
        .engine
        .search_memories(&query, limit)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn memory_upsert(
    state: State<'_, AppState>,
    memory: LongTermMemory,
) -> CommandResult<LongTermMemory> {
    let local = state
        .engine
        .upsert_memory(&memory)
        .await
        .map_err(to_command_error)?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let memory_id = memory.id.to_string();
        let cloud = match state
            .cloud
            .update_memory(&session.access_token, &memory_id, &memory)
            .await
        {
            Ok(saved) => Ok(saved),
            Err(_) => {
                state
                    .cloud
                    .create_memory(&session.access_token, &memory)
                    .await
            }
        };
        if let Ok(saved) = cloud {
            let _ = state.engine.upsert_memory(&saved).await;
            return Ok(saved);
        }
    }
    Ok(local)
}

#[tauri::command]
async fn memory_delete(state: State<'_, AppState>, memory_id: String) -> CommandResult<()> {
    let id = Uuid::parse_str(&memory_id).map_err(|err| err.to_string())?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let _ = state
            .cloud
            .delete_memory(&session.access_token, &memory_id)
            .await;
    }
    state
        .engine
        .delete_memory(id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn memory_index_status(state: State<'_, AppState>) -> CommandResult<MemoryIndexStatus> {
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if let Ok(status) = state.cloud.memory_index_status(&session.access_token).await {
            return Ok(status);
        }
    }
    state
        .engine
        .memory_index_status()
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn memory_index_reindex(state: State<'_, AppState>) -> CommandResult<MemoryReindexReport> {
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if let Ok(report) = state
            .cloud
            .memory_index_reindex(&session.access_token)
            .await
        {
            return Ok(report);
        }
    }
    state
        .engine
        .memory_index_reindex()
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn message_update(
    state: State<'_, AppState>,
    message_id: String,
    content: String,
) -> CommandResult<()> {
    let id = Uuid::parse_str(&message_id).map_err(|err| err.to_string())?;
    state
        .engine
        .update_message(id, content.clone())
        .map_err(to_command_error)?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let _ = state
            .cloud
            .update_message(&session.access_token, &message_id, &content)
            .await;
    }
    Ok(())
}

#[tauri::command]
async fn agent_run_start(
    state: State<'_, AppState>,
    request: AgentRunStartRequest,
) -> CommandResult<AgentRunView> {
    let view = state
        .engine
        .start_agent_run(request.clone())
        .await
        .map_err(to_command_error)?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let mut cloud_request = request;
        cloud_request.run_id = Some(view.run.id);
        cloud_request.conversation_id = view.run.conversation_id;
        let _ = state
            .cloud
            .create_agent_run(&session.access_token, &cloud_request)
            .await;
    }
    Ok(view)
}

#[tauri::command]
async fn agent_run_list(state: State<'_, AppState>) -> CommandResult<Vec<AgentRun>> {
    let local = state.engine.agent_runs().map_err(to_command_error)?;
    if !local.is_empty() {
        return Ok(local);
    }
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if let Ok(cloud) = state.cloud.list_agent_runs(&session.access_token).await {
            return Ok(cloud);
        }
    }
    Ok(local)
}

#[tauri::command]
async fn agent_orchestrator_snapshot(
    state: State<'_, AppState>,
) -> CommandResult<AgentOrchestratorSnapshot> {
    state
        .engine
        .agent_orchestrator_snapshot()
        .map_err(to_command_error)
}

#[tauri::command]
async fn agent_lane_list(state: State<'_, AppState>) -> CommandResult<Vec<AgentLaneView>> {
    state.engine.agent_lane_views().map_err(to_command_error)
}

#[tauri::command]
async fn permission_profiles_list(
    state: State<'_, AppState>,
) -> CommandResult<Vec<PermissionProfile>> {
    let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    else {
        return Ok(Vec::new());
    };
    state
        .cloud
        .list_permission_profiles(&session.access_token)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn permission_profile_upsert(
    state: State<'_, AppState>,
    profile: PermissionProfile,
) -> CommandResult<PermissionProfile> {
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .ok_or_else(|| "cloud session required".to_string())?;
    state
        .cloud
        .upsert_permission_profile(&session.access_token, &profile)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn agent_lane_pause(
    state: State<'_, AppState>,
    lane_id: String,
) -> CommandResult<AgentLaneView> {
    update_agent_lane_status(state, lane_id, false).await
}

#[tauri::command]
async fn agent_lane_resume(
    state: State<'_, AppState>,
    lane_id: String,
) -> CommandResult<AgentLaneView> {
    update_agent_lane_status(state, lane_id, true).await
}

async fn update_agent_lane_status(
    state: State<'_, AppState>,
    lane_id: String,
    active: bool,
) -> CommandResult<AgentLaneView> {
    let id = Uuid::parse_str(&lane_id).map_err(|err| err.to_string())?;
    let status = if active {
        aro_core::AgentLaneStatus::Active
    } else {
        aro_core::AgentLaneStatus::Paused
    };
    let lane = state
        .engine
        .update_agent_lane_status(id, status)
        .map_err(to_command_error)?;
    state
        .engine
        .agent_lane_views()
        .map_err(to_command_error)?
        .into_iter()
        .find(|view| view.lane.id == lane.id)
        .ok_or_else(|| "agent lane not found".to_string())
}

#[tauri::command]
async fn agent_lane_set_priority(
    state: State<'_, AppState>,
    lane_id: String,
    priority: AgentRunPriority,
) -> CommandResult<AgentLaneView> {
    let id = Uuid::parse_str(&lane_id).map_err(|err| err.to_string())?;
    let lane = state
        .engine
        .update_agent_lane_priority(id, priority)
        .map_err(to_command_error)?;
    state
        .engine
        .agent_lane_views()
        .map_err(to_command_error)?
        .into_iter()
        .find(|view| view.lane.id == lane.id)
        .ok_or_else(|| "agent lane not found".to_string())
}

#[tauri::command]
async fn agent_run_get(state: State<'_, AppState>, run_id: String) -> CommandResult<AgentRunView> {
    let id = Uuid::parse_str(&run_id).map_err(|err| err.to_string())?;
    match state.engine.agent_run_view(id) {
        Ok(view) => Ok(view),
        Err(local_err) => {
            if let Some(session) = state
                .refresh_cloud_session_from_keyring()
                .await
                .map_err(to_command_error)?
            {
                if let Ok(view) = state
                    .cloud
                    .get_agent_run(&session.access_token, &run_id)
                    .await
                {
                    return Ok(view);
                }
            }
            Err(to_command_error(local_err))
        }
    }
}

#[tauri::command]
async fn agent_run_pause(state: State<'_, AppState>, run_id: String) -> CommandResult<AgentRun> {
    update_agent_run_status(state, run_id, AgentRunStatus::Paused).await
}

#[tauri::command]
async fn agent_run_resume(state: State<'_, AppState>, run_id: String) -> CommandResult<AgentRun> {
    update_agent_run_status(state, run_id, AgentRunStatus::Running).await
}

#[tauri::command]
async fn agent_run_cancel(state: State<'_, AppState>, run_id: String) -> CommandResult<AgentRun> {
    update_agent_run_status(state, run_id, AgentRunStatus::Cancelled).await
}

async fn update_agent_run_status(
    state: State<'_, AppState>,
    run_id: String,
    status: AgentRunStatus,
) -> CommandResult<AgentRun> {
    let id = Uuid::parse_str(&run_id).map_err(|err| err.to_string())?;
    let run = state
        .engine
        .update_agent_run_status(id, status.clone())
        .map_err(to_command_error)?;
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        let result = match status {
            AgentRunStatus::Paused => {
                state
                    .cloud
                    .pause_agent_run(&session.access_token, &run_id)
                    .await
            }
            AgentRunStatus::Running => {
                state
                    .cloud
                    .resume_agent_run(&session.access_token, &run_id)
                    .await
            }
            AgentRunStatus::Cancelled => {
                state
                    .cloud
                    .cancel_agent_run(&session.access_token, &run_id)
                    .await
            }
            _ => Ok(run.clone()),
        };
        let _ = result;
    }
    Ok(run)
}

#[tauri::command]
async fn agent_context_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> CommandResult<Vec<AgentContextItem>> {
    let local = state
        .engine
        .search_agent_context(&query, limit.unwrap_or(20))
        .map_err(to_command_error)?;
    if !local.is_empty() {
        return Ok(local);
    }
    if let Some(session) = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
    {
        if let Ok(cloud) = state
            .cloud
            .search_agent_context(&session.access_token, &query, limit.unwrap_or(20))
            .await
        {
            return Ok(cloud);
        }
    }
    Ok(local)
}

#[tauri::command]
async fn message_regenerate(
    state: State<'_, AppState>,
    conversation_id: String,
    system_prompt: Option<String>,
) -> CommandResult<ChatMessage> {
    let id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    let settings = state.settings().await;
    let provider = ModelRouter::from_active(&settings.model, active_provider_api_key(&settings)?)
        .map_err(to_command_error)?;
    state
        .engine
        .regenerate_message(id, system_prompt, &provider, Some(&settings.memory))
        .await
        .map_err(to_command_error)
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamChunkPayload {
    conversation_id: String,
    message_id: String,
    content: String,
    done: bool,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ArenaStreamChunkPayload {
    slot: String, // "a" or "b"
    message_id: String,
    content: String,
    done: bool,
    model_id: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentStepPayload {
    conversation_id: String,
    step: aro_core::AgentStep,
}

#[tauri::command]
async fn message_send_stream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut request: SendMessageRequest,
    temp_assistant_message_id: String,
) -> CommandResult<SendMessageResponse> {
    use tauri::Emitter;
    let settings = state.settings().await;
    request.search_settings = Some(settings.search.clone());
    request.memory_settings = Some(settings.memory.clone());
    let provider = router_for_request(&settings, &request).map_err(to_command_error)?;

    let app_clone = app.clone();
    let app_clone_step = app.clone();
    let temp_msg_id_str = temp_assistant_message_id.clone();
    let temp_msg_id = Uuid::parse_str(&temp_assistant_message_id).map_err(|err| err.to_string())?;
    // Résoudre la conversation AVANT de streamer : émettre des chunks avec
    // un UUID nil les rendait inroutables côté frontend (perdus).
    if request.conversation_id.is_none() {
        let title_words: Vec<&str> = request.content.split_whitespace().take(8).collect();
        let title = if title_words.is_empty() {
            "New conversation".to_string()
        } else {
            title_words.join(" ")
        };
        let created = state
            .engine
            .create_conversation(title, request.mode.clone(), None, None)
            .map_err(to_command_error)?;
        request.conversation_id = Some(created.id);
    }
    let initial_conv_id = request.conversation_id.unwrap_or_default();
    let initial_conv_id_str = initial_conv_id.to_string();
    if !state.try_begin_send(initial_conv_id).await {
        return Err("generation already in progress for this conversation".to_string());
    }

    let mut step_callback = |step: aro_core::AgentStep| {
        if matches!(step.kind, aro_core::AgentStepKind::Tool | aro_core::AgentStepKind::Error) {
            let _ = app_clone_step.emit(
                "agent-step-update",
                AgentStepPayload {
                    conversation_id: initial_conv_id_str.clone(),
                    step,
                },
            );
        }
    };
    let mut on_step: Option<&mut (dyn FnMut(aro_core::AgentStep) + Send)> =
        Some(&mut step_callback);

    let response = state
        .engine
        .send_message_stream(
            request,
            &provider,
            temp_msg_id,
            &mut |chunk| {
                let _ = app_clone.emit(
                    "chat-stream-chunk",
                    StreamChunkPayload {
                        conversation_id: initial_conv_id.to_string(),
                        message_id: temp_msg_id_str.clone(),
                        content: chunk.to_string(),
                        done: false,
                    },
                );
            },
            &mut on_step,
        )
        .await;
    state.finish_send(&initial_conv_id).await;
    let response = response.map_err(to_command_error)?;
    if let Err(err) = state.persist_local_result(&response).await {
        tracing::warn!(error = %err, "cloud sync of local result failed; local data kept");
    }

    let _ = app.emit(
        "chat-stream-chunk",
        StreamChunkPayload {
            conversation_id: response.conversation.id.to_string(),
            message_id: response.assistant_message.id.to_string(),
            content: "".to_string(),
            done: true,
        },
    );

    Ok(response)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArenaRequest {
    content: String,
    mode: AssistantMode,
    system_prompt: Option<String>,
    model_a: ModelRef,
    model_b: ModelRef,
    temp_message_id_a: String,
    temp_message_id_b: String,
}

#[tauri::command]
async fn message_arena_stream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ArenaRequest,
) -> CommandResult<()> {
    use tauri::Emitter;
    let settings = state.settings().await;
    let provider_a = ModelRouter::from_model_ref(
        &settings.model,
        &request.model_a,
        provider_api_key(&request.model_a.provider_id)?,
    )
    .map_err(to_command_error)?;
    let provider_b = ModelRouter::from_model_ref(
        &settings.model,
        &request.model_b,
        provider_api_key(&request.model_b.provider_id)?,
    )
    .map_err(to_command_error)?;

    let msg_id_a = Uuid::parse_str(&request.temp_message_id_a).map_err(|err| err.to_string())?;
    let msg_id_b = Uuid::parse_str(&request.temp_message_id_b).map_err(|err| err.to_string())?;

    let req_a = SendMessageRequest {
        conversation_id: None,
        content: request.content.clone(),
        mode: request.mode.clone(),
        system_prompt: request.system_prompt.clone(),
        model_id: Some(request.model_a.model_id.clone()),
        provider: Some(request.model_a.provider_id.clone()),
        attachments: Vec::new(),
        web_access: aro_core::WebAccessMode::Off,
        search_settings: None,
        memory_settings: Some(settings.memory.clone()),
    };
    let req_b = SendMessageRequest {
        conversation_id: None,
        content: request.content.clone(),
        mode: request.mode.clone(),
        system_prompt: request.system_prompt.clone(),
        model_id: Some(request.model_b.model_id.clone()),
        provider: Some(request.model_b.provider_id.clone()),
        attachments: Vec::new(),
        web_access: aro_core::WebAccessMode::Off,
        search_settings: None,
        memory_settings: Some(settings.memory.clone()),
    };

    // Run both streams concurrently
    let app_a = app.clone();
    let model_id_a_str = request.model_a.model_id.clone();
    let msg_id_a_str = request.temp_message_id_a.clone();
    let engine_a = state.engine.clone();

    let app_b = app.clone();
    let model_id_b_str = request.model_b.model_id.clone();
    let msg_id_b_str = request.temp_message_id_b.clone();
    let engine_b = state.engine.clone();

    let fut_a = async move {
        let _ = engine_a
            .send_message_stream(
                req_a,
                &provider_a,
                msg_id_a,
                &mut |chunk| {
                    let _ = app_a.emit(
                        "arena-stream-chunk",
                        ArenaStreamChunkPayload {
                            slot: "a".to_string(),
                            message_id: msg_id_a_str.clone(),
                            content: chunk.to_string(),
                            done: false,
                            model_id: model_id_a_str.clone(),
                        },
                    );
                },
                &mut None,
            )
            .await;
        let _ = app_a.emit(
            "arena-stream-chunk",
            ArenaStreamChunkPayload {
                slot: "a".to_string(),
                message_id: msg_id_a_str.clone(),
                content: String::new(),
                done: true,
                model_id: model_id_a_str.clone(),
            },
        );
    };

    let fut_b = async move {
        let _ = engine_b
            .send_message_stream(
                req_b,
                &provider_b,
                msg_id_b,
                &mut |chunk| {
                    let _ = app_b.emit(
                        "arena-stream-chunk",
                        ArenaStreamChunkPayload {
                            slot: "b".to_string(),
                            message_id: msg_id_b_str.clone(),
                            content: chunk.to_string(),
                            done: false,
                            model_id: model_id_b_str.clone(),
                        },
                    );
                },
                &mut None,
            )
            .await;
        let _ = app_b.emit(
            "arena-stream-chunk",
            ArenaStreamChunkPayload {
                slot: "b".to_string(),
                message_id: msg_id_b_str.clone(),
                content: String::new(),
                done: true,
                model_id: model_id_b_str.clone(),
            },
        );
    };

    tokio::join!(fut_a, fut_b);

    Ok(())
}

#[tauri::command]
async fn message_regenerate_stream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    system_prompt: Option<String>,
    temp_assistant_message_id: String,
) -> CommandResult<ChatMessage> {
    use tauri::Emitter;
    let id = Uuid::parse_str(&conversation_id).map_err(|err| err.to_string())?;
    let settings = state.settings().await;
    let provider = ModelRouter::from_active(&settings.model, active_provider_api_key(&settings)?)
        .map_err(to_command_error)?;

    let app_clone = app.clone();
    let temp_msg_id_str = temp_assistant_message_id.clone();
    let temp_msg_id = Uuid::parse_str(&temp_assistant_message_id).map_err(|err| err.to_string())?;

    let assistant_msg = state
        .engine
        .regenerate_message_stream(
            id,
            system_prompt,
            &provider,
            temp_msg_id,
            &mut |chunk| {
                let _ = app_clone.emit(
                    "chat-stream-chunk",
                    StreamChunkPayload {
                        conversation_id: conversation_id.clone(),
                        message_id: temp_msg_id_str.clone(),
                        content: chunk.to_string(),
                        done: false,
                    },
                );
            },
            Some(&settings.memory),
        )
        .await
        .map_err(to_command_error)?;

    let _ = app.emit(
        "chat-stream-chunk",
        StreamChunkPayload {
            conversation_id: conversation_id.clone(),
            message_id: assistant_msg.id.to_string(),
            content: "".to_string(),
            done: true,
        },
    );

    Ok(assistant_msg)
}

#[tauri::command]
async fn voice_transcribe(
    state: State<'_, AppState>,
    request: TranscriptionRequest,
) -> CommandResult<TranscriptionResult> {
    let settings = state.settings().await;
    let scratch_dir = state.paths.scratch_dir.clone();
    tokio::task::spawn_blocking(move || {
        let voice = VoiceService::new(settings.voice, scratch_dir);
        voice.transcribe(request)
    })
    .await
    .map_err(|err| format!("voice transcription task failed: {err}"))?
    .map_err(to_command_error)
}

#[tauri::command]
async fn voice_synthesize(
    state: State<'_, AppState>,
    request: SynthesisRequest,
) -> CommandResult<SynthesisResult> {
    let settings = state.settings().await;
    let scratch_dir = state.paths.scratch_dir.clone();
    tokio::task::spawn_blocking(move || {
        let voice = VoiceService::new(settings.voice, scratch_dir);
        voice.synthesize(request)
    })
    .await
    .map_err(|err| format!("voice synthesis task failed: {err}"))?
    .map_err(to_command_error)
}

#[tauri::command]
async fn voice_wake_word_detect(
    state: State<'_, AppState>,
    request: WakeWordDetectionRequest,
) -> CommandResult<WakeWordDetectionResult> {
    let settings = state.settings().await;
    let scratch_dir = state.paths.scratch_dir.clone();
    tokio::task::spawn_blocking(move || -> AroResult<WakeWordDetectionResult> {
        let WakeWordDetectionRequest {
            audio_bytes,
            mime_type,
            language,
            variants,
            allow_embedded_aro,
        } = request;
        let voice_settings = settings.voice;
        let voice = VoiceService::new(voice_settings.clone(), scratch_dir);
        let mut gate_score = None;
        let mut gate_threshold = None;
        let mut gate_runtime_detail: Option<String> = None;

        if voice_settings.wake_word.enabled
            && matches!(
                voice_settings.wake_word.runtime,
                WakeWordRuntimeKind::LocalModel
            )
        {
            match voice.detect_wake_word(CoreWakeWordDetectionRequest {
                audio_bytes: audio_bytes.clone(),
                mime_type: mime_type.clone(),
            }) {
                Ok(model_result) => {
                    gate_score = Some(model_result.score);
                    gate_threshold = Some(model_result.threshold);
                    gate_runtime_detail = Some(model_result.runtime_detail.clone());
                    if !model_result.activated {
                        return Ok(WakeWordDetectionResult {
                            detected: false,
                            text: String::new(),
                            matched_variant: None,
                            runtime_detail: model_result.runtime_detail,
                            score: Some(model_result.score),
                            threshold: Some(model_result.threshold),
                            checked_at: chrono::Utc::now(),
                        });
                    }

                    if voice_settings.speech_to_text == VoiceRuntimeKind::Disabled {
                        return Ok(WakeWordDetectionResult {
                            detected: true,
                            text: String::new(),
                            matched_variant: Some("wake-model".to_string()),
                            runtime_detail: model_result.runtime_detail,
                            score: Some(model_result.score),
                            threshold: Some(model_result.threshold),
                            checked_at: chrono::Utc::now(),
                        });
                    }
                }
                Err(err) if voice_settings.speech_to_text != VoiceRuntimeKind::Disabled => {
                    eprintln!(
                        "wake-word local model unavailable, falling back to transcription: {err}"
                    );
                }
                Err(err) => return Err(err),
            }
        }

        let transcription = voice.transcribe(TranscriptionRequest {
            audio_bytes,
            mime_type,
            language,
        })?;
        let matched_variant = wake_word_match(
            &transcription.text,
            variants.as_deref(),
            allow_embedded_aro.unwrap_or(false),
        );

        Ok(WakeWordDetectionResult {
            detected: matched_variant.is_some(),
            text: transcription.text,
            matched_variant,
            runtime_detail: gate_runtime_detail
                .map(|detail| format!("{detail} + {}", transcription.runtime_detail))
                .unwrap_or(transcription.runtime_detail),
            score: gate_score,
            threshold: gate_threshold,
            checked_at: chrono::Utc::now(),
        })
    })
    .await
    .map_err(|err| format!("wake-word detection task failed: {err}"))?
    .map_err(to_command_error)
}

#[tauri::command]
async fn get_apple_data() -> CommandResult<serde_json::Value> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let mut notes = Vec::new();
        let mut reminders = Vec::new();
        let mut calendars = Vec::new();

        // Get Notes
        if let Ok(output) = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"Notes\" to get name of notes")
            .output()
        {
            let txt = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !txt.is_empty() {
                notes = txt.split(", ").map(|s| s.to_string()).collect();
            }
        }

        // Get Reminders
        if let Ok(output) = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"Reminders\" to get name of reminders")
            .output()
        {
            let txt = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !txt.is_empty() {
                reminders = txt.split(", ").map(|s| s.to_string()).collect();
            }
        }

        // Get Calendar titles
        if let Ok(output) = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"Calendar\" to get title of calendars")
            .output()
        {
            let txt = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !txt.is_empty() {
                calendars = txt.split(", ").map(|s| s.to_string()).collect();
            }
        }

        Ok(serde_json::json!({
            "notes": notes,
            "reminders": reminders,
            "calendars": calendars,
            "platform": "macos"
        }))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(serde_json::json!({
            "notes": [],
            "reminders": [],
            "calendars": [],
            "platform": "windows"
        }))
    }
}

#[tauri::command]
async fn show_main_window(app: tauri::AppHandle) -> CommandResult<()> {
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.show();
        let _ = main_win.set_focus();
    }
    Ok(())
}

#[tauri::command]
async fn plugins_list_installed(
    state: State<'_, AppState>,
) -> CommandResult<Vec<aro_plugins::InstalledPlugin>> {
    let plugins = state.engine.plugins();
    if !plugins.is_initialized() {
        let _ = plugins.load_all().await;
    }
    Ok(plugins.list_installed().await)
}

#[tauri::command]
async fn plugins_list_marketplace(
    state: State<'_, AppState>,
) -> CommandResult<Vec<aro_plugins::MarketplacePlugin>> {
    Ok(state.engine.plugins().list_marketplace().await)
}

#[tauri::command]
async fn plugins_install(
    state: State<'_, AppState>,
    request: aro_plugins::InstallPluginRequest,
) -> CommandResult<aro_plugins::InstalledPlugin> {
    let plugins = state.engine.plugins();
    match request.source {
        aro_plugins::PluginSourceType::Marketplace => plugins
            .install_from_marketplace(&request.target)
            .await
            .map_err(to_command_error),
        aro_plugins::PluginSourceType::Local => {
            let path = std::path::PathBuf::from(&request.target);
            plugins
                .install_from_directory(&path)
                .await
                .map_err(to_command_error)
        }
        aro_plugins::PluginSourceType::Git => plugins
            .install_from_git(&request.target)
            .await
            .map_err(to_command_error),
    }
}

#[tauri::command]
async fn plugins_custom_create(
    state: State<'_, AppState>,
    request: aro_plugins::CreateCustomPluginRequest,
) -> CommandResult<aro_plugins::InstalledPlugin> {
    state
        .engine
        .plugins()
        .install_custom(request)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn plugins_get(
    state: State<'_, AppState>,
    plugin_id: String,
) -> CommandResult<Option<aro_plugins::InstalledPlugin>> {
    Ok(state.engine.plugins().get_plugin(&plugin_id).await)
}

/// Sovereign-AI server controls: thin pass-through to the API with the cloud
/// session token. The server enforces org-admin; secrets never transit back
/// (presence flags only). Payloads stay `serde_json::Value` so no aro-store
/// dependency is needed here.
async fn ai_cloud_session(state: &State<'_, AppState>) -> CommandResult<String> {
    state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(to_command_error)?
        .map(|session| session.access_token)
        .ok_or_else(|| "cloud session required".to_string())
}

#[tauri::command]
async fn ai_cloud_status(state: State<'_, AppState>) -> CommandResult<serde_json::Value> {
    let token = ai_cloud_session(&state).await?;
    state
        .cloud
        .ai_cloud_status(&token)
        .await
        .map_err(to_command_error)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiCloudConsentCommand {
    enabled: bool,
    provider_ids: Vec<String>,
    data_residency: Option<String>,
}

#[tauri::command]
async fn ai_cloud_set_consent(
    state: State<'_, AppState>,
    request: AiCloudConsentCommand,
) -> CommandResult<serde_json::Value> {
    let token = ai_cloud_session(&state).await?;
    let body = serde_json::json!({
        "enabled": request.enabled,
        "providerIds": request.provider_ids,
        "dataResidency": request.data_residency,
    });
    state
        .cloud
        .ai_cloud_set_consent(&token, &body)
        .await
        .map_err(to_command_error)
}

#[derive(Debug, serde::Deserialize)]
struct AiCloudPutKeyCommand {
    #[serde(rename = "providerId")]
    provider_id: String,
    #[serde(rename = "apiKey")]
    api_key: String,
}

#[tauri::command]
async fn ai_cloud_put_key(
    state: State<'_, AppState>,
    request: AiCloudPutKeyCommand,
) -> CommandResult<serde_json::Value> {
    let token = ai_cloud_session(&state).await?;
    let body = serde_json::json!({ "apiKey": request.api_key });
    state
        .cloud
        .ai_cloud_put_key(&token, &request.provider_id, &body)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn ai_cloud_delete_key(
    state: State<'_, AppState>,
    provider_id: String,
) -> CommandResult<serde_json::Value> {
    let token = ai_cloud_session(&state).await?;
    state
        .cloud
        .ai_cloud_delete_key(&token, &provider_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn assistant_status(state: State<'_, AppState>) -> CommandResult<serde_json::Value> {
    let token = ai_cloud_session(&state).await?;
    state
        .cloud
        .assistant_status(&token)
        .await
        .map_err(to_command_error)
}

/// Read an uploaded plugin logo as `{ mime, dataUrl }` (JSON transport).
/// Returns `None` when the plugin carries no file logo.
#[tauri::command]
async fn plugins_read_plugin_logo(
    state: State<'_, AppState>,
    plugin_id: String,
) -> CommandResult<Option<serde_json::Value>> {
    let data_url = state
        .engine
        .plugins()
        .read_plugin_logo_data_url(&plugin_id)
        .await
        .map_err(to_command_error)?;
    Ok(data_url.map(|data_url| {
        let mime = data_url
            .split(';')
            .next()
            .unwrap_or("data:image/png")
            .trim_start_matches("data:");
        serde_json::json!({ "mime": mime, "dataUrl": data_url })
    }))
}

#[tauri::command]
async fn plugins_uninstall(
    state: State<'_, AppState>,
    plugin_id: String,
) -> CommandResult<serde_json::Value> {
    state
        .engine
        .plugins()
        .uninstall(&plugin_id)
        .await
        .map_err(to_command_error)?;
    Ok(serde_json::json!({ "success": true }))
}

#[tauri::command]
async fn plugins_toggle(
    state: State<'_, AppState>,
    plugin_id: String,
    enabled: bool,
) -> CommandResult<aro_plugins::InstalledPlugin> {
    state
        .engine
        .plugins()
        .set_enabled(&plugin_id, enabled)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn plugins_mcp_test(
    state: State<'_, AppState>,
    plugin_id: String,
    server_name: String,
) -> CommandResult<Vec<aro_mcp::McpTool>> {
    state
        .engine
        .plugins()
        .test_mcp_server(&plugin_id, &server_name)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn plugins_mcp_call(
    state: State<'_, AppState>,
    plugin_id: String,
    server_name: String,
    tool_name: String,
    arguments: Option<serde_json::Value>,
) -> CommandResult<aro_mcp::CallToolResult> {
    let args = arguments.unwrap_or_else(|| serde_json::json!({}));
    state
        .engine
        .plugins()
        .call_mcp_tool(&plugin_id, &server_name, &tool_name, args)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn plugins_skill_invoke(
    state: State<'_, AppState>,
    plugin_id: String,
    skill_id: String,
    input: Option<serde_json::Value>,
) -> CommandResult<aro_skills::SkillOutput> {
    let inp = input.unwrap_or_else(|| serde_json::json!({}));
    state
        .engine
        .plugins()
        .invoke_skill(&plugin_id, &skill_id, &inp)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn plugins_accounts_list(
    state: State<'_, AppState>,
    plugin_id: Option<String>,
) -> CommandResult<Vec<aro_plugins::PluginAccount>> {
    let plugins = state.engine.plugins();
    if !plugins.is_initialized() {
        let _ = plugins.load_all().await;
    }
    plugins
        .list_accounts(plugin_id.as_deref())
        .map_err(to_command_error)
}

#[tauri::command]
async fn plugins_account_connect_oauth_start(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    request: plugin_accounts::ConnectOAuthStartRequest,
) -> CommandResult<plugin_accounts::ConnectOAuthStartResponse> {
    let plugins = state.engine.plugins();
    if !plugins.is_initialized() {
        let _ = plugins.load_all().await;
    }
    let should_open = request.open_browser.unwrap_or(true);
    let res = plugin_accounts::start_oauth_flow(plugins, request).await?;
    if should_open {
        use tauri_plugin_opener::OpenerExt;
        let _ = app_handle.opener().open_url(&res.auth_url, None::<String>);
    }
    Ok(res)
}

#[tauri::command]
async fn plugins_account_connect_api_key(
    state: State<'_, AppState>,
    request: plugin_accounts::ConnectApiKeyRequest,
) -> CommandResult<aro_plugins::PluginAccount> {
    let plugins = state.engine.plugins();
    if !plugins.is_initialized() {
        let _ = plugins.load_all().await;
    }
    if request.api_key.trim().is_empty() {
        return Err("La clé API ou jeton ne peut pas être vide".to_string());
    }

    let auth_method = request.auth_method.unwrap_or_else(|| "api_key".to_string());
    let trimmed_key = request.api_key.trim();
    let account_identifier = if let Some(ident) = request.account_identifier.filter(|s| !s.trim().is_empty()) {
        ident.trim().to_string()
    } else if let Some(lbl) = request.label.as_ref().filter(|s| !s.trim().is_empty()) {
        lbl.trim().to_string()
    } else {
        format!(
            "key-{}...{}",
            &trimmed_key[..3.min(trimmed_key.len())],
            &trimmed_key[trimmed_key.len().saturating_sub(4)..]
        )
    };

    let label = request.label.unwrap_or_else(|| account_identifier.clone());

    let account = plugins
        .create_account(aro_plugins::CreatePluginAccountInput {
            plugin_id: request.plugin_id,
            account_identifier,
            label,
            email: None,
            display_name: None,
            avatar_url: None,
            auth_method,
            is_default: None,
            status: Some("active".to_string()),
        })
        .map_err(to_command_error)?;

    // Persist secret in OS Keyring
    plugin_accounts::save_plugin_account_secret(&account.id, trimmed_key)
        .map_err(to_command_error)?;

    Ok(account)
}

#[tauri::command]
async fn plugins_account_set_default(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<aro_plugins::PluginAccount> {
    state
        .engine
        .plugins()
        .set_default_account(&account_id)
        .map_err(to_command_error)
}

#[tauri::command]
async fn plugins_account_update_label(
    state: State<'_, AppState>,
    account_id: String,
    label: String,
) -> CommandResult<aro_plugins::PluginAccount> {
    state
        .engine
        .plugins()
        .update_account_label(&account_id, &label)
        .map_err(to_command_error)
}

#[tauri::command]
async fn plugins_account_disconnect(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<serde_json::Value> {
    let _ = plugin_accounts::delete_plugin_account_secret(&account_id);
    state
        .engine
        .plugins()
        .delete_account(&account_id)
        .map_err(to_command_error)?;
    Ok(serde_json::json!({ "success": true }))
}

#[tauri::command]
async fn plugins_account_test_health(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<plugin_accounts::AccountHealthReport> {
    plugin_accounts::validate_account_health(&state.engine.plugins(), &account_id).await
}

/// Exécute un outil avec le root effectif de la conversation injecté
/// quand l'appelant n'en fournit pas (factorise code_execute/document_create).
async fn execute_tool_with_workspace_root(
    state: &State<'_, AppState>,
    tool_id: &str,
    request: serde_json::Value,
) -> CommandResult<aro_core::ToolExecutionResult> {
    let conv_id = request
        .get("conversationId")
        .or_else(|| request.get("conversation_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    let mut tool_req = aro_core::ToolExecutionRequest::new(
        Uuid::new_v4(),
        conv_id,
        tool_id.to_string(),
        request.clone(),
    );
    if let Some(cid) = conv_id {
        if tool_req.input.get("root_path").is_none() && tool_req.input.get("rootPath").is_none() {
            if let Ok(Some(eff_root)) = state.engine.resolve_effective_root_path(cid) {
                if let Some(obj) = tool_req.input.as_object_mut() {
                    obj.insert("root_path".to_string(), serde_json::Value::String(eff_root));
                }
            }
        }
    }
    let policy = aro_tools::WebAccessPolicy::unrestricted();
    state
        .engine
        .tools()
        .execute(tool_req, &policy)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn code_execute(
    state: State<'_, AppState>,
    request: serde_json::Value,
) -> CommandResult<aro_core::ToolExecutionResult> {
    execute_tool_with_workspace_root(&state, aro_core::TOOL_CORE_CODE_EXECUTE, request).await
}

#[tauri::command]
async fn document_create(
    state: State<'_, AppState>,
    request: serde_json::Value,
) -> CommandResult<aro_core::ToolExecutionResult> {
    execute_tool_with_workspace_root(&state, aro_core::TOOL_CORE_DOCUMENT_CREATE, request).await
}

#[tauri::command]
async fn computer_use(
    state: State<'_, AppState>,
    request: serde_json::Value,
) -> CommandResult<aro_core::ToolExecutionResult> {
    execute_tool_with_workspace_root(&state, aro_core::TOOL_CORE_COMPUTER_USE, request).await
}

fn to_command_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn sync_health_for_cloud_error(error: &AroError) -> aro_core::SyncHealth {
    match error {
        AroError::Security(_) => aro_core::SyncHealth::SessionExpired,
        _ => aro_core::SyncHealth::OfflineReadOnly,
    }
}

fn runtime_unavailable_from_settings(
    settings: &AppSettings,
    error: impl std::fmt::Display,
) -> RuntimeStatus {
    let connection = settings.model.active_connection();
    RuntimeStatus::unavailable(
        connection
            .map(|provider| provider.kind.clone())
            .unwrap_or_else(|| settings.model.provider.clone()),
        settings.model.active_model_ref.model_id.clone(),
        connection.and_then(|provider| provider.endpoint.clone()),
        error.to_string(),
    )
}

async fn voice_status_for_settings(settings: VoiceSettings) -> CommandResult<VoiceStatus> {
    Ok(voice_model_status_for_settings(settings)
        .await?
        .voice_status)
}

async fn voice_model_status_for_settings(
    settings: VoiceSettings,
) -> CommandResult<VoiceModelsStatus> {
    tokio::task::spawn_blocking(move || build_voice_model_status(&settings))
        .await
        .map_err(|err| format!("voice model status task failed: {err}"))
}

fn build_voice_model_status(settings: &VoiceSettings) -> VoiceModelsStatus {
    let voice_status = settings.status();
    let speech_to_text = VoiceModelReadiness {
        capability: VoiceModelCapability::SpeechToText,
        runtime: VoiceModelRuntimeKind::from(&settings.speech_to_text),
        ready: voice_status.speech_to_text.ready,
        optional: false,
        binary_path: settings.whisper_binary.clone(),
        model_path: settings.whisper_model_path.clone(),
        detail: voice_model_detail(
            VoiceModelCapability::SpeechToText,
            &settings.speech_to_text,
            voice_status.speech_to_text.ready,
        ),
        issues: voice_status.speech_to_text.issues.clone(),
    };
    let text_to_speech = VoiceModelReadiness {
        capability: VoiceModelCapability::TextToSpeech,
        runtime: VoiceModelRuntimeKind::from(&settings.text_to_speech),
        ready: voice_status.text_to_speech.ready,
        optional: false,
        binary_path: settings.piper_binary.clone(),
        model_path: settings.piper_voice_path.clone(),
        detail: voice_model_detail(
            VoiceModelCapability::TextToSpeech,
            &settings.text_to_speech,
            voice_status.text_to_speech.ready,
        ),
        issues: voice_status.text_to_speech.issues.clone(),
    };
    let wake_word = wake_word_model_status(settings, &voice_status);
    let models = vec![
        speech_to_text.clone(),
        text_to_speech.clone(),
        wake_word.clone(),
    ];

    VoiceModelsStatus {
        enabled: settings.enabled,
        ready: voice_status.ready,
        checked_at: voice_status.checked_at,
        voice_status,
        speech_to_text,
        text_to_speech,
        wake_word,
        models,
    }
}

fn wake_word_model_status(
    settings: &VoiceSettings,
    voice_status: &VoiceStatus,
) -> VoiceModelReadiness {
    let mut issues = Vec::new();
    if let Some(issue) = voice_status
        .issues
        .iter()
        .find(|issue| issue.code == VoiceReadinessIssueCode::VoiceDisabled)
        .cloned()
    {
        issues.push(issue);
    }
    issues.extend(voice_status.wake_word.issues.iter().cloned());

    VoiceModelReadiness {
        capability: VoiceModelCapability::WakeWord,
        runtime: VoiceModelRuntimeKind::from(&settings.wake_word.runtime),
        ready: voice_status.wake_word.ready,
        optional: true,
        binary_path: None,
        model_path: settings.wake_word.model_path.clone(),
        detail: if voice_status.wake_word.ready {
            "Wake-word local model is ready.".to_string()
        } else if !settings.wake_word.enabled
            || matches!(settings.wake_word.runtime, WakeWordRuntimeKind::Disabled)
        {
            "Wake-word detection is disabled.".to_string()
        } else {
            "Wake-word local model needs configuration.".to_string()
        },
        issues,
    }
}

fn voice_model_detail(
    capability: VoiceModelCapability,
    runtime: &VoiceRuntimeKind,
    ready: bool,
) -> String {
    match (capability, runtime, ready) {
        (VoiceModelCapability::SpeechToText, VoiceRuntimeKind::WhisperCpp, true) => {
            "whisper.cpp speech-to-text model is ready.".to_string()
        }
        (VoiceModelCapability::SpeechToText, VoiceRuntimeKind::WhisperCpp, false) => {
            "whisper.cpp speech-to-text model needs configuration.".to_string()
        }
        (VoiceModelCapability::TextToSpeech, VoiceRuntimeKind::Piper, true) => {
            "Piper text-to-speech voice model is ready.".to_string()
        }
        (VoiceModelCapability::TextToSpeech, VoiceRuntimeKind::Piper, false) => {
            "Piper text-to-speech voice model needs configuration.".to_string()
        }
        (VoiceModelCapability::SpeechToText, VoiceRuntimeKind::Disabled, _) => {
            "Speech-to-text is disabled.".to_string()
        }
        (VoiceModelCapability::TextToSpeech, VoiceRuntimeKind::Disabled, _) => {
            "Text-to-speech is disabled.".to_string()
        }
        (VoiceModelCapability::SpeechToText, VoiceRuntimeKind::Piper, _) => {
            "Piper cannot be used for speech-to-text.".to_string()
        }
        (VoiceModelCapability::TextToSpeech, VoiceRuntimeKind::WhisperCpp, _) => {
            "whisper.cpp cannot be used for text-to-speech.".to_string()
        }
        (VoiceModelCapability::WakeWord, _, _) => {
            "Wake-word detection follows the speech-to-text model readiness.".to_string()
        }
    }
}

fn wake_word_match(
    text: &str,
    variants: Option<&[String]>,
    allow_embedded_aro: bool,
) -> Option<String> {
    let normalized_variants: Vec<String> = variants
        .map(|items| {
            items
                .iter()
                .map(|item| normalize_voice_text(item))
                .collect()
        })
        .unwrap_or_else(|| {
            ["aro", "haro", "arrow", "aero"]
                .into_iter()
                .map(str::to_string)
                .collect()
        });

    for word in voice_words(text) {
        if normalized_variants.iter().any(|variant| variant == &word) || is_aro_like_word(&word) {
            return Some(word);
        }
        if allow_embedded_aro && word.contains("aro") {
            return Some(word);
        }
    }

    None
}

fn voice_words(text: &str) -> Vec<String> {
    let normalized = normalize_voice_text(text);
    if normalized.is_empty() {
        Vec::new()
    } else {
        normalized.split_whitespace().map(str::to_string).collect()
    }
}

fn normalize_voice_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut last_was_space = true;

    for character in text.chars() {
        let folded = fold_voice_character(character);
        if folded.is_ascii_alphanumeric() {
            normalized.push(folded);
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    normalized.trim().to_string()
}

fn fold_voice_character(character: char) -> char {
    match character {
        'A'..='Z' => character.to_ascii_lowercase(),
        '\u{00e0}' | '\u{00e1}' | '\u{00e2}' | '\u{00e3}' | '\u{00e4}' | '\u{00e5}'
        | '\u{00c0}' | '\u{00c1}' | '\u{00c2}' | '\u{00c3}' | '\u{00c4}' | '\u{00c5}' => 'a',
        '\u{00e7}' | '\u{00c7}' => 'c',
        '\u{00e8}' | '\u{00e9}' | '\u{00ea}' | '\u{00eb}' | '\u{00c8}' | '\u{00c9}'
        | '\u{00ca}' | '\u{00cb}' => 'e',
        '\u{00ec}' | '\u{00ed}' | '\u{00ee}' | '\u{00ef}' | '\u{00cc}' | '\u{00cd}'
        | '\u{00ce}' | '\u{00cf}' => 'i',
        '\u{00f1}' | '\u{00d1}' => 'n',
        '\u{00f2}' | '\u{00f3}' | '\u{00f4}' | '\u{00f5}' | '\u{00f6}' | '\u{00d2}'
        | '\u{00d3}' | '\u{00d4}' | '\u{00d5}' | '\u{00d6}' => 'o',
        '\u{00f9}' | '\u{00fa}' | '\u{00fb}' | '\u{00fc}' | '\u{00d9}' | '\u{00da}'
        | '\u{00db}' | '\u{00dc}' => 'u',
        '\u{00fd}' | '\u{00ff}' | '\u{00dd}' => 'y',
        _ => character,
    }
}

fn is_aro_like_word(word: &str) -> bool {
    let bytes = word.as_bytes();
    let mut index = if bytes.first() == Some(&b'h') { 1 } else { 0 };

    let a_start = index;
    while bytes.get(index) == Some(&b'a') {
        index += 1;
    }
    let r_start = index;
    while bytes.get(index) == Some(&b'r') {
        index += 1;
    }
    let o_start = index;
    while bytes.get(index) == Some(&b'o') {
        index += 1;
    }

    index == bytes.len() && a_start < r_start && r_start < o_start && o_start < index
}

fn session_to_view(session: AuthSession) -> CloudSessionView {
    CloudSessionView {
        user: session.user,
        active_organization: session.active_organization,
        memberships: session.memberships,
        expires_at: session.expires_at,
    }
}

async fn model_options_for_settings(settings: &AppSettings) -> Vec<ModelRef> {
    let mut models = Vec::new();
    for connection in &settings.model.providers {
        let api_key = provider_api_key(&connection.id).ok().flatten();
        match list_models_for_connection(&settings.model, connection, api_key.as_deref()).await {
            Ok(mut provider_models) => models.append(&mut provider_models),
            Err(_) => models.extend(connection.models.clone()),
        }
    }
    if !models.iter().any(|model| {
        model.provider_id == settings.model.active_model_ref.provider_id
            && model.model_id == settings.model.active_model_ref.model_id
    }) {
        models.insert(0, settings.model.active_model_ref.clone());
    }
    models.sort_by(|a, b| {
        a.local
            .cmp(&b.local)
            .reverse()
            .then_with(|| a.provider_id.cmp(&b.provider_id))
            .then_with(|| a.label.cmp(&b.label))
    });
    models
}

fn active_provider_api_key(settings: &AppSettings) -> CommandResult<Option<String>> {
    provider_api_key(&settings.model.active_model_ref.provider_id)
}

fn provider_api_key(provider_id: &str) -> CommandResult<Option<String>> {
    load_provider_api_key(provider_id).map_err(to_command_error)
}

fn with_auth_status(mut providers: Vec<ModelProviderConnection>) -> Vec<ModelProviderConnection> {
    for provider in &mut providers {
        provider.auth_configured = !provider.kind.requires_api_key()
            || provider_api_key(&provider.id).ok().flatten().is_some();
    }
    providers
}

fn settings_with_auth_status(mut settings: AppSettings) -> AppSettings {
    settings.model.providers = with_auth_status(settings.model.providers);
    // `AppState::settings` hydrates the search credential from the OS keyring for native
    // execution. Renderer-facing commands must expose only the boolean status, never the
    // credential itself. `skip_serializing` on SearchSettings is another independent guard.
    settings.search.auth_configured = settings.search.api_key.is_some();
    settings.search.api_key = None;
    settings
}

fn sanitize_provider_auth_status(providers: &mut [ModelProviderConnection]) {
    for provider in providers {
        provider.auth_configured = false;
    }
}

fn validate_provider_endpoints(model: &aro_core::ModelSettings) -> AroResult<()> {
    for provider in &model.providers {
        match provider.kind {
            ModelProviderKind::Ollama | ModelProviderKind::LlamaCpp => {
                if let Some(endpoint) = provider.endpoint.as_deref() {
                    aro_runtime::ensure_loopback_url(endpoint)?;
                }
            }
            _ if !provider.kind.is_local() => {
                if let Some(endpoint) = provider.endpoint.as_deref() {
                    aro_runtime::ensure_remote_https_url(endpoint)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn router_for_request(
    settings: &AppSettings,
    request: &SendMessageRequest,
) -> AroResult<ModelRouter> {
    if let (Some(provider_id), Some(model_id)) = (&request.provider, &request.model_id) {
        let connection = settings
            .model
            .connection(provider_id)
            .ok_or_else(|| AroError::Configuration("model provider not found".to_string()))?;
        let model_ref = connection
            .models
            .iter()
            .find(|model| &model.model_id == model_id)
            .cloned()
            .unwrap_or_else(|| {
                ModelRef::new(
                    connection.id.clone(),
                    connection.kind.clone(),
                    model_id.clone(),
                    model_id.clone(),
                )
            });
        return ModelRouter::from_model_ref(
            &settings.model,
            &model_ref,
            load_provider_api_key(provider_id)?,
        );
    }
    ModelRouter::from_active(
        &settings.model,
        load_provider_api_key(&settings.model.active_model_ref.provider_id)?,
    )
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("aro=info")
        .try_init()
        .ok();

    let app_state = load_or_create_state().expect("failed to initialize ARO state");

    tauri::Builder::default()
        // This plugin must remain first so a deep link received by a second OS process is
        // forwarded to the already-running application.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let valid_integration_link = argv.iter().any(|argument| {
                Url::parse(argument).is_ok_and(|url| {
                    url.scheme() == "aro"
                        && url.host_str() == Some("integrations")
                        && url.path() == "/complete"
                        && url.query_pairs().any(|(name, value)| {
                            name == "attempt" && Uuid::parse_str(value.as_ref()).is_ok()
                        })
                })
            });
            if valid_integration_link {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed
                        && shortcut.matches(Modifiers::ALT, Code::Space)
                    {
                        if let Some(spotlight) = app.get_webview_window("spotlight") {
                            let is_visible = spotlight.is_visible().unwrap_or(false);
                            if is_visible {
                                let _ = spotlight.hide();
                            } else {
                                let _ = spotlight.show();
                                let _ = spotlight.set_focus();
                                let _ = spotlight.center();
                            }
                        }
                    }
                })
                .build(),
        )
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .setup(|app| {
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                app.deep_link().register_all()?;
            }
            // Register global shortcut: Alt + Space
            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);
            if let Err(e) = app.global_shortcut().register(shortcut) {
                eprintln!("[WARN] Failed to register Alt+Space global shortcut (may already be in use): {e}");
            }

            // Programmatically build spotlight window
            let _spotlight = WebviewWindowBuilder::new(
                app,
                "spotlight",
                WebviewUrl::App("index.html?mode=spotlight".into()),
            )
            .title("ARO Spotlight")
            .inner_size(680.0, 110.0)
            .min_inner_size(480.0, 90.0)
            .max_inner_size(1000.0, 600.0)
            .resizable(true)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .center()
            .visible(false)
            .build()
            .expect("failed to create spotlight window");

            Ok(())
        })
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            app_bootstrap,
            open_url,
            integration_connect_start,
            integration_connect_status,
            integration_api_key_connect,
            integration_catalog_v3_list,
            integration_installations_v3_list,
            integration_credential_v3_create,
            integration_authorization_v3_create,
            integration_authorization_v3_get,
            integration_installation_v3_action,
            cloud_session_get,
            cloud_auth_register,
            cloud_auth_login,
            cloud_invitation_accept,
            cloud_auth_logout,
            cloud_organizations_list,
            cloud_organization_create,
            cloud_organization_switch,
            cloud_state_set,
            cloud_state_delete,
            cloud_collection_get,
            cloud_collection_create,
            cloud_collection_update,
            cloud_collection_delete,
            plans_list,
            plan_create,
            plan_update,
            plan_delete,
            cloud_user_profile_update,
            cloud_organization_update,
            cloud_api_keys_list,
            billing::billing_request,
            billing::billing_open_payment,
            cloud_api_key_create,
            cloud_api_key_revoke,
            cloud_members_list,
            cloud_invitations_list,
            cloud_invitation_revoke,
            cloud_member_invite,
            cloud_member_role_update,
            cloud_member_remove,
            settings_get,
            settings_update,
            search_provider_set_api_key,
            search_provider_clear_api_key,
            preferences_update,
            runtime_check,
            voice_status,
            voice_model_status,
            model_list,
            model_provider_list,
            model_provider_upsert,
            model_provider_delete,
            model_provider_set_api_key,
            model_provider_clear_api_key,
            model_provider_test,
            model_catalog_refresh,
            model_select,
            conversation_create,
            conversation_list,
            conversation_delete,
            conversation_export,
            conversations_delete_empty,
            conversation_update_title,
            conversation_move,
            conversation_set_root_path,
            conversation_effective_root_path,
            conversation_search_content,
            workspace_tree_get,
            workspace_file_read,
            workspace_file_write,
            workspace_diff_apply,
            workspace_git_diff,
            select_folder_dialog,
            notify_desktop_os,
            notification_list,
            notification_create,
            notification_mark_read,
            notification_mark_all_read,
            notification_delete,
            notification_clear_all,
            notification_unread_count,
            notification_set_secret,
            notification_clear_secret,
            email_send_direct,
            email_test_connection,
            auth_password_reset_request,
            auth_password_reset_confirm,
            project_list,
            project_create,
            project_update,
            project_delete,
            folder_list,
            folder_create,
            folder_update,
            folder_delete,
            message_list,
            file_upload,
            file_download,
            message_send,
            message_send_stream,
            message_arena_stream,
            message_update,
            agent_run_start,
            agent_run_list,
            agent_orchestrator_snapshot,
            agent_lane_list,
            permission_profiles_list,
            permission_profile_upsert,
            agent_lane_pause,
            agent_lane_resume,
            agent_lane_set_priority,
            agent_run_get,
            agent_run_pause,
            agent_run_resume,
            agent_run_cancel,
            agent_context_search,
            message_regenerate,
            message_regenerate_stream,
            memory_reset,
            memory_list,
            memory_episodes,
            memory_search,
            memory_upsert,
            memory_delete,
            memory_index_status,
            memory_index_reindex,
            voice_transcribe,
            voice_synthesize,
            voice_wake_word_detect,
            get_apple_data,
            show_main_window,
            plugins_list_installed,
            plugins_list_marketplace,
            plugins_install,
            plugins_custom_create,
            plugins_get,
            plugins_read_plugin_logo,
            plugins_uninstall,
            plugins_toggle,
            plugins_mcp_test,
            plugins_mcp_call,
            plugins_skill_invoke,
            plugins_accounts_list,
            plugins_account_connect_oauth_start,
            plugins_account_connect_api_key,
            plugins_account_set_default,
            plugins_account_update_label,
            plugins_account_disconnect,
            plugins_account_test_health,
            ai_cloud_status,
            ai_cloud_set_consent,
            ai_cloud_put_key,
            ai_cloud_delete_key,
            assistant_status,
            code_execute,
            document_create,
            computer_use
        ])
        .run(tauri::generate_context!())
        .expect("error while running ARO");
}
