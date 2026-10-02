//! How a running run stops: requests (pause, cancel, the runner stopping) and limits (tokens,
//! active time, the project's daily budget, a step's budget). A pause lets every agent finish its
//! invocation, checkpointing a pending tool decision without granting it. A cancel or the runner
//! stopping ends them; a reached run limit ends them once their
//! turn is over (time: at once). A step's budget ends only that step's agent once its turn is
//! over and lets the others finish their invocations, like a pause. The run settles only after its
//! last agent has ended, and then stops at the limit it reached.

use orchester_core::{
  id::StepExecutionId,
  run::{RunLimit, RunStatus, execution_tokens},
  store::ActivityStore,
  tokens::TokensUsed,
};
use orchester_ipc::Rejection;

use super::{
  control::{Control, answer},
  idle,
  run::{ExecutionError, RunTask},
  step::worker::{Stop, StopCause},
};

impl RunTask {
  /// A pause waits for ordinary work or checkpoints a pending tool decision. Cancel and
  /// interruption end the agents immediately.
  pub(super) fn control(&mut self, control: Control) -> Result<(), ExecutionError> {
    match control {
      Control::Pause(ack) => {
        let requested = self.transition(|run, now| run.request_pause(now));
        if requested.is_ok() {
          self.stop_agents(Stop::AtApproval);
        }
        let result = match &requested {
          Ok(()) => Ok(()),
          Err(error) => Err(Rejection::Failed {
            detail: error.to_string(),
          }),
        };
        answer(ack, result);
        requested
      }
      Control::Cancel(ack) => {
        self.pending.cancel = Some(ack);
        self.stop_agents(Stop::Now(StopCause::Requested));
        Ok(())
      }
      Control::ChangePermissions(change, ack) => self.change_permissions(change, ack),
      Control::KeepPermissions(profile, ack) => self.keep_permissions(profile, ack),
      Control::DecidePlan(request, ack) => self.decide_plan(request, ack),
      Control::DecideToolApproval { approval, answer, ack } => self.decide_tool_approval(approval, answer, ack),
      Control::DeliverMessages(ack) => {
        Self::deliver_messages(ack);
        Ok(())
      }
      Control::Interrupt(cause) => {
        self.pending.interrupt = Some(cause);
        self.stop_agents(Stop::Now(StopCause::Requested));
        Ok(())
      }
    }
  }

  /// Whether new executions may start: nothing asks the run to stop.
  pub(super) fn may_start(&self) -> bool {
    self.pending.cancel.is_none()
      && self.pending.interrupt.is_none()
      && self.pending.limit.is_none()
      && self.agents.values().all(|active| !active.abandoned)
  }

  /// With no agent at work: carries out a cancel or interruption, a reached limit or a requested
  /// pause. `true` when the run no longer executes.
  pub(super) fn settle(&mut self) -> Result<bool, ExecutionError> {
    if let Some(ack) = self.pending.cancel.take() {
      let cancelled = self.transition(|run, now| run.cancel(now));
      let result = match &cancelled {
        Ok(()) => Ok(()),
        Err(error) => Err(Rejection::Failed {
          detail: error.to_string(),
        }),
      };
      answer(ack, result);
      return cancelled.map(|()| true);
    }
    if self.run.status() != &RunStatus::Running {
      // A run that succeeded while messages were queued for its agents works on at once.
      return Ok(!self.follow_up_queued()?);
    }
    if let Some(cause) = self.pending.interrupt {
      self.transition(|run, now| run.interrupt(cause, now))?;
      return Ok(true);
    }
    let limit = match self.pending.limit.take().filter(|limit| self.still_applies(*limit)) {
      Some(limit) => Some(limit),
      None => self.reached_limit()?,
    };
    if let Some(limit) = limit {
      self.transition(|run, now| run.stop_at_limit(limit, now))?;
      return Ok(true);
    }
    if self.run.pause_requested() {
      self.transition(|run, now| run.pause(now))?;
      return Ok(true);
    }
    Ok(false)
  }

  /// A limit the run has reached, checked before anything new starts: its own limits, the daily
  /// budget, or the budget of a step it continues (a step budget stays used up when the run's own
  /// limit was raised instead).
  fn reached_limit(&self) -> Result<Option<RunLimit>, ExecutionError> {
    Ok(if self.run.token_limit_reached() {
      Some(RunLimit::Tokens)
    } else if self.time_left()?.is_zero() {
      Some(RunLimit::Time)
    } else if idle::daily_budget_used(&self.db)? {
      Some(RunLimit::DailyBudget)
    } else if self
      .db
      .task_group_limit_reached(&self.run)
      .map_err(super::record::RecordError::from)?
    {
      Some(RunLimit::GroupBudget)
    } else {
      self
        .used_step_budget()?
        .map(|execution| RunLimit::StepBudget { execution })
    })
  }

  /// A running execution of the current attempt that has used its step's budget.
  fn used_step_budget(&self) -> Result<Option<StepExecutionId>, ExecutionError> {
    let invocations = self.db.invocations(self.run.id())?;
    Ok(self.run.running_executions().into_iter().find(|execution| {
      let step = self.run.execution(*execution).map(|execution| execution.step());
      step
        .and_then(|step| self.run.step_budget(step))
        .is_some_and(|budget| execution_tokens(&invocations, *execution).reached(budget))
    }))
  }

  /// Whether `limit`, reached while agents worked, still stops the run: a step's budget does only
  /// while its execution still runs (its agent may have finished the step in the turn that used
  /// the budget, or another step's failure may have ended the attempt).
  fn still_applies(&self, limit: RunLimit) -> bool {
    match limit {
      RunLimit::StepBudget { execution } => self.run.running_executions().contains(&execution),
      _ => true,
    }
  }

  /// A turn of `execution`'s agent completed with `tokens`: they count toward the run's budget,
  /// the project's daily budget and the step's budget. A run limit they used up ends every agent
  /// once its turn is over. A used-up step budget ends that step's agent once its turn is over;
  /// the others finish their invocations as for a pause and nothing new starts, so the run stops
  /// at the step's limit once they are done.
  pub(super) fn on_usage(&mut self, execution: StepExecutionId, tokens: TokensUsed) -> Result<(), ExecutionError> {
    self.transition(|run, now| run.record_usage(tokens, now))?;
    let step_budget_used = self
      .agents
      .get_mut(&execution)
      .is_some_and(|active| active.count(tokens));
    if self.pending.limit.is_none_or(|limit| !limit.is_run_limit()) {
      let limit = if self.run.token_limit_reached() {
        Some(RunLimit::Tokens)
      } else if idle::daily_budget_used(&self.db)? {
        Some(RunLimit::DailyBudget)
      } else if self
        .db
        .task_group_limit_reached(&self.run)
        .map_err(super::record::RecordError::from)?
      {
        Some(RunLimit::GroupBudget)
      } else {
        None
      };
      if let Some(limit) = limit {
        self.pending.limit = Some(limit);
        self.stop_agents(Stop::AfterTurn(StopCause::Limit(limit)));
        return Ok(());
      }
    }
    if step_budget_used && let Some(active) = self.agents.get(&execution) {
      let limit = RunLimit::StepBudget { execution };
      active.tell(Stop::AfterTurn(StopCause::Limit(limit)));
      self.pending.limit.get_or_insert(limit);
    }
    Ok(())
  }
}
