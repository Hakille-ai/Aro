use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use crate::NotificationSettings;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ModelProviderKind {
    Mock,
    Ollama,
    LlamaCpp,
    #[serde(rename = "openai", alias = "open-ai")]
    OpenAi,
    Anthropic,
    Google,
    Mistral,
    #[serde(rename = "openai-compatible", alias = "open-ai-compatible")]
    OpenAiCompatible,
}

impl ModelProviderKind {
    pub fn is_local(&self) -> bool {
        matches!(
            self,
            ModelProviderKind::Mock | ModelProviderKind::Ollama | ModelProviderKind::LlamaCpp
        )
    }

    pub fn requires_api_key(&self) -> bool {
        !self.is_local()
    }

    pub fn default_provider_id(&self) -> &'static str {
        match self {
            ModelProviderKind::Mock => "mock-local",
            ModelProviderKind::Ollama => "ollama-local",
            ModelProviderKind::LlamaCpp => "llama-cpp-local",
            ModelProviderKind::OpenAi => "openai",
            ModelProviderKind::Anthropic => "anthropic",
            ModelProviderKind::Google => "google",
            ModelProviderKind::Mistral => "mistral",
            ModelProviderKind::OpenAiCompatible => "openai-compatible",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ModelProviderKind::Mock => "Mock",
            ModelProviderKind::Ollama => "Ollama",
            ModelProviderKind::LlamaCpp => "llama.cpp",
            ModelProviderKind::OpenAi => "OpenAI",
            ModelProviderKind::Anthropic => "Anthropic",
            ModelProviderKind::Google => "Google Gemini",
            ModelProviderKind::Mistral => "Mistral",
            ModelProviderKind::OpenAiCompatible => "OpenAI-compatible",
        }
    }

    pub fn default_endpoint(&self) -> Option<&'static str> {
        match self {
            ModelProviderKind::Mock => None,
            ModelProviderKind::Ollama => Some("http://127.0.0.1:11434"),
            ModelProviderKind::LlamaCpp => Some("http://127.0.0.1:8080"),
            ModelProviderKind::OpenAi => Some("https://api.openai.com/v1"),
            ModelProviderKind::Anthropic => Some("https://api.anthropic.com/v1"),
            ModelProviderKind::Google => Some("https://generativelanguage.googleapis.com/v1beta"),
            ModelProviderKind::Mistral => Some("https://api.mistral.ai/v1"),
            ModelProviderKind::OpenAiCompatible => Some("https://api.example.com/v1"),
        }
    }

    pub fn default_model_id(&self) -> &'static str {
        match self {
            ModelProviderKind::Mock => "mock",
            ModelProviderKind::Ollama => "gemma3:1b",
            ModelProviderKind::LlamaCpp => "local-model",
            ModelProviderKind::OpenAi => "gpt-5.4-mini",
            ModelProviderKind::Anthropic => "claude-sonnet-5",
            ModelProviderKind::Google => "gemini-3.5-flash",
            ModelProviderKind::Mistral => "mistral-small-4",
            ModelProviderKind::OpenAiCompatible => "custom-model",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ModelFallbackPolicy {
    #[default]
    LocalFirst,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelRef {
    pub provider_id: String,
    pub provider_kind: ModelProviderKind,
    pub model_id: String,
    pub label: String,
    pub family: Option<String>,
    pub local: bool,
    pub installed: bool,
    pub ready: bool,
}

impl ModelRef {
    pub fn new(
        provider_id: impl Into<String>,
        provider_kind: ModelProviderKind,
        model_id: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        let local = provider_kind.is_local();
        Self {
            provider_id: provider_id.into(),
            provider_kind,
            model_id: model_id.into(),
            label: label.into(),
            family: None,
            local,
            installed: true,
            ready: true,
        }
    }
}

impl Default for ModelRef {
    fn default() -> Self {
        Self::new(
            ModelProviderKind::Mock.default_provider_id(),
            ModelProviderKind::Mock,
            "mock",
            "Mock",
        )
    }
}

/// Dynamic capability and token sizing profile inferred for a specific model and provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub context_window: usize,
    pub max_output_tokens: u32,
    pub supports_reasoning: bool,
    pub default_temperature: f32,
}

pub fn resolve_model_profile(model_id: &str, provider: &ModelProviderKind) -> ModelProfile {
    let id_lower = model_id.to_lowercase();
    let is_reasoning = id_lower.contains("coder")
        || id_lower.contains("qwen3")
        || id_lower.contains("deepseek-r1")
        || id_lower.contains("r1")
        || id_lower.contains("o1")
        || id_lower.contains("o3")
        || id_lower.contains("reasoner")
        || id_lower.contains("thinking")
        || id_lower.contains("qvq");

    let context_window = match provider {
        ModelProviderKind::Google => {
            if id_lower.contains("1.5") || id_lower.contains("2.0") || id_lower.contains("3.") {
                131_072
            } else {
                32_768
            }
        }
        ModelProviderKind::Anthropic => 200_000,
        ModelProviderKind::OpenAi => 128_000,
        ModelProviderKind::Mistral => 32_768,
        ModelProviderKind::OpenAiCompatible => {
            if id_lower.contains("deepseek") || id_lower.contains("qwen") || id_lower.contains("kimi") {
                65_536
            } else {
                32_768
            }
        }
        ModelProviderKind::Ollama | ModelProviderKind::LlamaCpp => {
            if id_lower.contains("qwen") || id_lower.contains("deepseek") || id_lower.contains("llama-3") {
                32_768
            } else if id_lower.contains("gemma") || id_lower.contains("phi") {
                8_192
            } else {
                16_384
            }
        }
        ModelProviderKind::Mock => 8_192,
    };

    let max_output = if is_reasoning {
        8_192
    } else {
        match provider {
            ModelProviderKind::Google | ModelProviderKind::Anthropic | ModelProviderKind::OpenAi => 8_192,
            ModelProviderKind::OpenAiCompatible => 8_192,
            ModelProviderKind::Ollama | ModelProviderKind::LlamaCpp => {
                if id_lower.contains("mini") || id_lower.contains("1b") {
                    4_096
                } else {
                    8_192
                }
            }
            ModelProviderKind::Mistral => 4_096,
            ModelProviderKind::Mock => 2_048,
        }
    };

    let default_temp = if is_reasoning { 0.6 } else { 0.7 };

    ModelProfile {
        context_window,
        max_output_tokens: max_output,
        supports_reasoning: is_reasoning,
        default_temperature: default_temp,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderConnection {
    pub id: String,
    pub kind: ModelProviderKind,
    pub display_name: String,
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub auth_configured: bool,
    pub models: Vec<ModelRef>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ModelProviderConnection {
    pub fn new(kind: ModelProviderKind, enabled: bool) -> Self {
        let now = Utc::now();
        let id = kind.default_provider_id().to_string();
        let default_model = ModelRef::new(
            id.clone(),
            kind.clone(),
            kind.default_model_id(),
            kind.default_model_id(),
        );
        Self {
            id,
            display_name: kind.display_name().to_string(),
            endpoint: kind.default_endpoint().map(str::to_string),
            auth_configured: false,
            kind,
            enabled,
            models: vec![default_model],
            created_at: now,
            updated_at: now,
        }
    }

    pub fn normalized(mut self) -> Self {
        if self.display_name.trim().is_empty() {
            self.display_name = self.kind.display_name().to_string();
        }
        if self
            .endpoint
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            self.endpoint = self.kind.default_endpoint().map(str::to_string);
        }
        if self.models.is_empty() {
            self.models.push(ModelRef::new(
                self.id.clone(),
                self.kind.clone(),
                self.kind.default_model_id(),
                self.kind.default_model_id(),
            ));
        }
        for model in &mut self.models {
            model.provider_id = self.id.clone();
            model.provider_kind = self.kind.clone();
            model.local = self.kind.is_local();
            if model.label.trim().is_empty() {
                model.label = model.model_id.clone();
            }
        }
        self
    }
}

pub fn default_model_providers() -> Vec<ModelProviderConnection> {
    vec![
        ModelProviderConnection::new(ModelProviderKind::Mock, true),
        ModelProviderConnection::new(ModelProviderKind::Ollama, true),
        ModelProviderConnection::new(ModelProviderKind::LlamaCpp, true),
    ]
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSettings {
    pub providers: Vec<ModelProviderConnection>,
    pub active_model_ref: ModelRef,
    pub fallback_policy: ModelFallbackPolicy,
    // Legacy compatibility fields are kept in sync so existing API and UI clients do not break.
    pub provider: ModelProviderKind,
    pub model_id: String,
    pub ollama_endpoint: String,
    pub llama_cpp_endpoint: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl Default for ModelSettings {
    fn default() -> Self {
        let providers = default_model_providers();
        let active_model_ref = ModelRef::new(
            ModelProviderKind::Mock.default_provider_id(),
            ModelProviderKind::Mock,
            "gemma3:1b",
            "gemma3:1b",
        );
        Self {
            providers,
            active_model_ref,
            fallback_policy: ModelFallbackPolicy::LocalFirst,
            provider: ModelProviderKind::Mock,
            model_id: "gemma3:1b".to_string(),
            ollama_endpoint: "http://127.0.0.1:11434".to_string(),
            llama_cpp_endpoint: "http://127.0.0.1:8080".to_string(),
            temperature: 0.7,
            max_tokens: 768,
        }
    }
}

impl<'de> Deserialize<'de> for ModelSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            providers: Option<Vec<ModelProviderConnection>>,
            active_model_ref: Option<ModelRef>,
            fallback_policy: Option<ModelFallbackPolicy>,
            provider: Option<ModelProviderKind>,
            model_id: Option<String>,
            ollama_endpoint: Option<String>,
            llama_cpp_endpoint: Option<String>,
            temperature: Option<f32>,
            max_tokens: Option<u32>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let default = ModelSettings::default();
        let mut settings = ModelSettings {
            providers: wire.providers.unwrap_or_else(default_model_providers),
            active_model_ref: wire.active_model_ref.unwrap_or_else(|| {
                let provider = wire.provider.clone().unwrap_or(default.provider.clone());
                let model_id = wire
                    .model_id
                    .clone()
                    .unwrap_or_else(|| provider.default_model_id().to_string());
                ModelRef::new(
                    provider.default_provider_id(),
                    provider,
                    model_id.clone(),
                    model_id,
                )
            }),
            fallback_policy: wire.fallback_policy.unwrap_or_default(),
            provider: wire.provider.unwrap_or(default.provider),
            model_id: wire.model_id.unwrap_or(default.model_id),
            ollama_endpoint: wire.ollama_endpoint.unwrap_or(default.ollama_endpoint),
            llama_cpp_endpoint: wire
                .llama_cpp_endpoint
                .unwrap_or(default.llama_cpp_endpoint),
            temperature: wire.temperature.unwrap_or(default.temperature),
            max_tokens: wire.max_tokens.unwrap_or(default.max_tokens),
        };
        settings = settings.normalized();
        Ok(settings)
    }
}

impl ModelSettings {
    pub fn normalized(mut self) -> Self {
        if self.providers.is_empty() {
            self.providers = default_model_providers();
        }

        for provider in &mut self.providers {
            if provider.kind == ModelProviderKind::Ollama {
                provider.endpoint = Some(self.ollama_endpoint.clone());
            }
            if provider.kind == ModelProviderKind::LlamaCpp {
                provider.endpoint = Some(self.llama_cpp_endpoint.clone());
            }
        }
        self.providers = self
            .providers
            .into_iter()
            .map(ModelProviderConnection::normalized)
            .collect();

        let active_provider = self
            .providers
            .iter()
            .find(|provider| provider.id == self.active_model_ref.provider_id)
            .cloned()
            .or_else(|| self.fallback_connection());
        if let Some(provider) = &active_provider {
            if self.active_model_ref.provider_id != provider.id {
                self.active_model_ref = provider.models.first().cloned().unwrap_or_else(|| {
                    ModelRef::new(
                        provider.id.clone(),
                        provider.kind.clone(),
                        provider.kind.default_model_id(),
                        provider.kind.default_model_id(),
                    )
                });
            }
        }
        if let Some(active_provider) = active_provider {
            self.provider = active_provider.kind.clone();
            self.model_id = self.active_model_ref.model_id.clone();
            self.active_model_ref.provider_kind = active_provider.kind.clone();
            self.active_model_ref.local = active_provider.kind.is_local();
            if self.active_model_ref.label.trim().is_empty() {
                self.active_model_ref.label = self.active_model_ref.model_id.clone();
            }
            if active_provider.kind == ModelProviderKind::Ollama {
                if let Some(endpoint) = active_provider.endpoint {
                    self.ollama_endpoint = endpoint;
                }
            } else if active_provider.kind == ModelProviderKind::LlamaCpp {
                if let Some(endpoint) = active_provider.endpoint {
                    self.llama_cpp_endpoint = endpoint;
                }
            }
        }

        self
    }

    fn fallback_connection(&self) -> Option<ModelProviderConnection> {
        self.providers
            .iter()
            .find(|provider| provider.kind == ModelProviderKind::Mock && provider.enabled)
            .or_else(|| {
                self.providers
                    .iter()
                    .find(|provider| provider.kind.is_local() && provider.enabled)
            })
            .or_else(|| self.providers.iter().find(|provider| provider.enabled))
            .or_else(|| self.providers.first())
            .cloned()
    }

    pub fn active_connection(&self) -> Option<&ModelProviderConnection> {
        self.providers
            .iter()
            .find(|provider| provider.id == self.active_model_ref.provider_id)
    }

    pub fn connection(&self, provider_id: &str) -> Option<&ModelProviderConnection> {
        self.providers
            .iter()
            .find(|provider| provider.id == provider_id)
    }

    pub fn select_model(&mut self, provider_id: &str, model_id: &str) -> bool {
        let Some(provider) = self.connection(provider_id).cloned() else {
            return false;
        };
        let model = provider
            .models
            .iter()
            .find(|model| model.model_id == model_id)
            .cloned()
            .unwrap_or_else(|| {
                ModelRef::new(
                    provider.id.clone(),
                    provider.kind.clone(),
                    model_id,
                    model_id,
                )
            });
        self.active_model_ref = model;
        self.provider = provider.kind.clone();
        self.model_id = model_id.to_string();
        if provider.kind == ModelProviderKind::Ollama {
            if let Some(endpoint) = provider.endpoint {
                self.ollama_endpoint = endpoint;
            }
        } else if provider.kind == ModelProviderKind::LlamaCpp {
            if let Some(endpoint) = provider.endpoint {
                self.llama_cpp_endpoint = endpoint;
            }
        }
        true
    }

    pub fn upsert_provider(&mut self, provider: ModelProviderConnection) {
        let provider = provider.normalized();
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|existing| existing.id == provider.id)
        {
            *existing = provider;
        } else {
            self.providers.push(provider);
        }
        let normalized = self.clone().normalized();
        *self = normalized;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSettings {
    pub enabled: bool,
    pub speech_to_text: VoiceRuntimeKind,
    pub text_to_speech: VoiceRuntimeKind,
    pub whisper_binary: Option<String>,
    pub whisper_model_path: Option<String>,
    pub piper_binary: Option<String>,
    pub piper_voice_path: Option<String>,
    #[serde(default)]
    pub wake_word: WakeWordSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VoiceRuntimeKind {
    Disabled,
    WhisperCpp,
    Piper,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WakeWordRuntimeKind {
    #[default]
    Disabled,
    LocalModel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WakeWordSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub runtime: WakeWordRuntimeKind,
    #[serde(default)]
    pub model_path: Option<String>,
    #[serde(default = "default_wake_word_threshold")]
    pub threshold: f32,
}

impl Default for WakeWordSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            runtime: WakeWordRuntimeKind::Disabled,
            model_path: None,
            threshold: default_wake_word_threshold(),
        }
    }
}

fn default_wake_word_threshold() -> f32 {
    0.65
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            speech_to_text: VoiceRuntimeKind::Disabled,
            text_to_speech: VoiceRuntimeKind::Disabled,
            whisper_binary: None,
            whisper_model_path: None,
            piper_binary: None,
            piper_voice_path: None,
            wake_word: WakeWordSettings::default(),
        }
    }
}

impl VoiceSettings {
    pub fn status(&self) -> crate::VoiceStatus {
        crate::VoiceStatus::from_settings(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSettings {
    pub provider: String,
    /// Transient compatibility input. Search credentials are never serialized into settings;
    /// desktop clients persist them in the operating-system keyring instead.
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub auth_configured: bool,
    pub endpoint: Option<String>,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            provider: "google-scrape".to_string(),
            api_key: None,
            auth_configured: false,
            endpoint: None,
        }
    }
}

fn default_context_mode() -> String {
    "auto".to_string()
}

fn default_total_token_ceiling() -> usize {
    8192
}

fn default_system_budget() -> usize {
    800
}

fn default_semantic_budget() -> usize {
    1600
}

fn default_episodic_budget() -> usize {
    2000
}

fn default_working_budget() -> usize {
    2400
}

fn default_reserve_budget() -> usize {
    1392
}

fn default_compaction_interval() -> usize {
    10
}

fn default_max_working_turns() -> usize {
    8
}

fn default_top_k() -> usize {
    5
}

fn default_min_salience() -> f32 {
    0.2
}

fn default_decay_half_life_days() -> f32 {
    0.0 // 0.0 means unlimited retention / no decay
}

fn default_rrf_fts_weight() -> f32 {
    0.40
}

fn default_rrf_vec_weight() -> f32 {
    0.60
}

fn default_rrf_k() -> f32 {
    60.0
}

fn default_auto_memorize() -> bool {
    true
}

/// Comprehensive configuration for all cognitive memory, context token partitions, and RRF consensus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryConfigurationSettings {
    #[serde(default = "default_context_mode")]
    pub context_mode: String,
    #[serde(default = "default_total_token_ceiling")]
    pub total_token_ceiling: usize,
    #[serde(default = "default_system_budget")]
    pub system_budget: usize,
    #[serde(default = "default_semantic_budget")]
    pub semantic_budget: usize,
    #[serde(default = "default_episodic_budget")]
    pub episodic_budget: usize,
    #[serde(default = "default_working_budget")]
    pub working_budget: usize,
    #[serde(default = "default_reserve_budget")]
    pub reserve_budget: usize,
    #[serde(default = "default_compaction_interval")]
    pub compaction_interval: usize,
    #[serde(default = "default_max_working_turns")]
    pub max_working_turns: usize,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default = "default_min_salience")]
    pub min_salience_threshold: f32,
    #[serde(default = "default_decay_half_life_days")]
    pub decay_half_life_days: f32,
    #[serde(default = "default_rrf_fts_weight")]
    pub rrf_fts_weight: f32,
    #[serde(default = "default_rrf_vec_weight")]
    pub rrf_vec_weight: f32,
    #[serde(default = "default_rrf_k")]
    pub rrf_k: f32,
    #[serde(default = "default_auto_memorize")]
    pub auto_memorize: bool,
}

impl Default for MemoryConfigurationSettings {
    fn default() -> Self {
        Self {
            context_mode: default_context_mode(),
            total_token_ceiling: default_total_token_ceiling(),
            system_budget: default_system_budget(),
            semantic_budget: default_semantic_budget(),
            episodic_budget: default_episodic_budget(),
            working_budget: default_working_budget(),
            reserve_budget: default_reserve_budget(),
            compaction_interval: default_compaction_interval(),
            max_working_turns: default_max_working_turns(),
            top_k: default_top_k(),
            min_salience_threshold: default_min_salience(),
            decay_half_life_days: default_decay_half_life_days(),
            rrf_fts_weight: default_rrf_fts_weight(),
            rrf_vec_weight: default_rrf_vec_weight(),
            rrf_k: default_rrf_k(),
            auto_memorize: default_auto_memorize(),
        }
    }
}

impl MemoryConfigurationSettings {
    pub fn to_context_budget(&self) -> crate::ContextBudget {
        crate::ContextBudget {
            system_budget: self.system_budget,
            semantic_budget: self.semantic_budget,
            episodic_budget: self.episodic_budget,
            working_budget: self.working_budget,
            reserve_budget: self.reserve_budget,
            total_ceiling: self.total_token_ceiling,
            max_working_turns: self.max_working_turns,
        }
    }

    /// Dynamically scales context partitions according to the active model and provider capability profile
    /// when contextMode is "auto", while preserving explicit manual user preferences.
    pub fn to_context_budget_for_model(
        &self,
        model_id: &str,
        provider: &ModelProviderKind,
    ) -> crate::ContextBudget {
        if self.context_mode == "manual" {
            return self.to_context_budget();
        }

        let profile = resolve_model_profile(model_id, provider);
        let ceiling = self.total_token_ceiling.max(profile.context_window).max(8192);

        // Reserve budget dynamically scales to guarantee ample headspace for model output + reasoning
        let reserve = (profile.max_output_tokens as usize)
            .max(self.reserve_budget)
            .max(2048);

        let remaining = ceiling.saturating_sub(reserve);
        let system = (remaining * 10 / 100).clamp(800, 4000);
        let semantic = (remaining * 20 / 100).max(self.semantic_budget);
        let episodic = (remaining * 25 / 100).max(self.episodic_budget);
        let working = remaining
            .saturating_sub(system)
            .saturating_sub(semantic)
            .saturating_sub(episodic)
            .max(2400);

        crate::ContextBudget {
            system_budget: system,
            semantic_budget: semantic,
            episodic_budget: episodic,
            working_budget: working,
            reserve_budget: reserve,
            total_ceiling: ceiling,
            max_working_turns: self.max_working_turns,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub model: ModelSettings,
    pub voice: VoiceSettings,
    #[serde(default)]
    pub search: SearchSettings,
    #[serde(default)]
    pub memory: MemoryConfigurationSettings,
    #[serde(default)]
    pub notification: NotificationSettings,
    pub retain_history: bool,
    pub speak_responses: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            model: ModelSettings::default(),
            voice: VoiceSettings::default(),
            search: SearchSettings::default(),
            memory: MemoryConfigurationSettings::default(),
            notification: NotificationSettings::default(),
            retain_history: true,
            speak_responses: false,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_settings_deserializes_legacy_shape() {
        let raw = r#"{
            "provider": "ollama",
            "modelId": "gemma4:latest",
            "ollamaEndpoint": "http://127.0.0.1:11434",
            "llamaCppEndpoint": "http://127.0.0.1:8080",
            "temperature": 0.4,
            "maxTokens": 512
        }"#;

        let settings: ModelSettings = serde_json::from_str(raw).expect("legacy settings");

        assert_eq!(settings.provider, ModelProviderKind::Ollama);
        assert_eq!(settings.active_model_ref.provider_id, "ollama-local");
        assert_eq!(settings.active_model_ref.model_id, "gemma4:latest");
        assert_eq!(settings.temperature, 0.4);
        assert!(settings
            .providers
            .iter()
            .any(|provider| provider.kind == ModelProviderKind::Ollama));
    }

    #[test]
    fn provider_kind_serializes_plan_wire_names() {
        assert_eq!(
            serde_json::to_string(&ModelProviderKind::OpenAi).unwrap(),
            "\"openai\""
        );
        assert_eq!(
            serde_json::to_string(&ModelProviderKind::OpenAiCompatible).unwrap(),
            "\"openai-compatible\""
        );

        let openai: ModelProviderKind = serde_json::from_str("\"open-ai\"").unwrap();
        let compatible: ModelProviderKind = serde_json::from_str("\"open-ai-compatible\"").unwrap();

        assert_eq!(openai, ModelProviderKind::OpenAi);
        assert_eq!(compatible, ModelProviderKind::OpenAiCompatible);
    }

    #[test]
    fn selecting_model_keeps_legacy_fields_in_sync() {
        let mut settings = ModelSettings::default();
        assert!(settings.select_model("ollama-local", "gemma3:1b"));

        assert_eq!(settings.provider, ModelProviderKind::Ollama);
        assert_eq!(settings.model_id, "gemma3:1b");
        assert_eq!(settings.active_model_ref.provider_id, "ollama-local");
        assert_eq!(
            settings.active_model_ref.provider_kind,
            ModelProviderKind::Ollama
        );
    }

    #[test]
    fn proprietary_providers_are_not_enabled_by_default() {
        let settings = ModelSettings::default();

        assert!(settings
            .providers
            .iter()
            .all(|provider| provider.kind.is_local()));
    }

    #[test]
    fn voice_settings_deserializes_without_wake_word_config() {
        let raw = r#"{
            "enabled": true,
            "speechToText": "disabled",
            "textToSpeech": "disabled",
            "whisperBinary": null,
            "whisperModelPath": null,
            "piperBinary": null,
            "piperVoicePath": null
        }"#;

        let settings: VoiceSettings = serde_json::from_str(raw).expect("legacy voice settings");

        assert!(settings.enabled);
        assert!(!settings.wake_word.enabled);
        assert_eq!(settings.wake_word.runtime, WakeWordRuntimeKind::Disabled);
        assert_eq!(settings.wake_word.threshold, 0.65);
    }

    #[test]
    fn normalization_repairs_missing_active_provider() {
        let mut settings = ModelSettings::default();
        let mut remote = ModelProviderConnection::new(ModelProviderKind::OpenAi, true);
        remote.auth_configured = true;
        settings.upsert_provider(remote);
        assert!(settings.select_model("openai", "gpt-5.4-mini"));

        settings
            .providers
            .retain(|provider| provider.id != "openai");
        let settings = settings.normalized();

        assert_eq!(settings.active_model_ref.provider_id, "mock-local");
        assert_eq!(settings.provider, ModelProviderKind::Mock);
        assert!(settings.active_connection().is_some());
    }

    #[test]
    fn search_credentials_are_never_serialized_with_settings() {
        let mut settings = AppSettings::default();
        settings.search.api_key = Some("compromised-test-secret".to_string());
        settings.search.auth_configured = true;

        let encoded = serde_json::to_string(&settings).expect("serialize app settings");

        assert!(!encoded.contains("compromised-test-secret"));
        assert!(!encoded.contains("apiKey"));
        assert!(encoded.contains("authConfigured"));
    }

    #[test]
    fn app_settings_deserializes_notification_settings_with_defaults() {
        let base = AppSettings::default();
        let serialized = serde_json::to_string(&base).expect("serialize");
        let settings: AppSettings = serde_json::from_str(&serialized).expect("deserialize");
        assert!(settings.notification.desktop_notifications_enabled);
        assert!(settings.notification.sound_enabled);
        assert_eq!(settings.notification.email_provider, "smtp");
    }
}


