use std::fmt::Debug;

use crate::{
    AgentDomainError, AgentEvent, ApprovalAggregate, ApprovalCommand, ApprovalState,
    CommandEnvelope, DomainEvent, EnvironmentAggregate, EnvironmentCommand, EnvironmentState,
    LifecycleState, RunAggregate, RunCommand, RunState, StepAggregate, StepAttemptAggregate,
    StepAttemptCommand, StepAttemptState, StepCommand, StepState, SubjectRef, TaskAggregate,
    TaskCommand, TaskState, Transition, TransitionContext,
};

pub fn reduce_run(
    current: &RunAggregate,
    command: &CommandEnvelope<RunCommand>,
    transition: &TransitionContext,
) -> Result<Transition<RunAggregate>, AgentDomainError> {
    ensure_envelope(
        current.metadata.tenant_id,
        current.metadata.version,
        command,
    )?;
    let from = current.status;
    let (to, reason) = match &command.payload {
        RunCommand::Queue if from == RunState::Created => (RunState::Queued, "queued"),
        RunCommand::Claim if from == RunState::Queued => (RunState::Starting, "claimed"),
        RunCommand::Start if from == RunState::Starting => (RunState::Running, "started"),
        RunCommand::WaitForTool if from == RunState::Running => {
            (RunState::WaitingForTool, "waiting_for_tool")
        }
        RunCommand::WaitForEvent if from == RunState::Running => {
            (RunState::WaitingForEvent, "waiting_for_event")
        }
        RunCommand::WaitForUser if from == RunState::Running => {
            (RunState::WaitingForUser, "waiting_for_user")
        }
        RunCommand::WaitForResource if from == RunState::Running => {
            (RunState::WaitingForResource, "waiting_for_resource")
        }
        RunCommand::Wake
            if matches!(
                from,
                RunState::WaitingForTool
                    | RunState::WaitingForEvent
                    | RunState::WaitingForUser
                    | RunState::WaitingForResource
            ) =>
        {
            (RunState::Queued, "wake_condition_satisfied")
        }
        RunCommand::Pause { safe_boundary }
            if matches!(
                from,
                RunState::Queued
                    | RunState::Starting
                    | RunState::Running
                    | RunState::WaitingForTool
                    | RunState::WaitingForEvent
                    | RunState::WaitingForUser
                    | RunState::WaitingForResource
                    | RunState::Retrying
                    | RunState::Recovering
            ) =>
        {
            ensure_safe_boundary(from_requires_safe_boundary(from), *safe_boundary)?;
            (RunState::Paused, "paused")
        }
        RunCommand::Resume if from == RunState::Paused => (RunState::Queued, "resumed"),
        RunCommand::ScheduleRetry
            if matches!(
                from,
                RunState::Starting
                    | RunState::Running
                    | RunState::WaitingForTool
                    | RunState::WaitingForResource
            ) =>
        {
            (RunState::Retrying, "retry_scheduled")
        }
        RunCommand::RetryReady if from == RunState::Retrying => {
            (RunState::Queued, "retry_backoff_elapsed")
        }
        RunCommand::BeginRecovery
            if matches!(
                from,
                RunState::Starting
                    | RunState::Running
                    | RunState::WaitingForTool
                    | RunState::WaitingForEvent
                    | RunState::WaitingForUser
                    | RunState::WaitingForResource
                    | RunState::Retrying
            ) =>
        {
            (RunState::Recovering, "recovery_started")
        }
        RunCommand::RecoveryReady if from == RunState::Recovering => {
            (RunState::Queued, "checkpoint_restored")
        }
        RunCommand::BeginCompletion if from == RunState::Running => {
            (RunState::Completing, "completion_started")
        }
        RunCommand::Complete { .. } if from == RunState::Completing => {
            (RunState::Completed, "completed")
        }
        RunCommand::Fail { error_code } if !from.is_terminal() => {
            validate_machine_code(error_code)?;
            (RunState::Failed, error_code.as_str())
        }
        RunCommand::Cancel { safe_boundary } if !from.is_terminal() => {
            ensure_safe_boundary(from_requires_safe_boundary(from), *safe_boundary)?;
            (RunState::Cancelled, "cancelled")
        }
        RunCommand::Expire { safe_boundary } if !from.is_terminal() => {
            ensure_safe_boundary(from_requires_safe_boundary(from), *safe_boundary)?;
            (RunState::Expired, "expired")
        }
        payload => return invalid_transition("run", from, run_command_name(payload)),
    };

    let mut aggregate = current.clone();
    aggregate.status = to;
    if let RunCommand::Complete { result_id } = &command.payload {
        aggregate.final_result_id = Some(*result_id);
    }
    finish_transition(
        aggregate,
        command,
        transition,
        SubjectRef::Run(current.id),
        run_event_type(to),
        DomainEvent::RunStateChanged {
            from,
            to,
            reason_code: reason.into(),
        },
        |aggregate| &mut aggregate.metadata,
    )
}

pub fn reduce_task(
    current: &TaskAggregate,
    command: &CommandEnvelope<TaskCommand>,
    transition: &TransitionContext,
) -> Result<Transition<TaskAggregate>, AgentDomainError> {
    ensure_envelope(
        current.metadata.tenant_id,
        current.metadata.version,
        command,
    )?;
    let from = current.status;
    let (to, reason) = match &command.payload {
        TaskCommand::Block if matches!(from, TaskState::Draft | TaskState::Ready) => {
            (TaskState::Blocked, "dependencies_blocked")
        }
        TaskCommand::MarkReady
            if matches!(
                from,
                TaskState::Draft | TaskState::Blocked | TaskState::RetryWait
            ) =>
        {
            (TaskState::Ready, "ready")
        }
        TaskCommand::Claim if from == TaskState::Ready => (TaskState::Claimed, "claimed"),
        TaskCommand::Start if from == TaskState::Claimed => (TaskState::Running, "started"),
        TaskCommand::Wait if from == TaskState::Running => (TaskState::Waiting, "waiting"),
        TaskCommand::Wake if from == TaskState::Waiting => {
            (TaskState::Ready, "wake_condition_satisfied")
        }
        TaskCommand::Pause { safe_boundary }
            if matches!(
                from,
                TaskState::Draft
                    | TaskState::Blocked
                    | TaskState::Ready
                    | TaskState::Claimed
                    | TaskState::Running
                    | TaskState::Waiting
                    | TaskState::RetryWait
            ) =>
        {
            ensure_safe_boundary(
                matches!(from, TaskState::Claimed | TaskState::Running),
                *safe_boundary,
            )?;
            (TaskState::Paused, "paused")
        }
        TaskCommand::Resume if from == TaskState::Paused => (TaskState::Ready, "resumed"),
        TaskCommand::Succeed { .. } if from == TaskState::Running => {
            (TaskState::Succeeded, "succeeded")
        }
        TaskCommand::Fail {
            error_code,
            retry_scheduled,
        } if matches!(
            from,
            TaskState::Claimed | TaskState::Running | TaskState::Waiting
        ) =>
        {
            validate_machine_code(error_code)?;
            (
                if *retry_scheduled {
                    TaskState::RetryWait
                } else {
                    TaskState::Failed
                },
                error_code.as_str(),
            )
        }
        TaskCommand::Cancel { safe_boundary } if !from.is_terminal() => {
            ensure_safe_boundary(
                matches!(from, TaskState::Claimed | TaskState::Running),
                *safe_boundary,
            )?;
            (TaskState::Cancelled, "cancelled")
        }
        TaskCommand::Skip
            if matches!(
                from,
                TaskState::Draft | TaskState::Blocked | TaskState::Ready
            ) =>
        {
            (TaskState::Skipped, "skipped")
        }
        TaskCommand::MarkCompensated { .. } if from == TaskState::Succeeded => {
            (TaskState::Compensated, "compensated")
        }
        payload => return invalid_transition("task", from, task_command_name(payload)),
    };

    let mut aggregate = current.clone();
    aggregate.status = to;
    match &command.payload {
        TaskCommand::Succeed { result_id } => aggregate.result_id = Some(*result_id),
        TaskCommand::MarkCompensated { result_id } => aggregate.compensation_result_id = *result_id,
        _ => {}
    }
    finish_transition(
        aggregate,
        command,
        transition,
        SubjectRef::Task(current.id),
        task_event_type(to),
        DomainEvent::TaskStateChanged {
            from,
            to,
            reason_code: reason.into(),
        },
        |aggregate| &mut aggregate.metadata,
    )
}

pub fn reduce_step(
    current: &StepAggregate,
    command: &CommandEnvelope<StepCommand>,
    transition: &TransitionContext,
) -> Result<Transition<StepAggregate>, AgentDomainError> {
    ensure_envelope(
        current.metadata.tenant_id,
        current.metadata.version,
        command,
    )?;
    let from = current.status;
    let (to, reason) = match &command.payload {
        StepCommand::MarkReady if from == StepState::Planned => (StepState::Ready, "ready"),
        StepCommand::Start if from == StepState::Ready => (StepState::Executing, "started"),
        StepCommand::Wait if from == StepState::Executing => (StepState::Waiting, "waiting"),
        StepCommand::Wake if from == StepState::Waiting => (StepState::Ready, "wake_satisfied"),
        StepCommand::Succeed { .. } if from == StepState::Executing => {
            (StepState::Succeeded, "succeeded")
        }
        StepCommand::Fail { error_code }
            if matches!(from, StepState::Executing | StepState::Waiting) =>
        {
            validate_machine_code(error_code)?;
            (StepState::Failed, error_code.as_str())
        }
        StepCommand::Cancel { safe_boundary } if !from.is_terminal() => {
            ensure_safe_boundary(from == StepState::Executing, *safe_boundary)?;
            (StepState::Cancelled, "cancelled")
        }
        StepCommand::Skip if matches!(from, StepState::Planned | StepState::Ready) => {
            (StepState::Skipped, "skipped")
        }
        payload => return invalid_transition("step", from, step_command_name(payload)),
    };

    let mut aggregate = current.clone();
    aggregate.status = to;
    if let StepCommand::Succeed { result_id } = &command.payload {
        aggregate.result_id = Some(*result_id);
    }
    finish_transition(
        aggregate,
        command,
        transition,
        SubjectRef::Step(current.id),
        step_event_type(to),
        DomainEvent::StepStateChanged {
            from,
            to,
            reason_code: reason.into(),
        },
        |aggregate| &mut aggregate.metadata,
    )
}

pub fn reduce_step_attempt(
    current: &StepAttemptAggregate,
    command: &CommandEnvelope<StepAttemptCommand>,
    transition: &TransitionContext,
) -> Result<Transition<StepAttemptAggregate>, AgentDomainError> {
    ensure_envelope(
        current.metadata.tenant_id,
        current.metadata.version,
        command,
    )?;
    let from = current.status;
    let (to, reason) = match &command.payload {
        StepAttemptCommand::Start if from == StepAttemptState::Leased => {
            (StepAttemptState::Started, "started")
        }
        StepAttemptCommand::PrepareEffect { .. } if from == StepAttemptState::Started => {
            (StepAttemptState::EffectPrepared, "effect_prepared")
        }
        StepAttemptCommand::CommitEffect {
            external_effect_ref,
        } if from == StepAttemptState::EffectPrepared => {
            validate_external_effect_ref(external_effect_ref)?;
            (StepAttemptState::EffectCommitted, "effect_committed")
        }
        StepAttemptCommand::ReconcileEffectCommitted {
            external_effect_ref,
        } if from == StepAttemptState::EffectUnknown => {
            validate_external_effect_ref(external_effect_ref)?;
            (
                StepAttemptState::EffectCommitted,
                "effect_reconciled_committed",
            )
        }
        StepAttemptCommand::ReconcileEffectAbsent if from == StepAttemptState::EffectUnknown => {
            (StepAttemptState::Failed, "effect_reconciled_absent")
        }
        StepAttemptCommand::RecordResult { .. }
            if matches!(
                from,
                StepAttemptState::Started | StepAttemptState::EffectCommitted
            ) =>
        {
            (StepAttemptState::ResultRecorded, "result_recorded")
        }
        StepAttemptCommand::Fail { error_code }
            if matches!(
                from,
                StepAttemptState::Leased
                    | StepAttemptState::Started
                    | StepAttemptState::EffectPrepared
            ) =>
        {
            validate_machine_code(error_code)?;
            (StepAttemptState::Failed, error_code.as_str())
        }
        StepAttemptCommand::Abandon if from == StepAttemptState::EffectPrepared => (
            StepAttemptState::EffectUnknown,
            "effect_reconciliation_required",
        ),
        StepAttemptCommand::Abandon
            if matches!(from, StepAttemptState::Leased | StepAttemptState::Started) =>
        {
            (StepAttemptState::Abandoned, "lease_abandoned")
        }
        payload => {
            return invalid_transition("step_attempt", from, step_attempt_command_name(payload))
        }
    };

    let mut aggregate = current.clone();
    aggregate.status = to;
    match &command.payload {
        StepAttemptCommand::PrepareEffect { effect_key } => {
            aggregate.effect_key = Some(effect_key.clone());
        }
        StepAttemptCommand::CommitEffect {
            external_effect_ref,
        } => aggregate.external_effect_ref = Some(external_effect_ref.clone()),
        StepAttemptCommand::ReconcileEffectCommitted {
            external_effect_ref,
        } => aggregate.external_effect_ref = Some(external_effect_ref.clone()),
        StepAttemptCommand::RecordResult { result_id } => aggregate.result_id = Some(*result_id),
        _ => {}
    }
    finish_transition(
        aggregate,
        command,
        transition,
        SubjectRef::StepAttempt(current.id),
        step_attempt_event_type(to),
        DomainEvent::StepAttemptStateChanged {
            from,
            to,
            reason_code: reason.into(),
        },
        |aggregate| &mut aggregate.metadata,
    )
}

pub fn reduce_approval(
    current: &ApprovalAggregate,
    command: &CommandEnvelope<ApprovalCommand>,
    transition: &TransitionContext,
) -> Result<Transition<ApprovalAggregate>, AgentDomainError> {
    ensure_envelope(
        current.metadata.tenant_id,
        current.metadata.version,
        command,
    )?;
    let from = current.status;
    let (to, reason) = match &command.payload {
        ApprovalCommand::Grant { action_digest } if from == ApprovalState::Requested => {
            ensure_approval_digest(current, action_digest)?;
            if command.requested_at >= current.expires_at {
                return invalid_transition("approval", from, "grant_expired");
            }
            (ApprovalState::Granted, "granted")
        }
        ApprovalCommand::Reject { action_digest } if from == ApprovalState::Requested => {
            ensure_approval_digest(current, action_digest)?;
            (ApprovalState::Rejected, "rejected")
        }
        ApprovalCommand::Expire
            if matches!(from, ApprovalState::Requested | ApprovalState::Granted) =>
        {
            if command.requested_at < current.expires_at {
                return invalid_transition("approval", from, "expire_before_deadline");
            }
            (ApprovalState::Expired, "expired")
        }
        ApprovalCommand::Revoke if from == ApprovalState::Granted => {
            (ApprovalState::Revoked, "revoked")
        }
        ApprovalCommand::Consume { action_digest } if from == ApprovalState::Granted => {
            ensure_approval_digest(current, action_digest)?;
            if command.requested_at >= current.expires_at {
                return invalid_transition("approval", from, "consume_expired");
            }
            (ApprovalState::Consumed, "consumed")
        }
        payload => return invalid_transition("approval", from, approval_command_name(payload)),
    };

    let mut aggregate = current.clone();
    aggregate.status = to;
    match &command.payload {
        ApprovalCommand::Grant { .. }
        | ApprovalCommand::Reject { .. }
        | ApprovalCommand::Revoke => {
            aggregate.decision_actor = Some(command.actor.clone());
        }
        ApprovalCommand::Consume { .. } => {
            aggregate.consumed_by = Some(command.actor.clone());
        }
        ApprovalCommand::Expire => {}
    }
    finish_transition(
        aggregate,
        command,
        transition,
        SubjectRef::Approval(current.id),
        approval_event_type(to),
        DomainEvent::ApprovalStateChanged {
            from,
            to,
            reason_code: reason.into(),
        },
        |aggregate| &mut aggregate.metadata,
    )
}

pub fn reduce_environment(
    current: &EnvironmentAggregate,
    command: &CommandEnvelope<EnvironmentCommand>,
    transition: &TransitionContext,
) -> Result<Transition<EnvironmentAggregate>, AgentDomainError> {
    ensure_envelope(
        current.metadata.tenant_id,
        current.metadata.version,
        command,
    )?;
    let from = current.status;
    let (to, reason) = match &command.payload {
        EnvironmentCommand::BeginProvisioning if from == EnvironmentState::Requested => {
            (EnvironmentState::Provisioning, "provisioning_started")
        }
        EnvironmentCommand::MarkReady if from == EnvironmentState::Provisioning => {
            (EnvironmentState::Ready, "ready")
        }
        EnvironmentCommand::Acquire if from == EnvironmentState::Ready => {
            (EnvironmentState::Busy, "acquired")
        }
        EnvironmentCommand::Release if from == EnvironmentState::Busy => {
            (EnvironmentState::Ready, "released")
        }
        EnvironmentCommand::Suspend { safe_boundary }
            if matches!(from, EnvironmentState::Ready | EnvironmentState::Busy) =>
        {
            ensure_safe_boundary(from == EnvironmentState::Busy, *safe_boundary)?;
            (EnvironmentState::Suspended, "suspended")
        }
        EnvironmentCommand::Resume if from == EnvironmentState::Suspended => {
            (EnvironmentState::Provisioning, "resume_started")
        }
        EnvironmentCommand::BeginDrain { safe_boundary }
            if matches!(
                from,
                EnvironmentState::Ready | EnvironmentState::Busy | EnvironmentState::Suspended
            ) =>
        {
            ensure_safe_boundary(from == EnvironmentState::Busy, *safe_boundary)?;
            (EnvironmentState::Draining, "draining")
        }
        EnvironmentCommand::BeginDestroy
            if matches!(
                from,
                EnvironmentState::Requested
                    | EnvironmentState::Provisioning
                    | EnvironmentState::Ready
                    | EnvironmentState::Suspended
                    | EnvironmentState::Draining
                    | EnvironmentState::Failed
                    | EnvironmentState::Quarantined
            ) =>
        {
            (EnvironmentState::Destroying, "destroying")
        }
        EnvironmentCommand::MarkDestroyed { .. } if from == EnvironmentState::Destroying => {
            (EnvironmentState::Destroyed, "destroyed")
        }
        EnvironmentCommand::Fail { error_code }
            if !matches!(
                from,
                EnvironmentState::Destroyed
                    | EnvironmentState::Failed
                    | EnvironmentState::Quarantined
            ) =>
        {
            validate_machine_code(error_code)?;
            (EnvironmentState::Failed, error_code.as_str())
        }
        EnvironmentCommand::Quarantine { error_code }
            if !matches!(
                from,
                EnvironmentState::Destroyed | EnvironmentState::Quarantined
            ) =>
        {
            validate_machine_code(error_code)?;
            (EnvironmentState::Quarantined, error_code.as_str())
        }
        payload => {
            return invalid_transition("environment", from, environment_command_name(payload))
        }
    };

    let mut aggregate = current.clone();
    aggregate.status = to;
    if let EnvironmentCommand::MarkDestroyed {
        cleanup_receipt_artifact_id,
    } = &command.payload
    {
        aggregate.cleanup_receipt_artifact_id = Some(*cleanup_receipt_artifact_id);
    }
    finish_transition(
        aggregate,
        command,
        transition,
        SubjectRef::Environment(current.id),
        environment_event_type(to),
        DomainEvent::EnvironmentStateChanged {
            from,
            to,
            reason_code: reason.into(),
        },
        |aggregate| &mut aggregate.metadata,
    )
}

fn ensure_envelope<C>(
    tenant_id: crate::TenantId,
    version: crate::AggregateVersion,
    command: &CommandEnvelope<C>,
) -> Result<(), AgentDomainError> {
    if tenant_id != command.tenant_id {
        return Err(AgentDomainError::InvariantViolation(
            "command tenant does not own the aggregate".into(),
        ));
    }
    command.actor.validate()?;
    if version != command.expected_version {
        return Err(AgentDomainError::VersionConflict {
            expected: command.expected_version.value(),
            actual: version.value(),
        });
    }
    Ok(())
}

fn finish_transition<A, C>(
    mut aggregate: A,
    command: &CommandEnvelope<C>,
    transition: &TransitionContext,
    subject: SubjectRef,
    event_type: &'static str,
    payload: DomainEvent,
    metadata: impl FnOnce(&mut A) -> &mut crate::AggregateMetadata,
) -> Result<Transition<A>, AgentDomainError> {
    if transition.recorded_at < command.requested_at {
        return Err(AgentDomainError::InvariantViolation(
            "an event cannot be recorded before its command was requested".into(),
        ));
    }
    let aggregate_metadata = metadata(&mut aggregate);
    aggregate_metadata.version = aggregate_metadata.version.next()?;
    aggregate_metadata.updated_at = command.requested_at;
    aggregate_metadata.last_event_id = Some(transition.event_id);
    let subject_version = aggregate_metadata.version;
    let event = AgentEvent::state_change(
        command.event_metadata(transition),
        subject,
        subject_version,
        event_type,
        payload,
    );
    Ok(Transition { aggregate, event })
}

fn ensure_safe_boundary(required: bool, provided: bool) -> Result<(), AgentDomainError> {
    if required && !provided {
        return Err(AgentDomainError::UnsafeBoundary);
    }
    Ok(())
}

fn ensure_approval_digest(
    approval: &ApprovalAggregate,
    provided: &crate::ContentDigest,
) -> Result<(), AgentDomainError> {
    if approval.action_digest != *provided {
        return Err(AgentDomainError::ApprovalDigestMismatch);
    }
    Ok(())
}

fn validate_machine_code(code: &str) -> Result<(), AgentDomainError> {
    if code.is_empty()
        || code.len() > 120
        || !code.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-')
        })
    {
        return Err(AgentDomainError::InvariantViolation(
            "reason and error codes must be lowercase machine identifiers".into(),
        ));
    }
    Ok(())
}

fn validate_external_effect_ref(reference: &str) -> Result<(), AgentDomainError> {
    if reference.trim().is_empty() || reference.len() > 2_048 {
        return Err(AgentDomainError::InvariantViolation(
            "external effect reference is invalid".into(),
        ));
    }
    Ok(())
}

fn from_requires_safe_boundary(state: RunState) -> bool {
    matches!(
        state,
        RunState::Starting | RunState::Running | RunState::WaitingForTool | RunState::Completing
    )
}

fn invalid_transition<A, T>(
    aggregate: &'static str,
    from: T,
    command: &'static str,
) -> Result<A, AgentDomainError>
where
    T: Debug,
{
    Err(AgentDomainError::InvalidTransition {
        aggregate,
        from: format!("{from:?}").to_ascii_lowercase(),
        command,
    })
}

fn run_command_name(command: &RunCommand) -> &'static str {
    match command {
        RunCommand::Queue => "queue",
        RunCommand::Claim => "claim",
        RunCommand::Start => "start",
        RunCommand::WaitForTool => "wait_for_tool",
        RunCommand::WaitForEvent => "wait_for_event",
        RunCommand::WaitForUser => "wait_for_user",
        RunCommand::WaitForResource => "wait_for_resource",
        RunCommand::Wake => "wake",
        RunCommand::Pause { .. } => "pause",
        RunCommand::Resume => "resume",
        RunCommand::ScheduleRetry => "schedule_retry",
        RunCommand::RetryReady => "retry_ready",
        RunCommand::BeginRecovery => "begin_recovery",
        RunCommand::RecoveryReady => "recovery_ready",
        RunCommand::BeginCompletion => "begin_completion",
        RunCommand::Complete { .. } => "complete",
        RunCommand::Fail { .. } => "fail",
        RunCommand::Cancel { .. } => "cancel",
        RunCommand::Expire { .. } => "expire",
    }
}

fn task_command_name(command: &TaskCommand) -> &'static str {
    match command {
        TaskCommand::Block => "block",
        TaskCommand::MarkReady => "mark_ready",
        TaskCommand::Claim => "claim",
        TaskCommand::Start => "start",
        TaskCommand::Wait => "wait",
        TaskCommand::Wake => "wake",
        TaskCommand::Pause { .. } => "pause",
        TaskCommand::Resume => "resume",
        TaskCommand::Succeed { .. } => "succeed",
        TaskCommand::Fail { .. } => "fail",
        TaskCommand::Cancel { .. } => "cancel",
        TaskCommand::Skip => "skip",
        TaskCommand::MarkCompensated { .. } => "mark_compensated",
    }
}

fn step_command_name(command: &StepCommand) -> &'static str {
    match command {
        StepCommand::MarkReady => "mark_ready",
        StepCommand::Start => "start",
        StepCommand::Wait => "wait",
        StepCommand::Wake => "wake",
        StepCommand::Succeed { .. } => "succeed",
        StepCommand::Fail { .. } => "fail",
        StepCommand::Cancel { .. } => "cancel",
        StepCommand::Skip => "skip",
    }
}

fn step_attempt_command_name(command: &StepAttemptCommand) -> &'static str {
    match command {
        StepAttemptCommand::Start => "start",
        StepAttemptCommand::PrepareEffect { .. } => "prepare_effect",
        StepAttemptCommand::CommitEffect { .. } => "commit_effect",
        StepAttemptCommand::ReconcileEffectCommitted { .. } => "reconcile_effect_committed",
        StepAttemptCommand::ReconcileEffectAbsent => "reconcile_effect_absent",
        StepAttemptCommand::RecordResult { .. } => "record_result",
        StepAttemptCommand::Fail { .. } => "fail",
        StepAttemptCommand::Abandon => "abandon",
    }
}

fn approval_command_name(command: &ApprovalCommand) -> &'static str {
    match command {
        ApprovalCommand::Grant { .. } => "grant",
        ApprovalCommand::Reject { .. } => "reject",
        ApprovalCommand::Expire => "expire",
        ApprovalCommand::Revoke => "revoke",
        ApprovalCommand::Consume { .. } => "consume",
    }
}

fn environment_command_name(command: &EnvironmentCommand) -> &'static str {
    match command {
        EnvironmentCommand::BeginProvisioning => "begin_provisioning",
        EnvironmentCommand::MarkReady => "mark_ready",
        EnvironmentCommand::Acquire => "acquire",
        EnvironmentCommand::Release => "release",
        EnvironmentCommand::Suspend { .. } => "suspend",
        EnvironmentCommand::Resume => "resume",
        EnvironmentCommand::BeginDrain { .. } => "begin_drain",
        EnvironmentCommand::BeginDestroy => "begin_destroy",
        EnvironmentCommand::MarkDestroyed { .. } => "mark_destroyed",
        EnvironmentCommand::Fail { .. } => "fail",
        EnvironmentCommand::Quarantine { .. } => "quarantine",
    }
}

fn run_event_type(state: RunState) -> &'static str {
    match state {
        RunState::Queued => "run.queued",
        RunState::Starting => "run.starting",
        RunState::Running => "run.started",
        RunState::WaitingForTool => "run.waiting_for_tool",
        RunState::WaitingForEvent => "run.waiting_for_event",
        RunState::WaitingForUser => "run.waiting_for_user",
        RunState::WaitingForResource => "run.waiting_for_resource",
        RunState::Paused => "run.paused",
        RunState::Retrying => "run.retrying",
        RunState::Recovering => "run.recovering",
        RunState::Completing => "run.completing",
        RunState::Completed => "run.completed",
        RunState::Failed => "run.failed",
        RunState::Cancelled => "run.cancelled",
        RunState::Expired => "run.expired",
        RunState::Created => "run.created",
    }
}

fn task_event_type(state: TaskState) -> &'static str {
    match state {
        TaskState::Draft => "task.created",
        TaskState::Blocked => "task.blocked",
        TaskState::Ready => "task.ready",
        TaskState::Claimed => "task.claimed",
        TaskState::Running => "task.started",
        TaskState::Waiting => "task.waiting",
        TaskState::Paused => "task.paused",
        TaskState::RetryWait => "task.retry_scheduled",
        TaskState::Succeeded => "task.completed",
        TaskState::Failed => "task.failed",
        TaskState::Cancelled => "task.cancelled",
        TaskState::Skipped => "task.skipped",
        TaskState::Compensated => "task.compensated",
    }
}

fn step_event_type(state: StepState) -> &'static str {
    match state {
        StepState::Planned => "step.planned",
        StepState::Ready => "step.ready",
        StepState::Executing => "step.started",
        StepState::Waiting => "step.waiting",
        StepState::Succeeded => "step.completed",
        StepState::Failed => "step.failed",
        StepState::Cancelled => "step.cancelled",
        StepState::Skipped => "step.skipped",
    }
}

fn step_attempt_event_type(state: StepAttemptState) -> &'static str {
    match state {
        StepAttemptState::Leased => "step_attempt.leased",
        StepAttemptState::Started => "step_attempt.started",
        StepAttemptState::EffectPrepared => "tool.call.prepared",
        StepAttemptState::EffectUnknown => "tool.call.effect_unknown",
        StepAttemptState::EffectCommitted => "tool.call.committed",
        StepAttemptState::ResultRecorded => "step_attempt.result_recorded",
        StepAttemptState::Failed => "step_attempt.failed",
        StepAttemptState::Abandoned => "step_attempt.abandoned",
    }
}

fn approval_event_type(state: ApprovalState) -> &'static str {
    match state {
        ApprovalState::Requested => "approval.requested",
        ApprovalState::Granted => "approval.granted",
        ApprovalState::Rejected => "approval.rejected",
        ApprovalState::Expired => "approval.expired",
        ApprovalState::Revoked => "approval.revoked",
        ApprovalState::Consumed => "approval.consumed",
    }
}

fn environment_event_type(state: EnvironmentState) -> &'static str {
    match state {
        EnvironmentState::Requested => "environment.requested",
        EnvironmentState::Provisioning => "environment.provisioning",
        EnvironmentState::Ready => "environment.ready",
        EnvironmentState::Busy => "environment.busy",
        EnvironmentState::Suspended => "environment.suspended",
        EnvironmentState::Draining => "environment.draining",
        EnvironmentState::Destroying => "environment.destroying",
        EnvironmentState::Destroyed => "environment.destroyed",
        EnvironmentState::Failed => "environment.failed",
        EnvironmentState::Quarantined => "environment.quarantined",
    }
}
