use aro_core::{
    AgentContextItem, AgentEvent, AgentRun, AgentRunStartRequest, AgentRunView, AppSettings,
    AroError, AroResult, AuthSession, BootstrapPayloadV2, ChatMessage, Conversation, FileObject,
    Folder, LongTermMemory, MembershipRole, Organization, OrganizationInvitation,
    OrganizationMember, PermissionProfile, Project, PublicApiKey, User, UserPreferences,
};
use aro_vector::{MemoryIndexStatus, MemoryReindexReport};
use reqwest::header;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

const KEYRING_SERVICE: &str = "ARO";
const KEYRING_REFRESH_TOKEN_USER: &str = "cloud-refresh-token";
const KEYRING_PROVIDER_PREFIX: &str = "model-provider";

#[derive(Debug, Clone)]
pub struct CloudApiClient {
    base_url: String,
    client: Client,
}

impl CloudApiClient {
    pub fn try_new(base_url: impl Into<String>) -> AroResult<Self> {
        let base_url = validate_api_base_url(&base_url.into())?;
        Ok(Self::from_normalized_base_url(base_url))
    }

    fn from_normalized_base_url(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn register(&self, request: RegisterRequest) -> AroResult<AuthSession> {
        self.post_public("/auth/register", &request).await
    }

    pub async fn login(&self, request: LoginRequest) -> AroResult<AuthSession> {
        self.post_public("/auth/login", &request).await
    }

    pub async fn refresh(&self, refresh_token: String) -> AroResult<AuthSession> {
        self.post_public("/auth/refresh", &RefreshRequest { refresh_token })
            .await
    }

    pub async fn logout(&self, refresh_token: String) -> AroResult<()> {
        let _: serde_json::Value = self
            .post_public("/auth/logout", &RefreshRequest { refresh_token })
            .await?;
        Ok(())
    }

    pub async fn accept_invitation(
        &self,
        token: String,
        email: String,
        password: String,
    ) -> AroResult<AuthSession> {
        self.post_public(
            "/auth/invitations/accept-account",
            &AcceptExistingInvitationWithPasswordRequest {
                token,
                email,
                password,
            },
        )
        .await
    }

    pub async fn bootstrap(&self, access_token: &str) -> AroResult<BootstrapPayloadV2> {
        self.get_authed("/bootstrap", access_token).await
    }

    pub async fn list_organizations(&self, access_token: &str) -> AroResult<Vec<Organization>> {
        self.get_authed("/organizations", access_token).await
    }

    pub async fn create_organization(
        &self,
        access_token: &str,
        request: &OrganizationCreateRequest,
    ) -> AroResult<OrganizationCreateResponse> {
        self.post_authed("/organizations", access_token, request)
            .await
    }

    pub async fn switch_organization(
        &self,
        access_token: &str,
        refresh_token: &str,
        organization_id: &str,
    ) -> AroResult<AuthSession> {
        self.post_authed(
            "/auth/switch-organization",
            access_token,
            &SwitchOrganizationRequest {
                organization_id: organization_id.to_string(),
                refresh_token: refresh_token.to_string(),
            },
        )
        .await
    }

    pub async fn update_user_profile(
        &self,
        access_token: &str,
        request: &UserProfilePatch,
    ) -> AroResult<User> {
        self.patch_authed("/users/me", access_token, request).await
    }

    pub async fn update_organization(
        &self,
        access_token: &str,
        organization_id: &str,
        request: &OrganizationPatch,
    ) -> AroResult<Organization> {
        self.patch_authed(
            &format!("/organizations/{organization_id}"),
            access_token,
            request,
        )
        .await
    }

    pub async fn list_api_keys(&self, access_token: &str) -> AroResult<Vec<PublicApiKey>> {
        self.get_authed("/api-keys", access_token).await
    }

    pub async fn create_api_key(
        &self,
        access_token: &str,
        name: String,
    ) -> AroResult<ApiKeyCreateResponse> {
        self.post_authed("/api-keys", access_token, &ApiKeyCreateRequest { name })
            .await
    }

    pub async fn revoke_api_key(&self, access_token: &str, api_key_id: &str) -> AroResult<()> {
        let url = format!("{}/api-keys/{api_key_id}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }

    pub async fn list_members(&self, access_token: &str) -> AroResult<Vec<OrganizationMember>> {
        self.get_authed("/memberships", access_token).await
    }

    pub async fn list_invitations(
        &self,
        access_token: &str,
        cursor: Option<&str>,
        limit: i64,
        include_closed: bool,
    ) -> AroResult<OrganizationInvitationPage> {
        let mut path = format!(
            "/invitations?limit={}&includeClosed={include_closed}",
            limit.clamp(1, 200)
        );
        if let Some(cursor) = cursor.filter(|cursor| !cursor.trim().is_empty()) {
            path.push_str("&cursor=");
            path.push_str(&url_escape(cursor));
        }
        self.get_authed(&path, access_token).await
    }

    pub async fn revoke_invitation(
        &self,
        access_token: &str,
        invitation_id: &str,
    ) -> AroResult<()> {
        let url = format!("{}/invitations/{invitation_id}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }

    pub async fn invite_member(
        &self,
        access_token: &str,
        request: &MembershipCreateRequest,
    ) -> AroResult<OrganizationInvitationReceipt> {
        self.post_authed("/memberships", access_token, request)
            .await
    }

    pub async fn update_member_role(
        &self,
        access_token: &str,
        membership_id: &str,
        role: MembershipRole,
    ) -> AroResult<OrganizationMember> {
        self.patch_authed(
            &format!("/memberships/{membership_id}"),
            access_token,
            &MembershipUpdateRequest { role },
        )
        .await
    }

    pub async fn remove_member(&self, access_token: &str, membership_id: &str) -> AroResult<()> {
        let url = format!("{}/memberships/{membership_id}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }

    pub async fn create_conversation(
        &self,
        access_token: &str,
        title: String,
        mode: aro_core::AssistantMode,
        project_id: Option<String>,
        folder_id: Option<String>,
    ) -> AroResult<Conversation> {
        self.post_authed(
            "/conversations",
            access_token,
            &CreateConversationRequest {
                title: Some(title),
                mode,
                project_id,
                folder_id,
            },
        )
        .await
    }

    pub async fn list_conversations(&self, access_token: &str) -> AroResult<Vec<Conversation>> {
        self.get_authed("/conversations", access_token).await
    }

    pub async fn request_password_reset(&self, email: String) -> AroResult<serde_json::Value> {
        self.post_public(
            "/auth/password-reset/request",
            &serde_json::json!({ "email": email }),
        )
        .await
    }

    pub async fn confirm_password_reset(
        &self,
        token: String,
        new_password: String,
    ) -> AroResult<serde_json::Value> {
        self.post_public(
            "/auth/password-reset/confirm",
            &serde_json::json!({ "token": token, "newPassword": new_password }),
        )
        .await
    }

    pub async fn move_conversation(
        &self,
        access_token: &str,
        conversation_id: &str,
        project_id: Option<String>,
        folder_id: Option<String>,
    ) -> AroResult<Conversation> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct MovePayload {
            #[serde(skip_serializing_if = "Option::is_none")]
            project_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            folder_id: Option<String>,
        }
        let url = format!("{}/conversations/{conversation_id}/move", self.base_url);
        let response = self
            .client
            .patch(url)
            .bearer_auth(access_token)
            .json(&MovePayload {
                project_id,
                folder_id,
            })
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response::<Conversation>(response).await
    }

    pub async fn list_projects(&self, access_token: &str) -> AroResult<Vec<Project>> {
        self.get_authed("/projects", access_token).await
    }

    pub async fn create_project(
        &self,
        access_token: &str,
        project: &Project,
    ) -> AroResult<Project> {
        self.post_authed("/projects", access_token, project).await
    }

    pub async fn delete_project(&self, access_token: &str, project_id: &str) -> AroResult<()> {
        let url = format!("{}/projects/{project_id}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        if response.status() == StatusCode::NO_CONTENT || response.status().is_success() {
            return Ok(());
        }
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }

    pub async fn list_folders(&self, access_token: &str) -> AroResult<Vec<Folder>> {
        self.get_authed("/folders", access_token).await
    }

    pub async fn create_folder(&self, access_token: &str, folder: &Folder) -> AroResult<Folder> {
        self.post_authed("/folders", access_token, folder).await
    }

    pub async fn delete_folder(&self, access_token: &str, folder_id: &str) -> AroResult<()> {
        let url = format!("{}/folders/{folder_id}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        if response.status() == StatusCode::NO_CONTENT || response.status().is_success() {
            return Ok(());
        }
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }
    pub async fn integration_connect_start(
        &self,
        access_token: &str,
        provider_id: &str,
        redirect_uri: Option<String>,
        scopes: Vec<String>,
    ) -> AroResult<serde_json::Value> {
        let body = serde_json::json!({
            "redirectUri": redirect_uri,
            "scopes": scopes,
        });
        self.post_authed(
            &format!("/integrations/{provider_id}/connect/start"),
            access_token,
            &body,
        )
        .await
    }

    pub async fn integration_connect_status(
        &self,
        access_token: &str,
        state: &str,
    ) -> AroResult<serde_json::Value> {
        self.get_authed(
            &format!("/integrations/connect/status?state={state}"),
            access_token,
        )
        .await
    }

    pub async fn integration_api_key_create(
        &self,
        access_token: &str,
        provider_id: &str,
        secret: String,
        public_config: serde_json::Value,
        account: serde_json::Value,
    ) -> AroResult<serde_json::Value> {
        self.post_authed(
            &format!("/integrations/{provider_id}/api-key"),
            access_token,
            &serde_json::json!({
                "secret": secret,
                "publicConfig": public_config,
                "account": account,
            }),
        )
        .await
    }

    pub async fn integration_catalog_v3(&self, access_token: &str) -> AroResult<serde_json::Value> {
        self.get_authed("/integration-catalog", access_token).await
    }

    pub async fn integration_installations_v3(
        &self,
        access_token: &str,
    ) -> AroResult<serde_json::Value> {
        self.get_authed("/integration-installations", access_token)
            .await
    }

    pub async fn integration_credential_v3_create(
        &self,
        access_token: &str,
        provider_id: &str,
        idempotency_key: &str,
        request: serde_json::Value,
    ) -> AroResult<serde_json::Value> {
        let url = format!(
            "{}/integration-catalog/{provider_id}/credentials",
            self.base_url
        );
        let response = self
            .client
            .post(url)
            .bearer_auth(access_token)
            .header("Idempotency-Key", idempotency_key)
            .header(header::CACHE_CONTROL, "no-store")
            .json(&request)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    pub async fn integration_authorization_v3_create(
        &self,
        access_token: &str,
        provider_id: &str,
        idempotency_key: &str,
        request: serde_json::Value,
    ) -> AroResult<serde_json::Value> {
        self.post_authed_idempotent(
            &format!("/integration-catalog/{provider_id}/authorization-attempts"),
            access_token,
            idempotency_key,
            &request,
        )
        .await
    }

    pub async fn integration_authorization_v3_get(
        &self,
        access_token: &str,
        attempt_id: &str,
    ) -> AroResult<serde_json::Value> {
        self.get_authed(
            &format!("/integration-authorization-attempts/{attempt_id}"),
            access_token,
        )
        .await
    }

    pub async fn integration_installation_v3_action(
        &self,
        access_token: &str,
        installation_id: &str,
        action: &str,
    ) -> AroResult<serde_json::Value> {
        self.post_authed(
            &format!("/integration-installations/{installation_id}/{action}"),
            access_token,
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn delete_conversation(
        &self,
        access_token: &str,
        conversation_id: &str,
    ) -> AroResult<()> {
        let url = format!("{}/conversations/{conversation_id}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }

    pub async fn update_conversation_title(
        &self,
        access_token: &str,
        conversation_id: &str,
        title: String,
    ) -> AroResult<Conversation> {
        self.patch_authed(
            &format!("/conversations/{conversation_id}"),
            access_token,
            &UpdateConversationRequest { title },
        )
        .await
    }

    pub async fn list_messages(
        &self,
        access_token: &str,
        conversation_id: &str,
    ) -> AroResult<Vec<ChatMessage>> {
        self.get_authed(
            &format!("/conversations/{conversation_id}/messages"),
            access_token,
        )
        .await
    }

    pub async fn update_message(
        &self,
        access_token: &str,
        message_id: &str,
        content: &str,
    ) -> AroResult<()> {
        let _: serde_json::Value = self
            .patch_authed(
                &format!("/messages/{message_id}"),
                access_token,
                &serde_json::json!({ "content": content }),
            )
            .await?;
        Ok(())
    }

    pub async fn update_settings(
        &self,
        access_token: &str,
        settings: &AppSettings,
    ) -> AroResult<AppSettings> {
        self.put_authed("/settings", access_token, settings).await
    }

    pub async fn update_preferences(
        &self,
        access_token: &str,
        preferences: &UserPreferencesPatch,
    ) -> AroResult<UserPreferences> {
        self.put_authed("/preferences", access_token, preferences)
            .await
    }

    pub async fn store_local_result(
        &self,
        access_token: &str,
        request: &LocalResultRequest,
    ) -> AroResult<()> {
        // The full message is synced, agent execution traces (`steps`)
        // included: the route carries no body limit and `messages.steps`
        // persists them server-side. Nothing is dropped.
        let payload_bytes = serde_json::to_vec(request).map(|v| v.len()).unwrap_or(0);
        let idempotency_key = format!("local-result-{}", request.assistant_message.id);
        let _: serde_json::Value = self
            .post_authed_idempotent(
                "/assistant/local-result",
                access_token,
                &idempotency_key,
                request,
            )
            .await
            .map_err(|err| {
                AroError::RuntimeUnavailable(format!(
                    "local result sync failed ({payload_bytes} bytes): {err}"
                ))
            })?;
        Ok(())
    }

    pub async fn upload_file(
        &self,
        access_token: &str,
        original_name: String,
        mime_type: String,
        bytes: Vec<u8>,
        sha256: Option<String>,
    ) -> AroResult<FileObject> {
        let upload: FileUploadCreateResponse = self
            .post_authed(
                "/files/uploads",
                access_token,
                &FileUploadCreateRequest {
                    original_name,
                    mime_type: mime_type.clone(),
                    size_bytes: bytes.len() as i64,
                    sha256,
                },
            )
            .await?;
        let response = self
            .client
            .put(format!("{}{}", self.base_url, upload.upload_url))
            .bearer_auth(access_token)
            .header(header::CONTENT_TYPE, mime_type)
            .body(bytes)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    pub async fn download_file(&self, access_token: &str, file_id: &str) -> AroResult<Vec<u8>> {
        let response = self
            .client
            .get(format!("{}/files/{}/content", self.base_url, file_id))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Err(AroError::RuntimeUnavailable(format!(
                "Failed to download file: {}",
                response.status()
            )));
        }
        let bytes = response.bytes().await.map_err(map_reqwest)?;
        Ok(bytes.to_vec())
    }

    pub async fn create_agent_run(
        &self,
        access_token: &str,
        request: &AgentRunStartRequest,
    ) -> AroResult<AgentRunView> {
        self.post_authed("/agent/runs", access_token, request).await
    }

    pub async fn list_agent_runs(&self, access_token: &str) -> AroResult<Vec<AgentRun>> {
        self.get_authed("/agent/runs", access_token).await
    }

    pub async fn get_agent_run(&self, access_token: &str, run_id: &str) -> AroResult<AgentRunView> {
        self.get_authed(&format!("/agent/runs/{run_id}"), access_token)
            .await
    }

    pub async fn pause_agent_run(&self, access_token: &str, run_id: &str) -> AroResult<AgentRun> {
        self.post_authed(
            &format!("/agent/runs/{run_id}/pause"),
            access_token,
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn resume_agent_run(&self, access_token: &str, run_id: &str) -> AroResult<AgentRun> {
        self.post_authed(
            &format!("/agent/runs/{run_id}/resume"),
            access_token,
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn cancel_agent_run(&self, access_token: &str, run_id: &str) -> AroResult<AgentRun> {
        self.post_authed(
            &format!("/agent/runs/{run_id}/cancel"),
            access_token,
            &serde_json::json!({}),
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn agent_run_events(
        &self,
        access_token: &str,
        run_id: &str,
    ) -> AroResult<Vec<AgentEvent>> {
        self.get_authed(&format!("/agent/runs/{run_id}/events"), access_token)
            .await
    }

    pub async fn search_agent_context(
        &self,
        access_token: &str,
        query: &str,
        limit: usize,
    ) -> AroResult<Vec<AgentContextItem>> {
        self.get_authed(
            &format!(
                "/agent/context/search?q={}&limit={limit}",
                url_escape(query)
            ),
            access_token,
        )
        .await
    }

    pub async fn list_memories(&self, access_token: &str) -> AroResult<Vec<LongTermMemory>> {
        self.get_authed("/memories", access_token).await
    }

    pub async fn search_memories(
        &self,
        access_token: &str,
        query: &str,
        limit: usize,
    ) -> AroResult<Vec<LongTermMemory>> {
        self.get_authed(
            &format!("/memories/search?q={}&limit={limit}", url_escape(query)),
            access_token,
        )
        .await
    }

    pub async fn create_memory(
        &self,
        access_token: &str,
        memory: &LongTermMemory,
    ) -> AroResult<LongTermMemory> {
        self.post_authed("/memories", access_token, memory).await
    }

    pub async fn update_memory(
        &self,
        access_token: &str,
        memory_id: &str,
        memory: &LongTermMemory,
    ) -> AroResult<LongTermMemory> {
        self.patch_authed(&format!("/memories/{memory_id}"), access_token, memory)
            .await
    }

    pub async fn delete_memory(&self, access_token: &str, memory_id: &str) -> AroResult<()> {
        let url = format!("{}/memories/{memory_id}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }

    pub async fn memory_index_status(&self, access_token: &str) -> AroResult<MemoryIndexStatus> {
        self.get_authed("/memory-index/status", access_token).await
    }

    pub async fn memory_index_reindex(&self, access_token: &str) -> AroResult<MemoryReindexReport> {
        self.post_authed(
            "/memory-index/reindex",
            access_token,
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn list_permission_profiles(
        &self,
        access_token: &str,
    ) -> AroResult<Vec<PermissionProfile>> {
        self.get_authed("/agent/permission-profiles", access_token)
            .await
    }

    pub async fn upsert_permission_profile(
        &self,
        access_token: &str,
        profile: &PermissionProfile,
    ) -> AroResult<PermissionProfile> {
        self.put_authed("/agent/permission-profiles", access_token, profile)
            .await
    }

    pub async fn set_client_state(
        &self,
        access_token: &str,
        key: &str,
        value: serde_json::Value,
    ) -> AroResult<()> {
        let _: serde_json::Value = self
            .put_authed(
                &format!("/client-state/{key}"),
                access_token,
                &ClientStateSetRequest { value },
            )
            .await?;
        Ok(())
    }

    pub async fn delete_client_state(&self, access_token: &str, key: &str) -> AroResult<()> {
        let url = format!("{}/client-state/{key}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }

    pub async fn get_collection(
        &self,
        access_token: &str,
        collection: &str,
    ) -> AroResult<serde_json::Value> {
        self.get_authed(&format!("/collections/{collection}"), access_token)
            .await
    }

    pub async fn billing_request(&self, access_token:&str, method:&str, path:&str, body:Option<serde_json::Value>) -> AroResult<serde_json::Value> {
        let valid = matches!((method,path),
            ("GET","/billing/catalog"|"/billing/account"|"/billing/compute-keys") |
            ("POST","/billing/checkout"|"/billing/checkout/resume"|"/billing/portal"|"/billing/compute-keys") |
            ("PUT","/billing/limits")) || (method=="DELETE" && path.strip_prefix("/billing/compute-keys/").is_some_and(|id|uuid::Uuid::parse_str(id).is_ok()));
        if !valid {return Err(aro_core::AroError::Security("unsupported billing operation".into()));}
        let method=reqwest::Method::from_bytes(method.as_bytes()).map_err(|e|aro_core::AroError::Configuration(e.to_string()))?;
        let mut request=self.client.request(method,format!("{}{}",self.base_url,path)).bearer_auth(access_token);
        if let Some(body)=body {request=request.json(&body);}
        read_response(request.send().await.map_err(map_reqwest)?).await
    }

    pub async fn create_collection_item(
        &self,
        access_token: &str,
        collection: &str,
        payload: serde_json::Value,
    ) -> AroResult<serde_json::Value> {
        self.post_authed(
            &format!("/collections/{collection}"),
            access_token,
            &payload,
        )
        .await
    }

    pub async fn update_collection_item(
        &self,
        access_token: &str,
        collection: &str,
        item_id: &str,
        payload: serde_json::Value,
    ) -> AroResult<serde_json::Value> {
        self.patch_authed(
            &format!("/collections/{collection}/{item_id}"),
            access_token,
            &payload,
        )
        .await
    }

    pub async fn delete_collection_item(
        &self,
        access_token: &str,
        collection: &str,
        item_id: &str,
    ) -> AroResult<()> {
        let url = format!("{}/collections/{collection}/{item_id}", self.base_url);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response::<serde_json::Value>(response).await?;
        Ok(())
    }

    async fn get_authed<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        access_token: &str,
    ) -> AroResult<T> {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    async fn post_public<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> AroResult<T> {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .json(body)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    async fn post_authed<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        access_token: &str,
        body: &B,
    ) -> AroResult<T> {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    async fn post_authed_idempotent<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        access_token: &str,
        idempotency_key: &str,
        body: &B,
    ) -> AroResult<T> {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .bearer_auth(access_token)
            .header("idempotency-key", idempotency_key)
            .json(body)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    async fn put_authed<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        access_token: &str,
        body: &B,
    ) -> AroResult<T> {
        let response = self
            .client
            .put(format!("{}{}", self.base_url, path))
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    async fn patch_authed<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        access_token: &str,
        body: &B,
    ) -> AroResult<T> {
        let response = self
            .client
            .patch(format!("{}{}", self.base_url, path))
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    async fn delete_authed<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        access_token: &str,
    ) -> AroResult<T> {
        let response = self
            .client
            .delete(format!("{}{}", self.base_url, path))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response).await
    }

    pub async fn ai_cloud_status(&self, access_token: &str) -> AroResult<serde_json::Value> {
        self.get_authed("/settings/ai-cloud/status", access_token)
            .await
    }

    pub async fn ai_cloud_set_consent(
        &self,
        access_token: &str,
        body: &serde_json::Value,
    ) -> AroResult<serde_json::Value> {
        self.put_authed("/settings/ai-cloud/consent", access_token, body)
            .await
    }

    pub async fn ai_cloud_put_key(
        &self,
        access_token: &str,
        provider_id: &str,
        body: &serde_json::Value,
    ) -> AroResult<serde_json::Value> {
        self.put_authed(
            &format!("/settings/ai-cloud/keys/{provider_id}"),
            access_token,
            body,
        )
        .await
    }

    pub async fn ai_cloud_delete_key(
        &self,
        access_token: &str,
        provider_id: &str,
    ) -> AroResult<serde_json::Value> {
        self.delete_authed(
            &format!("/settings/ai-cloud/keys/{provider_id}"),
            access_token,
        )
        .await
    }

    pub async fn assistant_status(&self, access_token: &str) -> AroResult<serde_json::Value> {
        self.get_authed("/assistant/status", access_token).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
    pub organization_name: String,
    pub organization_domain: Option<String>,
    pub organization_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SwitchOrganizationRequest {
    organization_id: String,
    refresh_token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshRequest {
    refresh_token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptExistingInvitationWithPasswordRequest {
    token: String,
    email: String,
    password: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateConversationRequest {
    title: Option<String>,
    mode: aro_core::AssistantMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateConversationRequest {
    title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalResultRequest {
    pub conversation: Conversation,
    pub user_message: ChatMessage,
    pub assistant_message: ChatMessage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileUploadCreateRequest {
    original_name: String,
    mime_type: String,
    size_bytes: i64,
    sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileUploadCreateResponse {
    upload_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientStateSetRequest {
    value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfilePatch {
    pub name: Option<String>,
    pub role_title: Option<String>,
    pub avatar_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPatch {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationCreateRequest {
    pub name: String,
    pub domain: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationCreateResponse {
    pub organization: Organization,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationInvitationPage {
    pub items: Vec<OrganizationInvitation>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferencesPatch {
    pub theme: Option<String>,
    pub language: Option<String>,
    pub wake_word_enabled: Option<bool>,
    pub inference_mode: Option<aro_core::InferenceMode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyCreateRequest {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyCreateResponse {
    pub key: PublicApiKey,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MembershipCreateRequest {
    pub name: String,
    pub email: String,
    pub role: MembershipRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationInvitationReceipt {
    pub invitation_id: String,
    pub organization_id: String,
    pub email: String,
    pub name: String,
    pub role: MembershipRole,
    pub status: String,
    pub expires_at: String,
    pub member: Option<OrganizationMember>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MembershipUpdateRequest {
    role: MembershipRole,
}

pub fn load_refresh_token() -> AroResult<Option<String>> {
    match keyring_entry() {
        Ok(entry) => match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(AroError::Configuration(err.to_string())),
        },
        Err(err) => Err(AroError::Configuration(err.to_string())),
    }
}

pub fn save_refresh_token(refresh_token: &str) -> AroResult<()> {
    keyring_entry()
        .map_err(|err| AroError::Configuration(err.to_string()))?
        .set_password(refresh_token)
        .map_err(|err| AroError::Configuration(err.to_string()))
}

pub fn clear_refresh_token() -> AroResult<()> {
    match keyring_entry() {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(AroError::Configuration(err.to_string())),
        },
        Err(err) => Err(AroError::Configuration(err.to_string())),
    }
}

fn keyring_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_TOKEN_USER)
}

pub fn load_provider_api_key(provider_id: &str) -> AroResult<Option<String>> {
    match provider_keyring_entry(provider_id) {
        Ok(entry) => match entry.get_password() {
            Ok(value) => {
                let value = value.trim().to_string();
                Ok((!value.is_empty()).then_some(value))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(AroError::Configuration(err.to_string())),
        },
        Err(err) => Err(err),
    }
}

pub fn save_provider_api_key(provider_id: &str, api_key: &str) -> AroResult<()> {
    if api_key.trim().is_empty() {
        return Err(AroError::Configuration(
            "provider API key is required".to_string(),
        ));
    }
    provider_keyring_entry(provider_id)?
        .set_password(api_key.trim())
        .map_err(|err| AroError::Configuration(err.to_string()))
}

pub fn clear_provider_api_key(provider_id: &str) -> AroResult<()> {
    match provider_keyring_entry(provider_id) {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(AroError::Configuration(err.to_string())),
        },
        Err(err) => Err(err),
    }
}

fn provider_keyring_entry(provider_id: &str) -> AroResult<keyring::Entry> {
    validate_provider_id(provider_id)?;
    keyring::Entry::new(
        KEYRING_SERVICE,
        &format!("{KEYRING_PROVIDER_PREFIX}-{provider_id}"),
    )
    .map_err(|err| AroError::Configuration(err.to_string()))
}

fn validate_provider_id(provider_id: &str) -> AroResult<()> {
    let valid = !provider_id.is_empty()
        && provider_id.len() <= 128
        && provider_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    if valid {
        Ok(())
    } else {
        Err(AroError::Configuration(
            "provider id contains unsupported characters".to_string(),
        ))
    }
}

async fn read_response<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> AroResult<T> {
    let status = response.status();
    if status.is_success() {
        return response.json::<T>().await.map_err(map_reqwest);
    }

    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            if body.trim().is_empty() {
                format!("cloud API returned {status}")
            } else {
                body
            }
        });

    if status == StatusCode::UNAUTHORIZED {
        Err(AroError::Security(message))
    } else if status == StatusCode::TOO_MANY_REQUESTS {
        Err(AroError::Unexpected(format!("cloud rate limit: {message}")))
    } else {
        Err(AroError::RuntimeUnavailable(message))
    }
}

fn map_reqwest(err: reqwest::Error) -> AroError {
    AroError::RuntimeUnavailable(err.to_string())
}

fn url_escape(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            other => format!("%{other:02X}").chars().collect(),
        })
        .collect()
}

fn validate_api_base_url(raw_url: &str) -> AroResult<String> {
    let mut url = Url::parse(raw_url.trim())
        .map_err(|err| AroError::Configuration(format!("invalid ARO_API_BASE_URL: {err}")))?;

    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AroError::Security(format!(
                "unsupported API URL scheme '{other}'"
            )));
        }
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(AroError::Security(
            "ARO_API_BASE_URL must not include credentials".to_string(),
        ));
    }

    if url.query().is_some() || url.fragment().is_some() {
        return Err(AroError::Configuration(
            "ARO_API_BASE_URL must not include query or fragment".to_string(),
        ));
    }

    if !matches!(url.path(), "" | "/" | "/v1" | "/v1/") {
        return Err(AroError::Configuration(
            "ARO_API_BASE_URL may only contain the optional /v1 API prefix".to_string(),
        ));
    }

    let host = url
        .host_str()
        .ok_or_else(|| AroError::Configuration("ARO_API_BASE_URL has no host".to_string()))?;
    let is_loopback = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if url.scheme() == "http" && !is_loopback {
        return Err(AroError::Security(
            "ARO_API_BASE_URL must use HTTPS unless it targets localhost/loopback".to_string(),
        ));
    }

    url.set_path("/v1");
    Ok(url.as_str().trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_base_url_accepts_loopback_http_and_remote_https() {
        assert_eq!(
            validate_api_base_url("http://127.0.0.1:8710/").expect("loopback http"),
            "http://127.0.0.1:8710/v1"
        );
        assert!(validate_api_base_url("http://localhost:8710").is_ok());
        assert!(validate_api_base_url("https://api.aro.example").is_ok());
        assert_eq!(
            validate_api_base_url("https://api.aro.example/v1/").expect("explicit v1"),
            "https://api.aro.example/v1"
        );
    }

    #[test]
    fn api_base_url_rejects_insecure_remote_and_embedded_credentials() {
        assert!(validate_api_base_url("http://api.aro.example").is_err());
        assert!(validate_api_base_url("https://user:pass@api.aro.example").is_err());
        assert!(validate_api_base_url("https://api.aro.example?token=secret").is_err());
        assert!(validate_api_base_url("https://api.aro.example/internal").is_err());
    }

    #[test]
    fn provider_ids_are_keyring_safe() {
        assert!(validate_provider_id("openai-compatible-prod_1").is_ok());
        assert!(validate_provider_id("").is_err());
        assert!(validate_provider_id("../openai").is_err());
        assert!(validate_provider_id("openai key").is_err());
    }

    #[test]
    fn organization_switch_serializes_the_current_refresh_token() {
        let value = serde_json::to_value(SwitchOrganizationRequest {
            organization_id: "organization-id".to_string(),
            refresh_token: "refresh-token".to_string(),
        })
        .expect("serialize organization switch request");

        assert_eq!(value["organizationId"], "organization-id");
        assert_eq!(value["refreshToken"], "refresh-token");
    }
}
