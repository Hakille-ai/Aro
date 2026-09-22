use aro_core::{
    AroError, AroResult, ChatMessage, MessageRole, ModelGeneration, ModelGenerationRequest,
    ModelProviderConnection, ModelProviderKind, ModelRef, ModelResponseFormat, ModelSettings,
    RuntimeStatus, TOOL_CORE_SEARCH_WEB, TOOL_CORE_WEB_PAGE_READ,
};
use aro_tools::{extract_urls, web_request_likely};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::{ensure_loopback_url, ensure_remote_https_url};

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn generate(&self, request: ModelGenerationRequest) -> AroResult<ModelGeneration>;
    async fn generate_stream(
        &self,
        request: ModelGenerationRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> AroResult<ModelGeneration>;
    async fn status(&self) -> RuntimeStatus;
    fn temperature(&self) -> f32;
    fn max_tokens(&self) -> u32;
}

pub async fn list_ollama_models(settings: &ModelSettings) -> AroResult<Vec<String>> {
    let endpoint = format!(
        "{}/api/tags",
        settings.ollama_endpoint.trim_end_matches('/')
    );
    ensure_loopback_url(&endpoint)?;

    let response = Client::new()
        .get(&endpoint)
        .send()
        .await
        .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(AroError::RuntimeUnavailable(format!(
            "Ollama returned {status} while listing models"
        )));
    }

    let body: OllamaTagsResponse = response
        .json()
        .await
        .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

    Ok(body.models.into_iter().map(|model| model.name).collect())
}

pub async fn list_models_for_connection(
    settings: &ModelSettings,
    connection: &ModelProviderConnection,
    api_key: Option<&str>,
) -> AroResult<Vec<ModelRef>> {
    let mut models = match connection.kind {
        ModelProviderKind::Mock => vec![
            model_ref(connection, "mock", "Mock", true, true),
            model_ref(connection, "gemma3:1b", "gemma3:1b", true, true),
        ],
        ModelProviderKind::Ollama => {
            let mut scoped = settings.clone();
            scoped.select_model(&connection.id, connection.kind.default_model_id());
            let installed = list_ollama_models(&scoped).await.unwrap_or_default();
            if installed.is_empty() {
                cached_models(connection, false, false)
            } else {
                installed
                    .into_iter()
                    .map(|model| model_ref(connection, &model, &model, true, true))
                    .collect()
            }
        }
        ModelProviderKind::LlamaCpp => cached_models(connection, true, true),
        ModelProviderKind::OpenAi => {
            remote_default_models(
                connection,
                &[
                    "gpt-5.5-pro",
                    "gpt-5.5",
                    "gpt-5.4-pro",
                    "gpt-5.4",
                    "gpt-5.4-mini",
                    "gpt-4o",
                    "gpt-4o-mini",
                    "o3",
                    "o3-mini",
                    "o1",
                    "o1-mini",
                ],
                api_key,
            )
            .await
        }
        ModelProviderKind::Anthropic => remote_static_models(
            connection,
            &[
                "claude-fable-5",
                "claude-sonnet-5",
                "claude-opus-4-8",
                "claude-haiku-4-5",
                "claude-3-7-sonnet-20250219",
                "claude-3-5-sonnet-20241022",
                "claude-3-5-haiku-20241022",
            ],
            api_key.is_some(),
        ),
        ModelProviderKind::Google => remote_static_models(
            connection,
            &[
                "gemini-3.5-flash",
                "gemini-3.1-pro",
                "gemini-3.1-flash-lite",
                "gemini-2.0-flash",
                "gemini-1.5-pro",
                "gemini-1.5-flash",
            ],
            api_key.is_some(),
        ),
        ModelProviderKind::Mistral => {
            remote_default_models(
                connection,
                &[
                    "mistral-large-latest",
                    "mistral-medium-3.5",
                    "mistral-small-4",
                    "mistral-small-latest",
                    "codestral-latest",
                    "pixtral-large-latest",
                ],
                api_key,
            )
            .await
        }
        ModelProviderKind::OpenAiCompatible => {
            remote_default_models(connection, &[connection.kind.default_model_id()], api_key).await
        }
    };
    models.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(models)
}

async fn remote_default_models(
    connection: &ModelProviderConnection,
    fallback: &[&str],
    api_key: Option<&str>,
) -> Vec<ModelRef> {
    let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) else {
        return remote_static_models(connection, fallback, false);
    };
    let Some(endpoint) = connection.endpoint.as_deref() else {
        return remote_static_models(connection, fallback, true);
    };
    if ensure_remote_https_url(endpoint).is_err() {
        return remote_static_models(connection, fallback, true);
    }
    let url = format!("{}/models", endpoint.trim_end_matches('/'));
    let response = Client::new().get(url).bearer_auth(api_key).send().await;
    match response {
        Ok(response) if response.status().is_success() => {
            match response.json::<OpenAiModelsResponse>().await {
                Ok(body) if !body.data.is_empty() => body
                    .data
                    .into_iter()
                    .map(|model| model_ref(connection, &model.id, &model.id, true, true))
                    .collect(),
                _ => remote_static_models(connection, fallback, true),
            }
        }
        _ => remote_static_models(connection, fallback, true),
    }
}

fn remote_static_models(
    connection: &ModelProviderConnection,
    ids: &[&str],
    auth_ready: bool,
) -> Vec<ModelRef> {
    ids.iter()
        .map(|id| model_ref(connection, id, id, auth_ready, auth_ready))
        .collect()
}

fn cached_models(
    connection: &ModelProviderConnection,
    installed: bool,
    ready: bool,
) -> Vec<ModelRef> {
    let models = if connection.models.is_empty() {
        vec![ModelRef::new(
            connection.id.clone(),
            connection.kind.clone(),
            connection.kind.default_model_id(),
            connection.kind.default_model_id(),
        )]
    } else {
        connection.models.clone()
    };

    models
        .into_iter()
        .map(|model| {
            model_ref(
                connection,
                &model.model_id,
                &model.label,
                installed && model.installed,
                ready && model.ready,
            )
        })
        .collect()
}

fn model_ref(
    connection: &ModelProviderConnection,
    model_id: &str,
    label: &str,
    installed: bool,
    ready: bool,
) -> ModelRef {
    ModelRef {
        provider_id: connection.id.clone(),
        provider_kind: connection.kind.clone(),
        model_id: model_id.to_string(),
        label: label.to_string(),
        family: Some(connection.display_name.clone()),
        local: connection.kind.is_local(),
        installed,
        ready,
    }
}

#[derive(Clone)]
pub struct ModelRouter {
    settings: ModelSettings,
    connection: ModelProviderConnection,
    model_ref: ModelRef,
    api_key: Option<String>,
    client: Client,
}

impl ModelRouter {
    pub fn from_active(settings: &ModelSettings, api_key: Option<String>) -> AroResult<Self> {
        let settings = settings.clone().normalized();
        let connection = settings.active_connection().cloned().ok_or_else(|| {
            AroError::Configuration("active model provider not found".to_string())
        })?;
        Self::from_parts(settings, connection, api_key)
    }

    pub fn from_model_ref(
        settings: &ModelSettings,
        model_ref: &ModelRef,
        api_key: Option<String>,
    ) -> AroResult<Self> {
        let mut settings = settings.clone().normalized();
        let connection = settings
            .connection(&model_ref.provider_id)
            .cloned()
            .ok_or_else(|| AroError::Configuration("model provider not found".to_string()))?;
        settings.select_model(&connection.id, &model_ref.model_id);
        Self::from_parts(settings, connection, api_key)
    }

    fn from_parts(
        settings: ModelSettings,
        connection: ModelProviderConnection,
        api_key: Option<String>,
    ) -> AroResult<Self> {
        if !connection.enabled {
            return Err(AroError::Configuration(format!(
                "{} is disabled",
                connection.display_name
            )));
        }
        if connection.kind.requires_api_key()
            && api_key.as_deref().unwrap_or_default().trim().is_empty()
        {
            return Err(AroError::Configuration(format!(
                "{} needs an API key before it can run",
                connection.display_name
            )));
        }
        if !connection.kind.is_local() {
            let endpoint = connection.endpoint.as_deref().ok_or_else(|| {
                AroError::Configuration("provider endpoint is missing".to_string())
            })?;
            ensure_remote_https_url(endpoint)?;
        }
        let model_ref = settings.active_model_ref.clone();
        Ok(Self {
            settings,
            connection,
            model_ref,
            api_key,
            client: Client::new(),
        })
    }

    fn local_provider(&self) -> AroResult<LocalModelProvider> {
        LocalModelProvider::from_settings(&self.settings)
    }

    fn endpoint(&self, suffix: &str) -> AroResult<String> {
        let endpoint =
            self.connection.endpoint.as_deref().ok_or_else(|| {
                AroError::Configuration("provider endpoint is missing".to_string())
            })?;
        Ok(format!("{}{}", endpoint.trim_end_matches('/'), suffix))
    }

    fn api_key(&self) -> AroResult<&str> {
        self.api_key
            .as_deref()
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| AroError::Configuration("provider API key is missing".to_string()))
    }

    async fn generate_openai_compatible(
        &self,
        request: ModelGenerationRequest,
        stream: bool,
        on_chunk: Option<&mut (dyn FnMut(String) + Send)>,
    ) -> AroResult<ModelGeneration> {
        let endpoint = self.endpoint("/chat/completions")?;
        let mut messages = vec![json!({
            "role": "system",
            "content": request.system_prompt,
        })];
        messages.extend(request.messages.iter().map(openai_message));
        let mut body = json!({
            "model": self.model_ref.model_id,
            "messages": messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "stream": stream
        });
        if request.response_format == ModelResponseFormat::AgentActionJson {
            if let Some(obj) = body.as_object_mut() {
                obj.insert(
                    "response_format".to_string(),
                    json!({ "type": "json_object" }),
                );
            }
        }
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(self.api_key()?)
            .json(&body)
            .send()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        if !response.status().is_success() {
            return provider_http_error(self.connection.display_name.as_str(), response).await;
        }

        if stream {
            let on_chunk = on_chunk.ok_or_else(|| {
                AroError::Unexpected(
                    "stream callback is required for streaming generation".to_string(),
                )
            })?;
            parse_openai_sse(response, on_chunk)
                .await
                .map(|content| ModelGeneration {
                    content,
                    provider_detail: format!("{} provider (stream)", self.connection.display_name),
                    token_estimate: None,
                })
        } else {
            let body: OpenAiChatResponse = response
                .json()
                .await
                .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;
            let content = body
                .choices
                .first()
                .map(|choice| {
                    let text = choice.message.content.clone().unwrap_or_default();
                    let reasoning = choice
                        .message
                        .reasoning_content
                        .as_deref()
                        .or(choice.message.reasoning.as_deref());
                    if let Some(r) = reasoning.filter(|r| !r.trim().is_empty()) {
                        if text.is_empty() {
                            format!("<think>{}</think>\n", r.trim())
                        } else if !text.contains("<think>") {
                            format!("<think>{}</think>\n{}", r.trim(), text)
                        } else {
                            text
                        }
                    } else {
                        text
                    }
                })
                .unwrap_or_default();
            Ok(ModelGeneration {
                content,
                provider_detail: self.connection.display_name.clone(),
                token_estimate: body.usage.and_then(|usage| usage.total_tokens),
            })
        }
    }

    async fn generate_anthropic(
        &self,
        request: ModelGenerationRequest,
        stream: bool,
        on_chunk: Option<&mut (dyn FnMut(String) + Send)>,
    ) -> AroResult<ModelGeneration> {
        let endpoint = self.endpoint("/messages")?;
        let messages = request
            .messages
            .iter()
            .filter(|message| message.role != MessageRole::System)
            .map(anthropic_message)
            .collect::<Vec<_>>();
        let response = self
            .client
            .post(endpoint)
            .header("x-api-key", self.api_key()?)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.model_ref.model_id,
                "system": request.system_prompt,
                "messages": messages,
                "temperature": request.temperature,
                "max_tokens": request.max_tokens,
                "stream": stream
            }))
            .send()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        if !response.status().is_success() {
            return provider_http_error("Anthropic", response).await;
        }

        if stream {
            let on_chunk = on_chunk.ok_or_else(|| {
                AroError::Unexpected(
                    "stream callback is required for streaming generation".to_string(),
                )
            })?;
            parse_anthropic_sse(response, on_chunk)
                .await
                .map(|content| ModelGeneration {
                    content,
                    provider_detail: "Anthropic provider (stream)".to_string(),
                    token_estimate: None,
                })
        } else {
            let body: AnthropicMessageResponse = response
                .json()
                .await
                .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;
            Ok(ModelGeneration {
                content: body
                    .content
                    .into_iter()
                    .filter_map(|part| part.text)
                    .collect::<Vec<_>>()
                    .join(""),
                provider_detail: "Anthropic provider".to_string(),
                token_estimate: None,
            })
        }
    }

    async fn generate_google(
        &self,
        request: ModelGenerationRequest,
        stream: bool,
        on_chunk: Option<&mut (dyn FnMut(String) + Send)>,
    ) -> AroResult<ModelGeneration> {
        let suffix = if stream {
            format!(
                "/models/{}:streamGenerateContent?alt=sse",
                self.model_ref.model_id
            )
        } else {
            format!("/models/{}:generateContent", self.model_ref.model_id)
        };
        let endpoint = self.endpoint(&suffix)?;
        let contents = request
            .messages
            .iter()
            .filter(|message| message.role != MessageRole::System)
            .map(google_content)
            .collect::<Vec<_>>();
        let mut generation_config = json!({
            "temperature": request.temperature,
            "maxOutputTokens": request.max_tokens
        });
        if request.response_format == ModelResponseFormat::AgentActionJson {
            if let Some(obj) = generation_config.as_object_mut() {
                obj.insert("responseMimeType".to_string(), json!("application/json"));
            }
        }
        let response = self
            .client
            .post(endpoint)
            .header("x-goog-api-key", self.api_key()?)
            .json(&json!({
                "systemInstruction": { "parts": [{ "text": request.system_prompt }] },
                "contents": contents,
                "generationConfig": generation_config
            }))
            .send()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        if !response.status().is_success() {
            return provider_http_error("Google Gemini", response).await;
        }

        if stream {
            let on_chunk = on_chunk.ok_or_else(|| {
                AroError::Unexpected(
                    "stream callback is required for streaming generation".to_string(),
                )
            })?;
            parse_google_sse(response, on_chunk)
                .await
                .map(|content| ModelGeneration {
                    content,
                    provider_detail: "Google Gemini provider (stream)".to_string(),
                    token_estimate: None,
                })
        } else {
            let body: GoogleGenerateResponse = response
                .json()
                .await
                .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;
            Ok(ModelGeneration {
                content: google_response_text(&body),
                provider_detail: "Google Gemini provider".to_string(),
                token_estimate: None,
            })
        }
    }
}

#[async_trait]
impl ModelProvider for ModelRouter {
    async fn generate(&self, request: ModelGenerationRequest) -> AroResult<ModelGeneration> {
        match self.connection.kind {
            ModelProviderKind::Mock | ModelProviderKind::Ollama | ModelProviderKind::LlamaCpp => {
                self.local_provider()?.generate(request).await
            }
            ModelProviderKind::OpenAi
            | ModelProviderKind::Mistral
            | ModelProviderKind::OpenAiCompatible => {
                self.generate_openai_compatible(request, false, None).await
            }
            ModelProviderKind::Anthropic => self.generate_anthropic(request, false, None).await,
            ModelProviderKind::Google => self.generate_google(request, false, None).await,
        }
    }

    async fn generate_stream(
        &self,
        request: ModelGenerationRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> AroResult<ModelGeneration> {
        match self.connection.kind {
            ModelProviderKind::Mock | ModelProviderKind::Ollama | ModelProviderKind::LlamaCpp => {
                self.local_provider()?
                    .generate_stream(request, on_chunk)
                    .await
            }
            ModelProviderKind::OpenAi
            | ModelProviderKind::Mistral
            | ModelProviderKind::OpenAiCompatible => {
                self.generate_openai_compatible(request, true, Some(on_chunk))
                    .await
            }
            ModelProviderKind::Anthropic => {
                self.generate_anthropic(request, true, Some(on_chunk)).await
            }
            ModelProviderKind::Google => self.generate_google(request, true, Some(on_chunk)).await,
        }
    }

    async fn status(&self) -> RuntimeStatus {
        match self.connection.kind {
            ModelProviderKind::Mock | ModelProviderKind::Ollama | ModelProviderKind::LlamaCpp => {
                match self.local_provider() {
                    Ok(provider) => provider.status().await,
                    Err(err) => RuntimeStatus::unavailable(
                        self.connection.kind.clone(),
                        self.model_ref.model_id.clone(),
                        self.connection.endpoint.clone(),
                        err.to_string(),
                    ),
                }
            }
            _ => {
                let ready = !self
                    .api_key
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty();
                RuntimeStatus {
                    model_provider: self.connection.kind.clone(),
                    model_id: self.model_ref.model_id.clone(),
                    model_ready: ready,
                    voice_ready: false,
                    endpoint: self.connection.endpoint.clone(),
                    detail: if ready {
                        format!("{} key is configured.", self.connection.display_name)
                    } else {
                        format!("{} needs an API key.", self.connection.display_name)
                    },
                    checked_at: Utc::now(),
                }
            }
        }
    }

    fn temperature(&self) -> f32 {
        self.settings.temperature
    }

    fn max_tokens(&self) -> u32 {
        let profile = aro_core::resolve_model_profile(
            &self.model_ref.model_id,
            &self.model_ref.provider_kind,
        );
        if self.settings.max_tokens >= 512 {
            self.settings.max_tokens.max(profile.max_output_tokens)
        } else {
            profile.max_output_tokens
        }
    }
}

#[derive(Clone)]
pub enum LocalModelProvider {
    Mock(MockProvider),
    Ollama(OllamaProvider),
    LlamaCpp(LlamaCppProvider),
}

impl LocalModelProvider {
    pub fn from_settings(settings: &ModelSettings) -> AroResult<Self> {
        match settings.provider {
            ModelProviderKind::Mock => Ok(Self::Mock(MockProvider::new(settings.clone()))),
            ModelProviderKind::Ollama => {
                ensure_loopback_url(&settings.ollama_endpoint)?;
                Ok(Self::Ollama(OllamaProvider::new(settings.clone())))
            }
            ModelProviderKind::LlamaCpp => {
                ensure_loopback_url(&settings.llama_cpp_endpoint)?;
                Ok(Self::LlamaCpp(LlamaCppProvider::new(settings.clone())))
            }
            _ => Err(AroError::Configuration(format!(
                "{} is not a local provider",
                settings.provider.display_name()
            ))),
        }
    }
}

#[async_trait]
impl ModelProvider for LocalModelProvider {
    async fn generate(&self, request: ModelGenerationRequest) -> AroResult<ModelGeneration> {
        match self {
            Self::Mock(provider) => provider.generate(request).await,
            Self::Ollama(provider) => provider.generate(request).await,
            Self::LlamaCpp(provider) => provider.generate(request).await,
        }
    }

    async fn generate_stream(
        &self,
        request: ModelGenerationRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> AroResult<ModelGeneration> {
        match self {
            Self::Mock(provider) => provider.generate_stream(request, on_chunk).await,
            Self::Ollama(provider) => provider.generate_stream(request, on_chunk).await,
            Self::LlamaCpp(provider) => provider.generate_stream(request, on_chunk).await,
        }
    }

    async fn status(&self) -> RuntimeStatus {
        match self {
            Self::Mock(provider) => provider.status().await,
            Self::Ollama(provider) => provider.status().await,
            Self::LlamaCpp(provider) => provider.status().await,
        }
    }

    fn temperature(&self) -> f32 {
        match self {
            Self::Mock(provider) => provider.temperature(),
            Self::Ollama(provider) => provider.temperature(),
            Self::LlamaCpp(provider) => provider.temperature(),
        }
    }

    fn max_tokens(&self) -> u32 {
        match self {
            Self::Mock(provider) => provider.max_tokens(),
            Self::Ollama(provider) => provider.max_tokens(),
            Self::LlamaCpp(provider) => provider.max_tokens(),
        }
    }
}

#[derive(Clone)]
pub struct MockProvider {
    settings: ModelSettings,
}

impl MockProvider {
    pub fn new(settings: ModelSettings) -> Self {
        Self { settings }
    }
}

#[async_trait]
impl ModelProvider for MockProvider {
    async fn generate(&self, request: ModelGenerationRequest) -> AroResult<ModelGeneration> {
        let content = mock_response(&request);
        Ok(ModelGeneration {
            content,
            provider_detail: "mock local provider".to_string(),
            token_estimate: Some(72),
        })
    }

    async fn generate_stream(
        &self,
        request: ModelGenerationRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> AroResult<ModelGeneration> {
        let content = mock_response(&request);

        let words = content.split_inclusive(' ');
        for word in words {
            on_chunk(word.to_string());
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }

        Ok(ModelGeneration {
            content,
            provider_detail: "mock local provider (stream)".to_string(),
            token_estimate: Some(72),
        })
    }

    async fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            model_provider: ModelProviderKind::Mock,
            model_id: self.settings.model_id.clone(),
            model_ready: true,
            voice_ready: false,
            endpoint: None,
            detail: "Mock provider is ready for UI and workflow development.".to_string(),
            checked_at: Utc::now(),
        }
    }

    fn temperature(&self) -> f32 {
        self.settings.temperature
    }

    fn max_tokens(&self) -> u32 {
        self.settings.max_tokens
    }
}

fn mock_response(request: &ModelGenerationRequest) -> String {
    let mode = format!("{:?}", request.mode).to_lowercase();
    let user_input = compact_model_echo(&request.user_input, 180);
    let wants_agent_json = request
        .system_prompt
        .contains("Return exactly one JSON object");
    let has_web_context = request.system_prompt.contains("web-page")
        || request.system_prompt.contains("web-search-result")
        || request.system_prompt.contains("tool-error");
    if wants_agent_json {
        if has_web_context {
            let content = if looks_french(&request.user_input) {
                format!(
                    "J'ai consulte les sources web disponibles dans le contexte et je peux repondre a partir de ces extraits pour : \"{user_input}\"."
                )
            } else {
                format!(
                    "I used the available web sources in context and can answer from those excerpts for: \"{user_input}\"."
                )
            };
            return json!({ "type": "final", "content": content }).to_string();
        }
        if let Some(url) = extract_urls(&request.user_input, 1).into_iter().next() {
            return json!({
                "type": "tool",
                "toolId": TOOL_CORE_WEB_PAGE_READ,
                "input": { "url": url, "maxChars": 12000 },
                "reason": "Need to read the user-provided page before answering."
            })
            .to_string();
        }
        if web_request_likely(&request.user_input) {
            return json!({
                "type": "tool",
                "toolId": TOOL_CORE_SEARCH_WEB,
                "input": { "query": request.user_input, "limit": 5 },
                "reason": "Need current public web context before answering."
            })
            .to_string();
        }
        let content = if looks_french(&request.user_input) {
            format!(
                "ARO est pret en mode de developpement local. J'utilise les consignes du mode {mode} et j'ai bien recu : \"{user_input}\"."
            )
        } else {
            format!(
                "ARO is ready in local development mode. I am using the {mode} instructions and received: \"{user_input}\"."
            )
        };
        return json!({ "type": "final", "content": content }).to_string();
    }
    if looks_french(&request.user_input) {
        format!(
            "ARO est prêt en mode de développement local. J'utilise les consignes du mode {mode} et j'ai bien reçu : \"{user_input}\". Branche Ollama ou llama.cpp pour obtenir une vraie réponse modèle avec la même identité ARO."
        )
    } else {
        format!(
            "ARO is ready in local development mode. I am using the {mode} instructions and received: \"{user_input}\". Connect Ollama or llama.cpp to get a real model response with the same ARO identity."
        )
    }
}

fn looks_french(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    [
        "bonjour", "salut", "merci", "comment", "pourquoi", "peux", "fais", "analyse", "projet",
        "reponds", "aide", "amelior",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn compact_model_echo(input: &str, max_chars: usize) -> String {
    let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        format!(
            "{}...",
            normalized.chars().take(max_chars).collect::<String>()
        )
    }
}

#[derive(Clone)]
pub struct OllamaProvider {
    settings: ModelSettings,
    client: Client,
}

impl OllamaProvider {
    pub fn new(settings: ModelSettings) -> Self {
        Self {
            settings,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    async fn generate_stream(
        &self,
        request: ModelGenerationRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> AroResult<ModelGeneration> {
        let endpoint = format!(
            "{}/api/chat",
            self.settings.ollama_endpoint.trim_end_matches('/')
        );
        ensure_loopback_url(&endpoint)?;

        let mut messages = vec![json!({
            "role": "system",
            "content": request.system_prompt,
        })];
        messages.extend(request.messages.iter().map(ollama_message));

        let mut body = json!({
            "model": self.settings.model_id,
            "messages": messages,
            "stream": true,
            // Garde le modele charge en memoire : les requetes suivantes
            // (mobile/web) demarrent sans le cout du chargement a froid.
            "keep_alive": "10m",
            "options": {
                "temperature": request.temperature,
                "num_predict": request.max_tokens
            }
        });
        if request.response_format == ModelResponseFormat::AgentActionJson {
            let model_lower = self.settings.model_id.to_lowercase();
            let is_reasoning_model = model_lower.contains("deepseek")
                || model_lower.contains("r1")
                || model_lower.contains("reason")
                || model_lower.contains("think")
                || model_lower.contains("qwq")
                || model_lower.contains("qwen");
            if !is_reasoning_model {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("format".to_string(), json!("json"));
                }
            }
        }
        let mut response = self
            .client
            .post(&endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let detail = response
                .text()
                .await
                .unwrap_or_else(|_| "no response body".to_string());
            return Err(AroError::RuntimeUnavailable(format!(
                "Ollama returned {status} for model '{}': {}",
                self.settings.model_id,
                clean_error_body(&detail)
            )));
        }

        let mut full_content = String::new();
        let mut eval_count = None;
        let mut line_buffer = String::new();
        let mut in_thinking = false;

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?
        {
            let text = String::from_utf8_lossy(&chunk);
            line_buffer.push_str(&text);

            while let Some(newline_idx) = line_buffer.find('\n') {
                let line = line_buffer[..newline_idx].to_string();
                line_buffer = line_buffer[newline_idx + 1..].to_string();

                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(ec) = val.get("eval_count").and_then(|v| v.as_u64()) {
                        eval_count = Some(ec as u32);
                    }

                    // Handle dedicated thinking delta if emitted by Ollama
                    if let Some(thinking_chunk) = val
                        .pointer("/message/thinking")
                        .or_else(|| val.pointer("/message/thought"))
                        .and_then(|v| v.as_str())
                    {
                        if !thinking_chunk.is_empty() {
                            if !in_thinking {
                                in_thinking = true;
                                full_content.push_str("<think>");
                                on_chunk("<think>".to_string());
                            }
                            full_content.push_str(thinking_chunk);
                            on_chunk(thinking_chunk.to_string());
                        }
                    }

                    // Handle standard content delta
                    if let Some(content) = val.pointer("/message/content").and_then(|v| v.as_str())
                    {
                        if !content.is_empty() {
                            if in_thinking {
                                in_thinking = false;
                                full_content.push_str("</think>\n");
                                on_chunk("</think>\n".to_string());
                            }
                            full_content.push_str(content);
                            on_chunk(content.to_string());
                        }
                    }
                }
            }
        }

        if in_thinking {
            full_content.push_str("</think>\n");
            on_chunk("</think>\n".to_string());
        }

        Ok(ModelGeneration {
            content: full_content,
            provider_detail: "ollama local provider (stream)".to_string(),
            token_estimate: eval_count,
        })
    }

    async fn generate(&self, request: ModelGenerationRequest) -> AroResult<ModelGeneration> {
        let endpoint = format!(
            "{}/api/chat",
            self.settings.ollama_endpoint.trim_end_matches('/')
        );
        ensure_loopback_url(&endpoint)?;

        let mut messages = vec![json!({
            "role": "system",
            "content": request.system_prompt,
        })];
        messages.extend(request.messages.iter().map(ollama_message));

        let mut body = json!({
            "model": self.settings.model_id,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": request.temperature,
                "num_predict": request.max_tokens
            }
        });
        if request.response_format == ModelResponseFormat::AgentActionJson {
            let model_lower = self.settings.model_id.to_lowercase();
            let is_reasoning_model = model_lower.contains("deepseek")
                || model_lower.contains("r1")
                || model_lower.contains("reason")
                || model_lower.contains("think")
                || model_lower.contains("qwq")
                || model_lower.contains("qwen");
            if !is_reasoning_model {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("format".to_string(), json!("json"));
                }
            }
        }

        let response = self
            .client
            .post(&endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let detail = response
                .text()
                .await
                .unwrap_or_else(|_| "no response body".to_string());
            return Err(AroError::RuntimeUnavailable(format!(
                "Ollama returned {status} for model '{}': {}",
                self.settings.model_id,
                clean_error_body(&detail)
            )));
        }

        let body: OllamaChatResponse = response
            .json()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        let raw_content = body.message.content.trim().to_string();
        let thinking = body.message.thinking.or(body.message.thought);
        let content = if let Some(t) = thinking.filter(|t| !t.trim().is_empty()) {
            if raw_content.is_empty() {
                format!("<think>{}</think>\n", t.trim())
            } else if !raw_content.contains("<think>") {
                format!("<think>{}</think>\n{}", t.trim(), raw_content)
            } else {
                raw_content
            }
        } else {
            raw_content
        };

        Ok(ModelGeneration {
            content,
            provider_detail: "ollama local provider".to_string(),
            token_estimate: body.eval_count,
        })
    }

    async fn status(&self) -> RuntimeStatus {
        let endpoint = format!(
            "{}/api/tags",
            self.settings.ollama_endpoint.trim_end_matches('/')
        );
        if let Err(err) = ensure_loopback_url(&endpoint) {
            return RuntimeStatus::unavailable(
                ModelProviderKind::Ollama,
                self.settings.model_id.clone(),
                Some(endpoint),
                err.to_string(),
            );
        }

        match self.client.get(&endpoint).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<OllamaTagsResponse>().await {
                    Ok(tags) => {
                        let installed = tags
                            .models
                            .iter()
                            .map(|model| model.name.as_str())
                            .collect::<Vec<_>>();
                        let has_model =
                            installed.iter().any(|name| *name == self.settings.model_id);
                        RuntimeStatus {
                            model_provider: ModelProviderKind::Ollama,
                            model_id: self.settings.model_id.clone(),
                            model_ready: has_model,
                            voice_ready: false,
                            endpoint: Some(self.settings.ollama_endpoint.clone()),
                            detail: if has_model {
                                format!("Ollama is ready with {}.", self.settings.model_id)
                            } else if installed.is_empty() {
                                format!(
                                "Ollama is reachable, but no models are installed. Run: ollama pull {}",
                                self.settings.model_id
                            )
                            } else {
                                format!(
                                "Ollama is reachable, but '{}' is not installed. Installed models: {}.",
                                self.settings.model_id,
                                installed.join(", ")
                            )
                            },
                            checked_at: Utc::now(),
                        }
                    }
                    Err(err) => RuntimeStatus::unavailable(
                        ModelProviderKind::Ollama,
                        self.settings.model_id.clone(),
                        Some(endpoint),
                        format!("Ollama tags response could not be read: {err}"),
                    ),
                }
            }
            Ok(response) => RuntimeStatus::unavailable(
                ModelProviderKind::Ollama,
                self.settings.model_id.clone(),
                Some(endpoint),
                format!("Ollama returned {}", response.status()),
            ),
            Err(err) => RuntimeStatus::unavailable(
                ModelProviderKind::Ollama,
                self.settings.model_id.clone(),
                Some(endpoint),
                err.to_string(),
            ),
        }
    }

    fn temperature(&self) -> f32 {
        self.settings.temperature
    }

    fn max_tokens(&self) -> u32 {
        let profile = aro_core::resolve_model_profile(
            &self.settings.model_id,
            &ModelProviderKind::Ollama,
        );
        if self.settings.max_tokens >= 512 {
            self.settings.max_tokens.max(profile.max_output_tokens)
        } else {
            profile.max_output_tokens
        }
    }
}

#[derive(Clone)]
pub struct LlamaCppProvider {
    settings: ModelSettings,
    client: Client,
}

impl LlamaCppProvider {
    pub fn new(settings: ModelSettings) -> Self {
        Self {
            settings,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ModelProvider for LlamaCppProvider {
    async fn generate_stream(
        &self,
        request: ModelGenerationRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> AroResult<ModelGeneration> {
        let endpoint = format!(
            "{}/v1/chat/completions",
            self.settings.llama_cpp_endpoint.trim_end_matches('/')
        );
        ensure_loopback_url(&endpoint)?;

        let mut messages = vec![json!({
            "role": "system",
            "content": request.system_prompt,
        })];
        messages.extend(request.messages.iter().map(openai_message));

        let mut body = json!({
            "model": self.settings.model_id,
            "messages": messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "stream": true
        });
        if request.response_format == ModelResponseFormat::AgentActionJson {
            let model_lower = self.settings.model_id.to_lowercase();
            let is_reasoning_model = model_lower.contains("deepseek")
                || model_lower.contains("r1")
                || model_lower.contains("reason")
                || model_lower.contains("think")
                || model_lower.contains("qwq")
                || model_lower.contains("qwen");
            if !is_reasoning_model {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert(
                        "response_format".to_string(),
                        json!({ "type": "json_object" }),
                    );
                }
            }
        }
        let response = self
            .client
            .post(&endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let detail = response
                .text()
                .await
                .unwrap_or_else(|_| "no response body".to_string());
            return Err(AroError::RuntimeUnavailable(format!(
                "llama.cpp returned {status}: {}",
                clean_error_body(&detail)
            )));
        }

        let full_content = parse_openai_sse(response, on_chunk).await?;

        Ok(ModelGeneration {
            content: full_content,
            provider_detail: "llama.cpp local provider (stream)".to_string(),
            token_estimate: None,
        })
    }

    async fn generate(&self, request: ModelGenerationRequest) -> AroResult<ModelGeneration> {
        let endpoint = format!(
            "{}/v1/chat/completions",
            self.settings.llama_cpp_endpoint.trim_end_matches('/')
        );
        ensure_loopback_url(&endpoint)?;

        let mut messages = vec![json!({
            "role": "system",
            "content": request.system_prompt,
        })];
        messages.extend(request.messages.iter().map(openai_message));

        let response = self
            .client
            .post(&endpoint)
            .json(&json!({
                "model": self.settings.model_id,
                "messages": messages,
                "temperature": request.temperature,
                "max_tokens": request.max_tokens
            }))
            .send()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let detail = response
                .text()
                .await
                .unwrap_or_else(|_| "no response body".to_string());
            return Err(AroError::RuntimeUnavailable(format!(
                "llama.cpp returned {status}: {}",
                clean_error_body(&detail)
            )));
        }

        let body: OpenAiChatResponse = response
            .json()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;
        let content = body
            .choices
            .first()
            .map(|choice| {
                let text = choice.message.content.clone().unwrap_or_default();
                let reasoning = choice
                    .message
                    .reasoning_content
                    .as_deref()
                    .or(choice.message.reasoning.as_deref());
                if let Some(r) = reasoning.filter(|r| !r.trim().is_empty()) {
                    if text.is_empty() {
                        format!("<think>{}</think>\n", r.trim())
                    } else if !text.contains("<think>") {
                        format!("<think>{}</think>\n{}", r.trim(), text)
                    } else {
                        text
                    }
                } else {
                    text
                }
            })
            .unwrap_or_default();

        Ok(ModelGeneration {
            content,
            provider_detail: "llama.cpp local provider".to_string(),
            token_estimate: body.usage.and_then(|usage| usage.total_tokens),
        })
    }

    async fn status(&self) -> RuntimeStatus {
        let endpoint = format!(
            "{}/health",
            self.settings.llama_cpp_endpoint.trim_end_matches('/')
        );
        if let Err(err) = ensure_loopback_url(&endpoint) {
            return RuntimeStatus::unavailable(
                ModelProviderKind::LlamaCpp,
                self.settings.model_id.clone(),
                Some(endpoint),
                err.to_string(),
            );
        }

        match self.client.get(&endpoint).send().await {
            Ok(response) if response.status().is_success() => RuntimeStatus {
                model_provider: ModelProviderKind::LlamaCpp,
                model_id: self.settings.model_id.clone(),
                model_ready: true,
                voice_ready: false,
                endpoint: Some(self.settings.llama_cpp_endpoint.clone()),
                detail: "llama.cpp server is reachable on loopback.".to_string(),
                checked_at: Utc::now(),
            },
            Ok(response) => RuntimeStatus::unavailable(
                ModelProviderKind::LlamaCpp,
                self.settings.model_id.clone(),
                Some(endpoint),
                format!("llama.cpp returned {}", response.status()),
            ),
            Err(err) => RuntimeStatus::unavailable(
                ModelProviderKind::LlamaCpp,
                self.settings.model_id.clone(),
                Some(endpoint),
                err.to_string(),
            ),
        }
    }

    fn temperature(&self) -> f32 {
        self.settings.temperature
    }

    fn max_tokens(&self) -> u32 {
        let profile = aro_core::resolve_model_profile(
            &self.settings.model_id,
            &ModelProviderKind::LlamaCpp,
        );
        if self.settings.max_tokens >= 512 {
            self.settings.max_tokens.max(profile.max_output_tokens)
        } else {
            profile.max_output_tokens
        }
    }
}

fn role_as_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
    }
}

fn ollama_message(message: &ChatMessage) -> serde_json::Value {
    json!({
        "role": role_as_str(&message.role),
        "content": message.content,
    })
}

fn openai_message(message: &ChatMessage) -> serde_json::Value {
    json!({
        "role": role_as_str(&message.role),
        "content": message.content,
    })
}

fn anthropic_message(message: &ChatMessage) -> serde_json::Value {
    let role = match message.role {
        MessageRole::Assistant => "assistant",
        _ => "user",
    };
    json!({
        "role": role,
        "content": message.content,
    })
}

fn google_content(message: &ChatMessage) -> serde_json::Value {
    let role = match message.role {
        MessageRole::Assistant => "model",
        _ => "user",
    };
    json!({
        "role": role,
        "parts": [{ "text": message.content }],
    })
}

async fn provider_http_error<T>(provider: &str, response: reqwest::Response) -> AroResult<T> {
    let status = response.status();
    let detail = response
        .text()
        .await
        .unwrap_or_else(|_| "no response body".to_string());
    Err(AroError::RuntimeUnavailable(format!(
        "{provider} returned {status}: {}",
        clean_error_body(&detail)
    )))
}

async fn parse_openai_sse(
    mut response: reqwest::Response,
    on_chunk: &mut (dyn FnMut(String) + Send),
) -> AroResult<String> {
    let mut in_thinking = false;
    let mut result = parse_sse_lines(&mut response, |data, full_content| {
        if data == "[DONE]" {
            return;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(reasoning) = value
                .pointer("/choices/0/delta/reasoning_content")
                .or_else(|| value.pointer("/choices/0/delta/reasoning"))
                .and_then(|value| value.as_str())
            {
                if !reasoning.is_empty() {
                    if !in_thinking {
                        in_thinking = true;
                        full_content.push_str("<think>");
                        on_chunk("<think>".to_string());
                    }
                    full_content.push_str(reasoning);
                    on_chunk(reasoning.to_string());
                }
            }

            if let Some(content) = value
                .pointer("/choices/0/delta/content")
                .and_then(|value| value.as_str())
            {
                if !content.is_empty() {
                    if in_thinking {
                        in_thinking = false;
                        full_content.push_str("</think>\n");
                        on_chunk("</think>\n".to_string());
                    }
                    full_content.push_str(content);
                    on_chunk(content.to_string());
                }
            }
        }
    })
    .await?;

    if in_thinking {
        result.push_str("</think>\n");
        on_chunk("</think>\n".to_string());
    }

    Ok(result)
}

async fn parse_anthropic_sse(
    mut response: reqwest::Response,
    on_chunk: &mut (dyn FnMut(String) + Send),
) -> AroResult<String> {
    let mut in_thinking = false;
    let mut result = parse_sse_lines(&mut response, |data, full_content| {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
            let block_type = value.get("type").and_then(|value| value.as_str());
            if block_type == Some("content_block_delta") {
                if let Some(thinking) = value.pointer("/delta/thinking").and_then(|value| value.as_str()) {
                    if !thinking.is_empty() {
                        if !in_thinking {
                            in_thinking = true;
                            full_content.push_str("<think>");
                            on_chunk("<think>".to_string());
                        }
                        full_content.push_str(thinking);
                        on_chunk(thinking.to_string());
                    }
                }
                if let Some(content) = value
                    .pointer("/delta/text")
                    .and_then(|value| value.as_str())
                {
                    if !content.is_empty() {
                        if in_thinking {
                            in_thinking = false;
                            full_content.push_str("</think>\n");
                            on_chunk("</think>\n".to_string());
                        }
                        full_content.push_str(content);
                        on_chunk(content.to_string());
                    }
                }
            }
        }
    })
    .await?;

    if in_thinking {
        result.push_str("</think>\n");
        on_chunk("</think>\n".to_string());
    }

    Ok(result)
}

async fn parse_google_sse(
    mut response: reqwest::Response,
    on_chunk: &mut (dyn FnMut(String) + Send),
) -> AroResult<String> {
    parse_sse_lines(&mut response, |data, full_content| {
        if let Ok(value) = serde_json::from_str::<GoogleGenerateResponse>(data) {
            let content = google_response_text(&value);
            if !content.is_empty() {
                full_content.push_str(&content);
                on_chunk(content);
            }
        }
    })
    .await
}

async fn parse_sse_lines(
    response: &mut reqwest::Response,
    mut handle_data: impl FnMut(&str, &mut String),
) -> AroResult<String> {
    let mut full_content = String::new();
    let mut line_buffer = String::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?
    {
        let text = String::from_utf8_lossy(&chunk);
        line_buffer.push_str(&text);
        while let Some(newline_idx) = line_buffer.find('\n') {
            let line = line_buffer[..newline_idx].to_string();
            line_buffer = line_buffer[newline_idx + 1..].to_string();
            let trimmed = line.trim();
            if let Some(data) = trimmed.strip_prefix("data: ") {
                handle_data(data.trim(), &mut full_content);
            }
        }
    }
    Ok(full_content)
}

fn google_response_text(body: &GoogleGenerateResponse) -> String {
    body.candidates
        .iter()
        .flat_map(|candidate| candidate.content.parts.iter())
        .filter_map(|part| part.text.as_ref())
        .cloned()
        .collect::<Vec<_>>()
        .join("")
}

fn clean_error_body(raw: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(error) = value.get("error").and_then(|value| value.as_str()) {
            return error.to_string();
        }
    }

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "empty response body".to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelTag>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelTag {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    thought: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleGenerateResponse {
    #[serde(default)]
    candidates: Vec<GoogleCandidate>,
}

#[derive(Debug, Deserialize)]
struct GoogleCandidate {
    content: GoogleContent,
}

#[derive(Debug, Deserialize)]
struct GoogleContent {
    #[serde(default)]
    parts: Vec<GooglePart>,
}

#[derive(Debug, Deserialize)]
struct GooglePart {
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use aro_core::ModelProviderConnection;

    #[test]
    fn router_requires_api_key_for_remote_provider() {
        let mut settings = ModelSettings::default();
        let provider = ModelProviderConnection::new(ModelProviderKind::OpenAi, true);
        let provider_id = provider.id.clone();
        let model_id = provider.models[0].model_id.clone();
        settings.upsert_provider(provider);
        settings.select_model(&provider_id, &model_id);

        let err = match ModelRouter::from_active(&settings, None) {
            Ok(_) => panic!("remote provider should require an API key"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("API key"));
    }

    #[test]
    fn router_accepts_local_active_model_without_api_key() {
        let mut settings = ModelSettings::default();
        settings.select_model("mock-local", "mock");

        assert!(ModelRouter::from_active(&settings, None).is_ok());
    }

    #[test]
    fn cached_catalog_keeps_model_scoped_to_provider() {
        let mut settings = ModelSettings::default();
        assert!(settings.select_model("ollama-local", "gemma3:1b"));
        let mut remote = ModelProviderConnection::new(ModelProviderKind::OpenAi, true);
        remote.auth_configured = true;
        settings.upsert_provider(remote);
        assert!(settings.select_model("openai", "gpt-5.4-mini"));

        let ollama = settings.connection("ollama-local").unwrap();
        let models = cached_models(ollama, false, false);

        assert!(models
            .iter()
            .all(|model| model.provider_id == "ollama-local"));
        assert!(models
            .iter()
            .all(|model| model.provider_kind == ModelProviderKind::Ollama));
        assert!(models.iter().all(|model| !model.ready));
        assert!(!models.iter().any(|model| model.model_id == "gpt-5.4-mini"));
    }

    #[test]
    fn router_rejects_insecure_remote_endpoint() {
        let mut settings = ModelSettings::default();
        let mut provider = ModelProviderConnection::new(ModelProviderKind::OpenAi, true);
        provider.endpoint = Some("http://api.example.com/v1".to_string());
        let provider_id = provider.id.clone();
        let model_id = provider.models[0].model_id.clone();
        settings.upsert_provider(provider);
        settings.select_model(&provider_id, &model_id);

        let err = match ModelRouter::from_active(&settings, Some("sk-test".to_string())) {
            Ok(_) => panic!("remote provider should require HTTPS"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("HTTPS"));
    }

    #[test]
    fn mock_response_matches_french_input() {
        let request = ModelGenerationRequest {
            mode: aro_core::AssistantMode::Chat,
            system_prompt: "You are ARO.".to_string(),
            messages: Vec::new(),
            user_input: "Salut, analyse le projet".to_string(),
            temperature: 0.7,
            max_tokens: 128,
            response_format: ModelResponseFormat::DirectText,
        };

        let content = mock_response(&request);

        assert!(content.contains("ARO est prêt"));
        assert!(content.contains("mode de développement local"));
    }

    #[test]
    fn mock_response_matches_english_input() {
        let request = ModelGenerationRequest {
            mode: aro_core::AssistantMode::Code,
            system_prompt: "You are ARO.".to_string(),
            messages: Vec::new(),
            user_input: "Please inspect the project".to_string(),
            temperature: 0.7,
            max_tokens: 128,
            response_format: ModelResponseFormat::DirectText,
        };

        let content = mock_response(&request);

        assert!(content.contains("ARO is ready"));
        assert!(content.contains("code instructions"));
    }
}
