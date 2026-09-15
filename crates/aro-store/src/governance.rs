use aro_core::{AroError, AroResult};
use aro_policy::{AuthorizationDecision, AuthorizationRequest, PolicyRule};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::{map_sqlx, AroStore, TenantContext};

impl AroStore {
    /// Loads only published, currently active rules through the tenant RLS transaction. A single
    /// malformed persisted rule fails the whole load so policy corruption can never widen access.
    pub async fn list_active_policy_rules(
        &self,
        context: TenantContext,
        now: DateTime<Utc>,
    ) -> AroResult<Vec<PolicyRule>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT rule.definition
            FROM policy_rules rule
            JOIN policy_versions version
              ON version.organization_id = rule.organization_id
             AND version.id = rule.policy_version_id
            JOIN policy_sets policy_set
              ON policy_set.organization_id = version.organization_id
             AND policy_set.id = version.policy_set_id
            WHERE rule.organization_id = $1
              AND rule.enabled
              AND version.status = 'published'
              AND policy_set.status = 'active'
              AND (rule.valid_from IS NULL OR rule.valid_from <= $2)
              AND (rule.valid_until IS NULL OR rule.valid_until > $2)
              AND (version.valid_from IS NULL OR version.valid_from <= $2)
              AND (version.valid_until IS NULL OR version.valid_until > $2)
            ORDER BY rule.layer, rule.priority, rule.rule_key
            "#,
        )
        .bind(context.organization_id())
        .bind(now)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let mut policies = Vec::with_capacity(rows.len());
        for row in rows {
            let definition = row.get::<Value, _>("definition");
            let rule = serde_json::from_value(definition).map_err(|error| {
                AroError::Security(format!(
                    "published authorization policy is invalid; evaluation failed closed: {error}"
                ))
            })?;
            policies.push(rule);
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(policies)
    }

    /// Persists the decision and a hash-linked, append-only security event in one tenant-bound
    /// transaction. Snapshots intentionally contain authorization metadata only: subject/resource
    /// attributes, confirmation payloads and any raw model/tool data are excluded.
    pub async fn record_permission_evaluation(
        &self,
        context: TenantContext,
        request: &AuthorizationRequest,
        decision: &AuthorizationDecision,
        execution_result: Option<&str>,
    ) -> AroResult<()> {
        if request.subject.organization_id != context.organization_id()
            || request.resource.organization_id != context.organization_id()
            || request.subject.actor_user_id != Some(context.actor_id())
        {
            return Err(AroError::Security(
                "permission evaluation does not belong to the authenticated tenant context"
                    .to_string(),
            ));
        }
        if decision.request_id != request.id {
            return Err(AroError::Security(
                "permission decision is not bound to the supplied request".to_string(),
            ));
        }

        let request_snapshot = sanitized_request_snapshot(request);
        let decision_snapshot = sanitized_decision_snapshot(decision);
        let decision_name = enum_name(&decision.decision)?;
        let subject_kind = enum_name(&request.subject.kind)?;
        let risk = enum_name(&request.context.risk)?;
        let classification = enum_name(&request.resource.classification)?;
        let constraints = serde_json::to_value(&decision.constraints).map_err(map_serde)?;

        let mut tx = self.begin_tenant_tx(context).await?;
        sqlx::query(
            r#"
            INSERT INTO permission_evaluations (
              id, organization_id, actor_user_id, subject_kind, subject_id, agent_id, run_id,
              task_id, tool_id, action, resource_kind, resource_id, environment, risk_level,
              data_classification, decision, reason, error_code, applied_policies, constraints,
              request_snapshot, decision_snapshot, engine_version, evaluated_at, execution_result,
              revocation_checked_at
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
              $17, $18, $19, $20, $21, $22, $23, $24, $25, $24
            )
            "#,
        )
        .bind(decision.id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&subject_kind)
        .bind(&request.subject.id)
        .bind(&request.context.agent_id)
        .bind(request.context.run_id)
        .bind(&request.context.task_id)
        .bind(&request.context.tool_id)
        .bind(&request.action)
        .bind(&request.resource.kind)
        .bind(&request.resource.id)
        .bind(&request.context.environment)
        .bind(&risk)
        .bind(&classification)
        .bind(&decision_name)
        .bind(&decision.reason)
        .bind(&decision.error_code)
        .bind(&decision.applied_policies)
        .bind(&constraints)
        .bind(&request_snapshot)
        .bind(&decision_snapshot)
        .bind(&decision.engine_version)
        .bind(decision.evaluated_at)
        .bind(execution_result)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        // Serialize the append sequence per organization so two concurrent events cannot fork the
        // hash chain. The lock is transaction-scoped and contains no user-controlled SQL.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(context.organization_id().to_string())
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let previous_event = sqlx::query(
            r#"
            SELECT tenant_sequence, event_hash
            FROM security_audit_events
            WHERE organization_id = $1
            ORDER BY tenant_sequence DESC
            LIMIT 1
            "#,
        )
        .bind(context.organization_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .map(|row| {
            (
                row.get::<i64, _>("tenant_sequence"),
                row.get::<String, _>("event_hash"),
            )
        });
        let tenant_sequence = previous_event
            .as_ref()
            .map_or(1, |(sequence, _)| sequence.saturating_add(1));
        let previous_hash = previous_event.map(|(_, event_hash)| event_hash);

        let audit_payload = json!({
            "decision": decision_name,
            "reason": decision.reason,
            "errorCode": decision.error_code,
            "appliedPolicies": decision.applied_policies,
            "constraints": constraints,
            "engineVersion": decision.engine_version,
            "executionResult": execution_result,
        });
        let event_hash = audit_event_hash(
            previous_hash.as_deref(),
            decision.id,
            &audit_payload,
            decision.evaluated_at,
        )?;
        sqlx::query(
            r#"
            INSERT INTO security_audit_events (
              organization_id, actor_user_id, subject_kind, subject_id, event_type, action,
              resource_kind, resource_id, evaluation_id, outcome, tenant_sequence, payload,
              previous_hash, event_hash, occurred_at
            )
            VALUES ($1, $2, $3, $4, 'permission.evaluated', $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(subject_kind)
        .bind(&request.subject.id)
        .bind(&request.action)
        .bind(&request.resource.kind)
        .bind(&request.resource.id)
        .bind(decision.id)
        .bind(if decision.permits_execution() {
            "authorized"
        } else {
            "blocked"
        })
        .bind(tenant_sequence)
        .bind(audit_payload)
        .bind(previous_hash)
        .bind(event_hash)
        .bind(decision.evaluated_at)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }
}

fn sanitized_request_snapshot(request: &AuthorizationRequest) -> Value {
    json!({
        "id": request.id,
        "intentDigest": request.intent_digest,
        "subject": {
            "kind": request.subject.kind,
            "id": request.subject.id,
            "actorUserId": request.subject.actor_user_id,
            "organizationId": request.subject.organization_id,
            "parentSubjectId": request.subject.parent_subject_id,
        },
        "action": request.action,
        "resource": {
            "kind": request.resource.kind,
            "id": request.resource.id,
            "organizationId": request.resource.organization_id,
            "ownerId": request.resource.owner_id,
            "classification": request.resource.classification,
            "relationNames": request.resource.relations.keys().collect::<Vec<_>>(),
        },
        "context": {
            "workspaceId": request.context.workspace_id,
            "conversationId": request.context.conversation_id,
            "agentId": request.context.agent_id,
            "runId": request.context.run_id,
            "taskId": request.context.task_id,
            "toolId": request.context.tool_id,
            "pluginId": request.context.plugin_id,
            "skillId": request.context.skill_id,
            "mcpServerId": request.context.mcp_server_id,
            "environment": request.context.environment,
            "modelId": request.context.model_id,
            "providerId": request.context.provider_id,
            "externalProvider": request.context.external_provider,
            "risk": request.context.risk,
            "amountMinor": request.context.amount_minor,
            "actionsAlreadyUsed": request.context.actions_already_used,
            "costMinor": request.context.cost_minor,
            "budgetAlreadyUsedMinor": request.context.budget_already_used_minor,
            "origin": request.context.origin,
        },
        "requestedScopes": request.requested_scopes,
        "evidence": {
            "confirmationIds": request.evidence.confirmations.iter().map(|item| item.id).collect::<Vec<_>>(),
            "stepUpEvidencePresent": request.evidence.step_up.is_some(),
            "adminApprovalIds": request.evidence.admin_approvals.iter().map(|item| item.id).collect::<Vec<_>>(),
            "consentIds": request.evidence.consent_ids,
            "capabilityIds": request.evidence.capability_ids,
        },
        "requestedAt": request.requested_at,
        "expiresAt": request.expires_at,
    })
}

fn sanitized_decision_snapshot(decision: &AuthorizationDecision) -> Value {
    json!({
        "id": decision.id,
        "requestId": decision.request_id,
        "decision": decision.decision,
        "reason": decision.reason,
        "appliedPolicies": decision.applied_policies,
        "constraints": decision.constraints,
        "authorizedScopes": decision.authorized_scopes,
        "validUntil": decision.valid_until,
        "delegationAllowed": decision.delegation_allowed,
        "redactFields": decision.redact_fields,
        "auditLevel": decision.audit_level,
        "confirmation": decision.confirmation,
        "errorCode": decision.error_code,
        "evaluatedAt": decision.evaluated_at,
        "engineVersion": decision.engine_version,
    })
}

fn audit_event_hash(
    previous_hash: Option<&str>,
    evaluation_id: uuid::Uuid,
    payload: &Value,
    occurred_at: chrono::DateTime<chrono::Utc>,
) -> AroResult<String> {
    let canonical_payload = serde_json::to_vec(payload).map_err(map_serde)?;
    let mut hasher = Sha256::new();
    hasher.update(previous_hash.unwrap_or("genesis").as_bytes());
    hasher.update(evaluation_id.as_bytes());
    hasher.update(occurred_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true));
    hasher.update(canonical_payload);
    Ok(format!("{:x}", hasher.finalize()))
}

fn enum_name<T: Serialize>(value: &T) -> AroResult<String> {
    serde_json::to_value(value)
        .map_err(map_serde)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| {
            AroError::Unexpected("authorization enum did not serialize as text".to_string())
        })
}

fn map_serde(error: serde_json::Error) -> AroError {
    AroError::Unexpected(format!("authorization audit serialization failed: {error}"))
}

#[cfg(test)]
mod tests {
    use aro_policy::DecisionKind;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn event_hash_is_chained_and_payload_sensitive() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let first = audit_event_hash(None, id, &json!({"decision": "deny"}), now).unwrap();
        let changed = audit_event_hash(None, id, &json!({"decision": "allow"}), now).unwrap();
        let chained =
            audit_event_hash(Some(&first), id, &json!({"decision": "deny"}), now).unwrap();
        assert_ne!(first, changed);
        assert_ne!(first, chained);
    }

    #[test]
    fn decision_names_match_database_contract() {
        assert_eq!(
            enum_name(&DecisionKind::AllowWithConstraints).unwrap(),
            "allow_with_constraints"
        );
        assert_eq!(
            enum_name(&DecisionKind::RequireConfirmation).unwrap(),
            "require_confirmation"
        );
    }
}
