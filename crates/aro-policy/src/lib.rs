//! Central, side-effect-free authorization decisions for ARO.
//!
//! The model or a tool may describe an intent, but only this evaluator may turn matching,
//! server-provided policies into an authorization decision. Callers must load policy rules,
//! consents, grants, confirmations and revocations from trusted server-side stores.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const POLICY_ENGINE_VERSION: &str = "aro-policy/1";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum DataClassification {
    Public,
    #[default]
    Internal,
    Personal,
    Confidential,
    Sensitive,
    HighlySensitive,
    Secret,
    Regulated,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SubjectKind {
    User,
    Agent,
    SubAgent,
    Service,
    Plugin,
    McpServer,
    Administrator,
    InternalApplication,
    AutomatedTask,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectRef {
    pub kind: SubjectKind,
    /// Stable identifier for the effective subject. Never a display name.
    pub id: String,
    pub actor_user_id: Option<Uuid>,
    pub organization_id: Uuid,
    pub parent_subject_id: Option<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    pub kind: String,
    pub id: String,
    pub organization_id: Uuid,
    pub owner_id: Option<String>,
    #[serde(default)]
    pub classification: DataClassification,
    #[serde(default)]
    pub relations: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum RiskLevel {
    #[default]
    Low,
    Moderate,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationContext {
    pub workspace_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub agent_id: Option<String>,
    pub run_id: Option<Uuid>,
    pub task_id: Option<String>,
    pub tool_id: Option<String>,
    pub plugin_id: Option<String>,
    pub skill_id: Option<String>,
    pub mcp_server_id: Option<String>,
    pub environment: String,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    #[serde(default)]
    pub external_provider: bool,
    #[serde(default)]
    pub risk: RiskLevel,
    pub amount_minor: Option<i64>,
    #[serde(default)]
    pub actions_already_used: u64,
    pub cost_minor: Option<i64>,
    #[serde(default)]
    pub budget_already_used_minor: i64,
    pub device_id: Option<String>,
    pub ip_address: Option<String>,
    pub origin: String,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationEvidence {
    pub id: Uuid,
    pub subject_id: String,
    pub action: String,
    pub resource_id: String,
    pub agent_id: Option<String>,
    pub amount_minor: Option<i64>,
    /// Digest of the canonical intent, including tool input and every material parameter.
    pub intent_digest: String,
    pub valid_from: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub strong: bool,
}

impl ConfirmationEvidence {
    fn matches(&self, request: &AuthorizationRequest, now: DateTime<Utc>, strong: bool) -> bool {
        self.subject_id == request.subject.id
            && self.action == request.action
            && self.resource_id == request.resource.id
            && self.agent_id == request.context.agent_id
            && self.amount_minor == request.context.amount_minor
            && self.intent_digest == request.intent_digest
            && self.valid_from <= now
            && self.expires_at > now
            && self.consumed_at.is_none()
            && (!strong || self.strong)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdminApprovalEvidence {
    pub id: Uuid,
    pub subject_id: String,
    pub action: String,
    pub resource_id: String,
    pub agent_id: Option<String>,
    pub amount_minor: Option<i64>,
    pub intent_digest: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StepUpEvidence {
    pub subject_id: String,
    pub device_id: Option<String>,
    pub authenticated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl StepUpEvidence {
    fn matches(&self, request: &AuthorizationRequest, now: DateTime<Utc>) -> bool {
        self.subject_id == request.subject.id
            && self.device_id == request.context.device_id
            && self.authenticated_at <= now
            && self.expires_at > now
            && self.revoked_at.is_none()
    }
}

impl AdminApprovalEvidence {
    fn matches(&self, request: &AuthorizationRequest, now: DateTime<Utc>) -> bool {
        self.subject_id == request.subject.id
            && self.action == request.action
            && self.resource_id == request.resource.id
            && self.agent_id == request.context.agent_id
            && self.amount_minor == request.context.amount_minor
            && self.intent_digest == request.intent_digest
            && self.expires_at > now
            && self.revoked_at.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationEvidence {
    #[serde(default)]
    pub confirmations: Vec<ConfirmationEvidence>,
    #[serde(default)]
    pub step_up: Option<StepUpEvidence>,
    #[serde(default)]
    pub admin_approvals: Vec<AdminApprovalEvidence>,
    #[serde(default)]
    pub consent_ids: Vec<Uuid>,
    #[serde(default)]
    pub capability_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationRequest {
    pub id: Uuid,
    /// Server-computed digest of the full canonical intent. Evidence must bind to this value.
    pub intent_digest: String,
    pub subject: SubjectRef,
    pub action: String,
    pub resource: ResourceRef,
    pub context: EvaluationContext,
    #[serde(default)]
    pub requested_scopes: Vec<String>,
    #[serde(default)]
    pub evidence: AuthorizationEvidence,
    pub requested_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Allow,
    Deny,
    AllowWithConstraints,
    RequireConfirmation,
    RequireStepUpAuthentication,
    RequireAdminApproval,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum AuditLevel {
    None,
    #[default]
    Standard,
    Enhanced,
    Immutable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationRequirement {
    pub strong: bool,
    pub action: String,
    pub resource_id: String,
    pub subject_id: String,
    pub agent_id: Option<String>,
    pub amount_minor: Option<i64>,
    pub expires_in_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionConstraints {
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub allowed_resource_ids: Vec<String>,
    #[serde(default)]
    pub allowed_scopes: Vec<String>,
    #[serde(default)]
    pub allowed_tool_ids: Vec<String>,
    #[serde(default)]
    pub allowed_environments: Vec<String>,
    #[serde(default)]
    pub allowed_model_ids: Vec<String>,
    #[serde(default)]
    pub allowed_provider_ids: Vec<String>,
    pub max_actions: Option<u64>,
    pub max_amount_minor: Option<i64>,
    pub max_budget_minor: Option<i64>,
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub redact_fields: Vec<String>,
    #[serde(default)]
    pub local_model_only: bool,
    #[serde(default)]
    pub external_transfer_allowed: bool,
    #[serde(default)]
    pub delegation_allowed: bool,
    #[serde(default)]
    pub audit_level: AuditLevel,
}

impl Default for DecisionConstraints {
    fn default() -> Self {
        Self {
            read_only: false,
            allowed_resource_ids: Vec::new(),
            allowed_scopes: Vec::new(),
            allowed_tool_ids: Vec::new(),
            allowed_environments: Vec::new(),
            allowed_model_ids: Vec::new(),
            allowed_provider_ids: Vec::new(),
            max_actions: None,
            max_amount_minor: None,
            max_budget_minor: None,
            valid_until: None,
            redact_fields: Vec::new(),
            local_model_only: false,
            // Data egress and delegation require an explicit positive grant.
            external_transfer_allowed: false,
            delegation_allowed: false,
            audit_level: AuditLevel::Standard,
        }
    }
}

impl DecisionConstraints {
    fn intersect(&self, other: &Self) -> Result<Self, &'static str> {
        Ok(Self {
            read_only: self.read_only || other.read_only,
            allowed_resource_ids: intersect_bounds(
                &self.allowed_resource_ids,
                &other.allowed_resource_ids,
                "resource_constraints_disjoint",
            )?,
            allowed_scopes: intersect_bounds(
                &self.allowed_scopes,
                &other.allowed_scopes,
                "scope_constraints_disjoint",
            )?,
            allowed_tool_ids: intersect_bounds(
                &self.allowed_tool_ids,
                &other.allowed_tool_ids,
                "tool_constraints_disjoint",
            )?,
            allowed_environments: intersect_bounds(
                &self.allowed_environments,
                &other.allowed_environments,
                "environment_constraints_disjoint",
            )?,
            allowed_model_ids: intersect_bounds(
                &self.allowed_model_ids,
                &other.allowed_model_ids,
                "model_constraints_disjoint",
            )?,
            allowed_provider_ids: intersect_bounds(
                &self.allowed_provider_ids,
                &other.allowed_provider_ids,
                "provider_constraints_disjoint",
            )?,
            max_actions: minimum(self.max_actions, other.max_actions),
            max_amount_minor: minimum(self.max_amount_minor, other.max_amount_minor),
            max_budget_minor: minimum(self.max_budget_minor, other.max_budget_minor),
            valid_until: minimum(self.valid_until, other.valid_until),
            redact_fields: union(&self.redact_fields, &other.redact_fields),
            local_model_only: self.local_model_only || other.local_model_only,
            external_transfer_allowed: self.external_transfer_allowed
                && other.external_transfer_allowed,
            delegation_allowed: self.delegation_allowed && other.delegation_allowed,
            audit_level: self.audit_level.max(other.audit_level),
        })
    }

    fn is_constrained(&self) -> bool {
        self != &Self::default()
    }

    fn validate_request(&self, request: &AuthorizationRequest) -> Option<&'static str> {
        if self.read_only && !is_read_action(&request.action) {
            return Some("read_only_constraint");
        }
        if !self.allowed_resource_ids.is_empty()
            && !self.allowed_resource_ids.contains(&request.resource.id)
        {
            return Some("resource_outside_constraint");
        }
        if !request.requested_scopes.iter().all(|scope| {
            self.allowed_scopes.is_empty() || self.allowed_scopes.iter().any(|item| item == scope)
        }) {
            return Some("scope_outside_constraint");
        }
        if let Some(tool_id) = request.context.tool_id.as_ref() {
            if !self.allowed_tool_ids.is_empty() && !self.allowed_tool_ids.contains(tool_id) {
                return Some("tool_outside_constraint");
            }
        }
        if !self.allowed_environments.is_empty()
            && !self
                .allowed_environments
                .contains(&request.context.environment)
        {
            return Some("environment_outside_constraint");
        }
        if let Some(model_id) = request.context.model_id.as_ref() {
            if !self.allowed_model_ids.is_empty() && !self.allowed_model_ids.contains(model_id) {
                return Some("model_outside_constraint");
            }
        }
        if let Some(provider_id) = request.context.provider_id.as_ref() {
            if !self.allowed_provider_ids.is_empty()
                && !self.allowed_provider_ids.contains(provider_id)
            {
                return Some("provider_outside_constraint");
            }
        }
        if self.local_model_only && request.context.external_provider {
            return Some("local_model_required");
        }
        if !self.external_transfer_allowed && request.context.external_provider {
            return Some("external_transfer_forbidden");
        }
        if let (Some(requested), Some(maximum)) =
            (request.context.amount_minor, self.max_amount_minor)
        {
            if requested > maximum {
                return Some("amount_limit_exceeded");
            }
        }
        if self
            .max_actions
            .is_some_and(|maximum| request.context.actions_already_used >= maximum)
        {
            return Some("action_limit_exceeded");
        }
        if let Some(maximum) = self.max_budget_minor {
            let next_cost = request.context.cost_minor.unwrap_or_default().max(0);
            if request
                .context
                .budget_already_used_minor
                .saturating_add(next_cost)
                > maximum
            {
                return Some("budget_limit_exceeded");
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyLayer {
    LegacyAdapter,
    Capability,
    TemporaryGrant,
    Consent,
    Role,
    User,
    Organization,
    GlobalSecurity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Allow,
    Deny,
    RequireConfirmation,
    RequireStepUpAuthentication,
    RequireAdminApproval,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyTarget {
    #[serde(default)]
    pub subject_kinds: Vec<SubjectKind>,
    #[serde(default)]
    pub subject_ids: Vec<String>,
    #[serde(default)]
    pub organization_ids: Vec<Uuid>,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub resource_kinds: Vec<String>,
    #[serde(default)]
    pub resource_ids: Vec<String>,
    #[serde(default)]
    pub environments: Vec<String>,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    #[serde(default)]
    pub agent_ids: Vec<String>,
    #[serde(default)]
    pub classifications: Vec<DataClassification>,
    #[serde(default)]
    pub require_resource_owner_match: bool,
    #[serde(default)]
    pub required_relations: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub subject_attributes: BTreeMap<String, Value>,
    #[serde(default)]
    pub resource_attributes: BTreeMap<String, Value>,
    #[serde(default)]
    pub context_attributes: BTreeMap<String, Value>,
}

impl PolicyTarget {
    fn matches(&self, request: &AuthorizationRequest) -> bool {
        matches_bound(&self.subject_kinds, &request.subject.kind)
            && matches_bound(&self.subject_ids, &request.subject.id)
            && matches_bound(&self.organization_ids, &request.subject.organization_id)
            && matches_pattern_bound(&self.actions, &request.action)
            && matches_pattern_bound(&self.resource_kinds, &request.resource.kind)
            && matches_pattern_bound(&self.resource_ids, &request.resource.id)
            && matches_bound(&self.environments, &request.context.environment)
            && matches_optional_bound(&self.tool_ids, request.context.tool_id.as_ref())
            && matches_optional_bound(&self.agent_ids, request.context.agent_id.as_ref())
            && matches_bound(&self.classifications, &request.resource.classification)
            && (!self.require_resource_owner_match
                || request.resource.owner_id.as_deref() == Some(request.subject.id.as_str()))
            && attributes_match(&self.subject_attributes, &request.subject.attributes)
            && attributes_match(&self.resource_attributes, &request.resource.attributes)
            && attributes_match(&self.context_attributes, &request.context.attributes)
            && self.required_relations.iter().all(|(relation, audiences)| {
                request
                    .resource
                    .relations
                    .get(relation)
                    .is_some_and(|actual| {
                        audiences.is_empty() && actual.iter().any(|id| id == &request.subject.id)
                            || audiences.iter().all(|id| actual.contains(id))
                    })
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: String,
    pub version: u64,
    pub layer: PolicyLayer,
    pub priority: i32,
    pub effect: PolicyEffect,
    pub target: PolicyTarget,
    #[serde(default)]
    pub constraints: DecisionConstraints,
    pub reason: String,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl PolicyRule {
    fn active_at(&self, now: DateTime<Utc>) -> bool {
        self.enabled
            && self.valid_from.is_none_or(|from| from <= now)
            && self.valid_until.is_none_or(|until| until > now)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationDecision {
    pub id: Uuid,
    pub request_id: Uuid,
    pub decision: DecisionKind,
    pub reason: String,
    #[serde(default)]
    pub applied_policies: Vec<String>,
    #[serde(default)]
    pub constraints: DecisionConstraints,
    #[serde(default)]
    pub authorized_scopes: Vec<String>,
    pub valid_until: Option<DateTime<Utc>>,
    pub delegation_allowed: bool,
    #[serde(default)]
    pub redact_fields: Vec<String>,
    pub audit_level: AuditLevel,
    pub confirmation: Option<ConfirmationRequirement>,
    pub error_code: Option<String>,
    pub evaluated_at: DateTime<Utc>,
    pub engine_version: String,
}

impl AuthorizationDecision {
    pub fn permits_execution(&self) -> bool {
        matches!(
            self.decision,
            DecisionKind::Allow | DecisionKind::AllowWithConstraints
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn evaluate(
        &self,
        request: &AuthorizationRequest,
        policies: &[PolicyRule],
        now: DateTime<Utc>,
    ) -> AuthorizationDecision {
        if request.subject.id.trim().is_empty()
            || request.action.trim().is_empty()
            || request.resource.id.trim().is_empty()
            || request.intent_digest.trim().is_empty()
        {
            return deny(
                request,
                now,
                "invalid_request",
                "Authorization identity is incomplete.",
                vec![],
            );
        }
        if request.subject.organization_id != request.resource.organization_id {
            return deny(
                request,
                now,
                "tenant_boundary_violation",
                "Subject and resource belong to different organizations.",
                vec![],
            );
        }
        if request.expires_at.is_some_and(|expiry| expiry <= now) {
            return deny(
                request,
                now,
                "request_expired",
                "The authorization request expired.",
                vec![],
            );
        }
        if request.resource.classification == DataClassification::Secret
            && request.context.external_provider
        {
            return deny(
                request,
                now,
                "secret_external_transfer",
                "Secret data may not be transferred to an external provider.",
                vec!["invariant.secret-no-external-transfer".to_string()],
            );
        }

        let mut matching = policies
            .iter()
            .filter(|policy| policy.active_at(now) && policy.target.matches(request))
            .collect::<Vec<_>>();
        matching.sort_by_key(|policy| (policy.layer, policy.priority, policy.version));

        let applied = matching
            .iter()
            .map(|policy| format!("{}@{}", policy.id, policy.version))
            .collect::<Vec<_>>();

        if let Some(blocking) = matching
            .iter()
            .rev()
            .find(|rule| rule.effect == PolicyEffect::Deny)
        {
            return deny(request, now, "explicit_deny", &blocking.reason, applied);
        }

        let allow_rules = matching
            .iter()
            .filter(|rule| rule.effect == PolicyEffect::Allow)
            .copied()
            .collect::<Vec<_>>();
        if allow_rules.is_empty() {
            return deny(
                request,
                now,
                "no_matching_allow",
                "No active policy grants this action on this resource.",
                applied,
            );
        }

        if let Some(rule) = matching
            .iter()
            .rev()
            .find(|rule| rule.effect == PolicyEffect::RequireAdminApproval)
        {
            if !request
                .evidence
                .admin_approvals
                .iter()
                .any(|approval| approval.matches(request, now))
            {
                return gated(
                    request,
                    now,
                    DecisionKind::RequireAdminApproval,
                    &rule.reason,
                    applied,
                    "admin_approval_required",
                    None,
                );
            }
        }
        if let Some(rule) = matching
            .iter()
            .rev()
            .find(|rule| rule.effect == PolicyEffect::RequireStepUpAuthentication)
        {
            if !request
                .evidence
                .step_up
                .as_ref()
                .is_some_and(|evidence| evidence.matches(request, now))
            {
                return gated(
                    request,
                    now,
                    DecisionKind::RequireStepUpAuthentication,
                    &rule.reason,
                    applied,
                    "step_up_authentication_required",
                    None,
                );
            }
        }
        if let Some(rule) = matching
            .iter()
            .rev()
            .find(|rule| rule.effect == PolicyEffect::RequireConfirmation)
        {
            let strong = request.context.risk == RiskLevel::Critical;
            let confirmed = request
                .evidence
                .confirmations
                .iter()
                .any(|confirmation| confirmation.matches(request, now, strong));
            if !confirmed {
                let requirement = ConfirmationRequirement {
                    strong,
                    action: request.action.clone(),
                    resource_id: request.resource.id.clone(),
                    subject_id: request.subject.id.clone(),
                    agent_id: request.context.agent_id.clone(),
                    amount_minor: request.context.amount_minor,
                    expires_in_seconds: 300,
                };
                return gated(
                    request,
                    now,
                    DecisionKind::RequireConfirmation,
                    &rule.reason,
                    applied,
                    "confirmation_required",
                    Some(requirement),
                );
            }
        }

        let mut constraints = allow_rules[0].constraints.clone();
        constraints.valid_until = minimum(constraints.valid_until, allow_rules[0].valid_until);
        for rule in allow_rules.iter().skip(1) {
            let mut next = rule.constraints.clone();
            next.valid_until = minimum(next.valid_until, rule.valid_until);
            constraints = match constraints.intersect(&next) {
                Ok(constraints) => constraints,
                Err(error_code) => {
                    return deny(
                        request,
                        now,
                        error_code,
                        "Active policies have disjoint constraints; access is denied rather than widened.",
                        applied,
                    )
                }
            };
        }
        // Restrictive gate policies also narrow a grant; they can never widen it.
        for rule in matching
            .iter()
            .filter(|rule| rule.effect != PolicyEffect::Allow)
        {
            if rule.constraints == DecisionConstraints::default() {
                continue;
            }
            let mut next = rule.constraints.clone();
            next.valid_until = minimum(next.valid_until, rule.valid_until);
            constraints = match constraints.intersect(&next) {
                Ok(constraints) => constraints,
                Err(error_code) => {
                    return deny(
                        request,
                        now,
                        error_code,
                        "Active policies have disjoint constraints; access is denied rather than widened.",
                        applied,
                    )
                }
            };
        }
        constraints.valid_until = minimum(constraints.valid_until, request.expires_at);

        if let Some(error_code) = constraints.validate_request(request) {
            return deny(
                request,
                now,
                error_code,
                "The requested action exceeds the effective authorization constraints.",
                applied,
            );
        }

        let decision_kind = if constraints.is_constrained() {
            DecisionKind::AllowWithConstraints
        } else {
            DecisionKind::Allow
        };
        AuthorizationDecision {
            id: Uuid::new_v4(),
            request_id: request.id,
            decision: decision_kind,
            reason:
                "The action is granted by the listed policies within the effective constraints."
                    .to_string(),
            applied_policies: applied,
            authorized_scopes: if constraints.allowed_scopes.is_empty() {
                request.requested_scopes.clone()
            } else {
                request
                    .requested_scopes
                    .iter()
                    .filter(|scope| constraints.allowed_scopes.contains(scope))
                    .cloned()
                    .collect()
            },
            valid_until: constraints.valid_until,
            delegation_allowed: constraints.delegation_allowed,
            redact_fields: constraints.redact_fields.clone(),
            audit_level: constraints.audit_level,
            constraints,
            confirmation: None,
            error_code: None,
            evaluated_at: now,
            engine_version: POLICY_ENGINE_VERSION.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DelegationGrant {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub subject_id: String,
    pub parent_grant_id: Option<Uuid>,
    pub depth: u16,
    pub max_depth: u16,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub resource_ids: Vec<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    pub max_calls: Option<u64>,
    pub max_budget_minor: Option<i64>,
    pub expires_at: DateTime<Utc>,
    pub delegation_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DelegationRequest {
    pub organization_id: Uuid,
    pub child_subject_id: String,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub resource_ids: Vec<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    pub max_calls: Option<u64>,
    pub max_budget_minor: Option<i64>,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub delegation_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelegationError {
    NotDelegable,
    TenantMismatch,
    DepthExceeded,
    Expired,
    PermissionExpansion(&'static str),
}

/// Produces an attenuated child capability. Every dimension must be a subset of the parent, and
/// numeric/time limits can only decrease. Revoking the parent grant invalidates the returned
/// `parent_grant_id` chain.
pub fn attenuate_delegation(
    parent: &DelegationGrant,
    request: &DelegationRequest,
    now: DateTime<Utc>,
) -> Result<DelegationGrant, DelegationError> {
    if !parent.delegation_allowed {
        return Err(DelegationError::NotDelegable);
    }
    if parent.organization_id != request.organization_id {
        return Err(DelegationError::TenantMismatch);
    }
    if parent.depth >= parent.max_depth {
        return Err(DelegationError::DepthExceeded);
    }
    if parent.expires_at <= now || request.expires_at <= now {
        return Err(DelegationError::Expired);
    }
    ensure_subset(&request.actions, &parent.actions, "actions")?;
    ensure_subset(&request.resource_ids, &parent.resource_ids, "resources")?;
    ensure_subset(&request.scopes, &parent.scopes, "scopes")?;
    ensure_subset(&request.tool_ids, &parent.tool_ids, "tools")?;
    ensure_not_greater(request.max_calls, parent.max_calls, "calls")?;
    ensure_not_greater(request.max_budget_minor, parent.max_budget_minor, "budget")?;
    if request.delegation_allowed && parent.depth + 1 >= parent.max_depth {
        return Err(DelegationError::PermissionExpansion("delegation depth"));
    }

    Ok(DelegationGrant {
        id: Uuid::new_v4(),
        organization_id: parent.organization_id,
        subject_id: request.child_subject_id.clone(),
        parent_grant_id: Some(parent.id),
        depth: parent.depth + 1,
        max_depth: parent.max_depth,
        actions: request.actions.clone(),
        resource_ids: request.resource_ids.clone(),
        scopes: request.scopes.clone(),
        tool_ids: request.tool_ids.clone(),
        max_calls: request.max_calls,
        max_budget_minor: request.max_budget_minor,
        expires_at: request.expires_at.min(parent.expires_at),
        delegation_allowed: request.delegation_allowed,
    })
}

fn ensure_subset(
    requested: &[String],
    parent: &[String],
    dimension: &'static str,
) -> Result<(), DelegationError> {
    if requested.iter().all(|value| parent.contains(value)) {
        Ok(())
    } else {
        Err(DelegationError::PermissionExpansion(dimension))
    }
}

fn ensure_not_greater<T: PartialOrd + Copy>(
    requested: Option<T>,
    parent: Option<T>,
    dimension: &'static str,
) -> Result<(), DelegationError> {
    match (requested, parent) {
        (Some(requested), Some(parent)) if requested <= parent => Ok(()),
        (None, None) => Ok(()),
        _ => Err(DelegationError::PermissionExpansion(dimension)),
    }
}

fn deny(
    request: &AuthorizationRequest,
    now: DateTime<Utc>,
    code: &str,
    reason: &str,
    applied_policies: Vec<String>,
) -> AuthorizationDecision {
    AuthorizationDecision {
        id: Uuid::new_v4(),
        request_id: request.id,
        decision: DecisionKind::Deny,
        reason: reason.to_string(),
        applied_policies,
        constraints: DecisionConstraints::default(),
        authorized_scopes: Vec::new(),
        valid_until: None,
        delegation_allowed: false,
        redact_fields: Vec::new(),
        audit_level: AuditLevel::Enhanced,
        confirmation: None,
        error_code: Some(code.to_string()),
        evaluated_at: now,
        engine_version: POLICY_ENGINE_VERSION.to_string(),
    }
}

fn gated(
    request: &AuthorizationRequest,
    now: DateTime<Utc>,
    decision: DecisionKind,
    reason: &str,
    applied_policies: Vec<String>,
    code: &str,
    confirmation: Option<ConfirmationRequirement>,
) -> AuthorizationDecision {
    AuthorizationDecision {
        id: Uuid::new_v4(),
        request_id: request.id,
        decision,
        reason: reason.to_string(),
        applied_policies,
        constraints: DecisionConstraints::default(),
        authorized_scopes: Vec::new(),
        valid_until: None,
        delegation_allowed: false,
        redact_fields: Vec::new(),
        audit_level: AuditLevel::Enhanced,
        confirmation,
        error_code: Some(code.to_string()),
        evaluated_at: now,
        engine_version: POLICY_ENGINE_VERSION.to_string(),
    }
}

fn matches_bound<T: PartialEq>(allowed: &[T], actual: &T) -> bool {
    allowed.is_empty() || allowed.contains(actual)
}

fn matches_optional_bound<T: PartialEq>(allowed: &[T], actual: Option<&T>) -> bool {
    allowed.is_empty() || actual.is_some_and(|actual| allowed.contains(actual))
}

fn matches_pattern_bound(patterns: &[String], actual: &str) -> bool {
    patterns.is_empty()
        || patterns
            .iter()
            .any(|pattern| pattern_matches(pattern, actual))
}

fn pattern_matches(pattern: &str, actual: &str) -> bool {
    pattern == "*"
        || pattern == actual
        || pattern
            .strip_suffix(".*")
            .is_some_and(|prefix| actual.starts_with(&format!("{prefix}.")))
}

fn is_read_action(action: &str) -> bool {
    matches!(
        action.rsplit('.').next().unwrap_or_default(),
        "read" | "search" | "list" | "get" | "inspect" | "view" | "status"
    )
}

fn attributes_match(required: &BTreeMap<String, Value>, actual: &BTreeMap<String, Value>) -> bool {
    required
        .iter()
        .all(|(key, expected)| actual.get(key) == Some(expected))
}

fn intersect_bounds(
    left: &[String],
    right: &[String],
    disjoint_error: &'static str,
) -> Result<Vec<String>, &'static str> {
    if left.is_empty() {
        return Ok(right.to_vec());
    }
    if right.is_empty() {
        return Ok(left.to_vec());
    }
    let right = right.iter().collect::<BTreeSet<_>>();
    let intersection = left
        .iter()
        .filter(|item| right.contains(item))
        .cloned()
        .collect::<Vec<_>>();
    if intersection.is_empty() {
        Err(disjoint_error)
    } else {
        Ok(intersection)
    }
}

fn union(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .chain(right)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn minimum<T: Ord>(left: Option<T>, right: Option<T>) -> Option<T> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    fn request(now: DateTime<Utc>) -> AuthorizationRequest {
        let organization_id = Uuid::new_v4();
        AuthorizationRequest {
            id: Uuid::new_v4(),
            intent_digest: "sha256:test-intent".to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Agent,
                id: "agent:research".to_string(),
                actor_user_id: Some(Uuid::new_v4()),
                organization_id,
                parent_subject_id: None,
                attributes: BTreeMap::new(),
            },
            action: "network.read".to_string(),
            resource: ResourceRef {
                kind: "tool".to_string(),
                id: "core.search.web".to_string(),
                organization_id,
                owner_id: None,
                classification: DataClassification::Public,
                relations: BTreeMap::new(),
                attributes: BTreeMap::new(),
            },
            context: EvaluationContext {
                workspace_id: None,
                conversation_id: None,
                agent_id: Some("agent:research".to_string()),
                run_id: Some(Uuid::new_v4()),
                task_id: None,
                tool_id: Some("core.search.web".to_string()),
                plugin_id: None,
                skill_id: None,
                mcp_server_id: None,
                environment: "development".to_string(),
                model_id: None,
                provider_id: None,
                external_provider: false,
                risk: RiskLevel::Low,
                amount_minor: None,
                actions_already_used: 0,
                cost_minor: None,
                budget_already_used_minor: 0,
                device_id: None,
                ip_address: None,
                origin: "agent-run".to_string(),
                attributes: BTreeMap::new(),
            },
            requested_scopes: vec!["search.web".to_string()],
            evidence: AuthorizationEvidence::default(),
            requested_at: now,
            expires_at: Some(now + Duration::minutes(5)),
        }
    }

    fn allow(request: &AuthorizationRequest) -> PolicyRule {
        PolicyRule {
            id: "role.researcher.web-read".to_string(),
            version: 1,
            layer: PolicyLayer::Role,
            priority: 100,
            effect: PolicyEffect::Allow,
            target: PolicyTarget {
                subject_ids: vec![request.subject.id.clone()],
                actions: vec!["network.read".to_string()],
                resource_ids: vec![request.resource.id.clone()],
                ..PolicyTarget::default()
            },
            constraints: DecisionConstraints {
                allowed_scopes: vec!["search.web".to_string()],
                allowed_tool_ids: vec![request.resource.id.clone()],
                allowed_environments: vec!["development".to_string()],
                max_actions: Some(1),
                valid_until: request.expires_at,
                ..DecisionConstraints::default()
            },
            reason: "Research agents may search approved public sources.".to_string(),
            valid_from: None,
            valid_until: request.expires_at,
            enabled: true,
        }
    }

    #[test]
    fn default_deny_is_explicit_and_explainable() {
        let now = Utc::now();
        let decision = PolicyEngine.evaluate(&request(now), &[], now);
        assert_eq!(decision.decision, DecisionKind::Deny);
        assert_eq!(decision.error_code.as_deref(), Some("no_matching_allow"));
        assert!(!decision.permits_execution());
    }

    #[test]
    fn organization_deny_overrides_lower_layer_allow() {
        let now = Utc::now();
        let request = request(now);
        let mut deny_rule = allow(&request);
        deny_rule.id = "organization.block-web".to_string();
        deny_rule.layer = PolicyLayer::Organization;
        deny_rule.effect = PolicyEffect::Deny;
        deny_rule.reason = "The organization blocks Web access.".to_string();

        let decision = PolicyEngine.evaluate(&request, &[allow(&request), deny_rule], now);
        assert_eq!(decision.decision, DecisionKind::Deny);
        assert_eq!(decision.error_code.as_deref(), Some("explicit_deny"));
    }

    #[test]
    fn confirmation_is_bound_to_exact_subject_action_resource_agent_and_amount() {
        let now = Utc::now();
        let mut request = request(now);
        request.context.risk = RiskLevel::Critical;
        request.context.amount_minor = Some(2_500);
        let mut confirmation_rule = allow(&request);
        confirmation_rule.id = "security.confirm-critical".to_string();
        confirmation_rule.layer = PolicyLayer::GlobalSecurity;
        confirmation_rule.effect = PolicyEffect::RequireConfirmation;
        confirmation_rule.reason = "Critical actions require strong confirmation.".to_string();

        let pending =
            PolicyEngine.evaluate(&request, &[allow(&request), confirmation_rule.clone()], now);
        assert_eq!(pending.decision, DecisionKind::RequireConfirmation);
        assert!(pending.confirmation.is_some_and(|item| item.strong));

        request.evidence.confirmations.push(ConfirmationEvidence {
            id: Uuid::new_v4(),
            subject_id: request.subject.id.clone(),
            action: request.action.clone(),
            resource_id: request.resource.id.clone(),
            agent_id: request.context.agent_id.clone(),
            amount_minor: request.context.amount_minor,
            intent_digest: request.intent_digest.clone(),
            valid_from: now - Duration::seconds(1),
            expires_at: now + Duration::minutes(2),
            consumed_at: None,
            strong: true,
        });
        let allowed = PolicyEngine.evaluate(&request, &[allow(&request), confirmation_rule], now);
        assert!(allowed.permits_execution());
    }

    #[test]
    fn tenant_boundary_is_an_engine_invariant() {
        let now = Utc::now();
        let mut request = request(now);
        request.resource.organization_id = Uuid::new_v4();
        let decision = PolicyEngine.evaluate(&request, &[allow(&request)], now);
        assert_eq!(
            decision.error_code.as_deref(),
            Some("tenant_boundary_violation")
        );
    }

    #[test]
    fn constraints_are_intersections_not_unions() {
        let now = Utc::now();
        let request = request(now);
        let first = allow(&request);
        let mut second = allow(&request);
        second.id = "organization.narrow-web".to_string();
        second.layer = PolicyLayer::Organization;
        second.constraints.allowed_scopes =
            vec!["search.web".to_string(), "search.export".to_string()];
        second.constraints.max_actions = Some(5);

        let decision = PolicyEngine.evaluate(&request, &[first, second], now);
        assert!(decision.permits_execution());
        assert_eq!(decision.constraints.allowed_scopes, vec!["search.web"]);
        assert_eq!(decision.constraints.max_actions, Some(1));
    }

    #[test]
    fn disjoint_constraint_sets_deny_instead_of_becoming_unbounded() {
        let now = Utc::now();
        let request = request(now);
        let first = allow(&request);
        let mut second = allow(&request);
        second.id = "organization.other-tool-only".to_string();
        second.layer = PolicyLayer::Organization;
        second.constraints.allowed_tool_ids = vec!["core.files.delete".to_string()];

        let decision = PolicyEngine.evaluate(&request, &[first, second], now);
        assert_eq!(decision.decision, DecisionKind::Deny);
        assert_eq!(
            decision.error_code.as_deref(),
            Some("tool_constraints_disjoint")
        );
    }

    #[test]
    fn read_only_and_usage_limits_are_enforced() {
        let now = Utc::now();
        let mut write_request = request(now);
        write_request.action = "files.update".to_string();
        let mut write_allow = allow(&write_request);
        write_allow.target.actions = vec![write_request.action.clone()];
        write_allow.constraints.read_only = true;
        let write_decision = PolicyEngine.evaluate(&write_request, &[write_allow], now);
        assert_eq!(
            write_decision.error_code.as_deref(),
            Some("read_only_constraint")
        );

        let mut exhausted = request(now);
        exhausted.context.actions_already_used = 1;
        let exhausted_decision = PolicyEngine.evaluate(&exhausted, &[allow(&exhausted)], now);
        assert_eq!(
            exhausted_decision.error_code.as_deref(),
            Some("action_limit_exceeded")
        );
    }

    #[test]
    fn child_delegation_cannot_expand_parent_permissions() {
        let now = Utc::now();
        let organization_id = Uuid::new_v4();
        let parent = DelegationGrant {
            id: Uuid::new_v4(),
            organization_id,
            subject_id: "agent:parent".to_string(),
            parent_grant_id: None,
            depth: 0,
            max_depth: 2,
            actions: vec!["files.read".to_string()],
            resource_ids: vec!["project:one".to_string()],
            scopes: vec!["files:read".to_string()],
            tool_ids: vec!["core.files.read".to_string()],
            max_calls: Some(10),
            max_budget_minor: Some(1_000),
            expires_at: now + Duration::hours(1),
            delegation_allowed: true,
        };
        let mut child = DelegationRequest {
            organization_id,
            child_subject_id: "agent:child".to_string(),
            actions: parent.actions.clone(),
            resource_ids: parent.resource_ids.clone(),
            scopes: parent.scopes.clone(),
            tool_ids: parent.tool_ids.clone(),
            max_calls: Some(5),
            max_budget_minor: Some(500),
            expires_at: now + Duration::minutes(15),
            delegation_allowed: false,
        };
        assert!(attenuate_delegation(&parent, &child, now).is_ok());
        child.actions.push("files.delete".to_string());
        assert_eq!(
            attenuate_delegation(&parent, &child, now),
            Err(DelegationError::PermissionExpansion("actions"))
        );
    }
}
