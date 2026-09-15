use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    AgentDomainError, AgentVersionId, ContentDigest, ConversationId, EnvironmentProfileId,
    WorkspaceId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentVisibility {
    Private,
    Organization,
    Published,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTemplateVersionSpec {
    pub schema_version: u16,
    pub display_name: String,
    pub description: String,
    pub visibility: AgentVisibility,
    pub parameter_schema: Value,
    pub default_agent_version: AgentVersionSpec,
}

impl AgentTemplateVersionSpec {
    pub const CURRENT_SCHEMA_VERSION: u16 = 1;

    pub fn validate(&self) -> Result<(), AgentDomainError> {
        if self.schema_version != Self::CURRENT_SCHEMA_VERSION {
            return Err(AgentDomainError::InvalidAgentSpec(
                "unsupported agent template specification version".into(),
            ));
        }
        validate_non_empty("template display_name", &self.display_name, 160)?;
        validate_non_empty("template description", &self.description, 8_000)?;
        if !self.parameter_schema.is_object() {
            return Err(AgentDomainError::InvalidAgentSpec(
                "template parameter_schema must be a JSON object schema".into(),
            ));
        }
        self.default_agent_version.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstanceBindingSpec {
    pub agent_version_id: AgentVersionId,
    pub conversation_id: Option<ConversationId>,
    pub workspace_id: Option<WorkspaceId>,
    pub environment_profile_id: Option<EnvironmentProfileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    Supervised,
    ConfirmSensitive,
    PolicyBound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionLocality {
    Local,
    PrivateCloud,
    PublicCloud,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRouteSpec {
    pub provider_id: String,
    pub model_id: String,
    pub locality: ExecutionLocality,
    pub region: Option<String>,
    pub maximum_data_classification: DataClassification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoutingPolicy {
    pub primary: ModelRouteSpec,
    #[serde(default)]
    pub fallbacks: Vec<ModelRouteSpec>,
    #[serde(default)]
    pub allow_cross_locality_fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityBinding {
    pub capability_id: String,
    pub version: String,
    #[serde(default)]
    pub actions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityPolicy {
    #[serde(default)]
    pub allowed: Vec<CapabilityBinding>,
    #[serde(default)]
    pub denied: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Agent,
    Run,
    Session,
    Task,
    User,
    Organization,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWriteMode {
    Disabled,
    ProposeOnly,
    AutomaticPolicyBound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicy {
    #[serde(default)]
    pub readable_scopes: BTreeSet<MemoryScope>,
    pub write_mode: MemoryWriteMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentKind {
    LocalEdge,
    Container,
    Sandbox,
    Browser,
    Remote,
    Cloud,
    Enterprise,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentPolicy {
    pub default_profile_id: Option<EnvironmentProfileId>,
    #[serde(default)]
    pub allowed_kinds: BTreeSet<EnvironmentKind>,
    #[serde(default)]
    pub can_change_environment: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLimits {
    pub maximum_tasks: u32,
    pub maximum_parallel_tasks: u16,
    pub maximum_steps_per_task: u32,
    pub maximum_subagent_depth: u8,
    pub maximum_subagents: u16,
    pub activation_timeout_seconds: u32,
    pub run_timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetLimits {
    pub cost_microunits: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tool_calls: u32,
    pub model_calls: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionSpec {
    pub schema_version: u16,
    pub display_name: String,
    pub objective: String,
    pub instructions_ref: String,
    pub instructions_digest: ContentDigest,
    pub autonomy: AutonomyLevel,
    pub model_policy: ModelRoutingPolicy,
    pub capability_policy: CapabilityPolicy,
    pub memory_policy: MemoryPolicy,
    pub environment_policy: EnvironmentPolicy,
    pub runtime_limits: RuntimeLimits,
    pub budget_limits: BudgetLimits,
}

impl AgentVersionSpec {
    pub const CURRENT_SCHEMA_VERSION: u16 = 1;

    pub fn validate(&self) -> Result<(), AgentDomainError> {
        if self.schema_version != Self::CURRENT_SCHEMA_VERSION {
            return Err(AgentDomainError::InvalidAgentSpec(format!(
                "unsupported agent specification version {}",
                self.schema_version
            )));
        }
        validate_non_empty("display_name", &self.display_name, 160)?;
        validate_non_empty("objective", &self.objective, 8_000)?;
        validate_non_empty("instructions_ref", &self.instructions_ref, 2_048)?;
        validate_route(&self.model_policy.primary)?;

        let mut routes = HashSet::new();
        routes.insert((
            self.model_policy.primary.provider_id.as_str(),
            self.model_policy.primary.model_id.as_str(),
        ));
        for route in &self.model_policy.fallbacks {
            validate_route(route)?;
            if !routes.insert((route.provider_id.as_str(), route.model_id.as_str())) {
                return Err(AgentDomainError::InvalidAgentSpec(
                    "model routes must be unique".into(),
                ));
            }
            if !self.model_policy.allow_cross_locality_fallback
                && route.locality != self.model_policy.primary.locality
            {
                return Err(AgentDomainError::InvalidAgentSpec(
                    "cross-locality fallback requires explicit permission".into(),
                ));
            }
        }

        let mut capabilities = HashSet::new();
        for binding in &self.capability_policy.allowed {
            validate_identifier("capability_id", &binding.capability_id, 180)?;
            validate_non_empty("capability version", &binding.version, 80)?;
            if !capabilities.insert(binding.capability_id.as_str()) {
                return Err(AgentDomainError::InvalidAgentSpec(
                    "capability bindings must be unique".into(),
                ));
            }
            if self
                .capability_policy
                .denied
                .contains(&binding.capability_id)
            {
                return Err(AgentDomainError::InvalidAgentSpec(
                    "a capability cannot be both allowed and denied".into(),
                ));
            }
        }

        let limits = &self.runtime_limits;
        if limits.maximum_tasks == 0
            || limits.maximum_parallel_tasks == 0
            || u32::from(limits.maximum_parallel_tasks) > limits.maximum_tasks
            || limits.maximum_steps_per_task == 0
            || limits.activation_timeout_seconds == 0
            || limits.run_timeout_seconds < u64::from(limits.activation_timeout_seconds)
            || (limits.maximum_subagents == 0) != (limits.maximum_subagent_depth == 0)
            || limits.activation_timeout_seconds > 3_600
            || limits.run_timeout_seconds > 31_536_000
        {
            return Err(AgentDomainError::InvalidAgentSpec(
                "runtime limits are inconsistent".into(),
            ));
        }
        if self.environment_policy.default_profile_id.is_some()
            && self.environment_policy.allowed_kinds.is_empty()
        {
            return Err(AgentDomainError::InvalidAgentSpec(
                "a default environment requires at least one allowed environment kind".into(),
            ));
        }
        Ok(())
    }
}

fn validate_route(route: &ModelRouteSpec) -> Result<(), AgentDomainError> {
    validate_identifier("provider_id", &route.provider_id, 120)?;
    validate_identifier("model_id", &route.model_id, 180)?;
    if let Some(region) = &route.region {
        validate_identifier("region", region, 80)?;
    }
    Ok(())
}

fn validate_identifier(
    label: &str,
    value: &str,
    maximum_length: usize,
) -> Result<(), AgentDomainError> {
    validate_non_empty(label, value, maximum_length)?;
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | ':' | '/')
    }) {
        return Err(AgentDomainError::InvalidAgentSpec(format!(
            "{label} contains unsupported characters"
        )));
    }
    Ok(())
}

fn validate_non_empty(
    label: &str,
    value: &str,
    maximum_length: usize,
) -> Result<(), AgentDomainError> {
    let value = value.trim();
    if value.is_empty() || value.len() > maximum_length {
        return Err(AgentDomainError::InvalidAgentSpec(format!(
            "{label} must contain between 1 and {maximum_length} bytes"
        )));
    }
    Ok(())
}
