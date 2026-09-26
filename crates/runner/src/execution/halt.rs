//! How a running run stops: requests (pause, cancel, the runner stopping) and limits (tokens,
//! active time, the project's daily budget, a step's budget). A pause lets every agent finish its
//! invocation; a cancel or the runner stopping ends them; a reached limit ends them once their
//! turn is over (time: at once). The run settles only after its last agent has ended.

use orchester_core::{
  id::StepExecutionId,
  run::{RunLimit, RunStatus},
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
  /// Handles a request. A pause is recorded at once and takes effect when the agents have ended;
  /// a cancel or the runner stopping ends the agents.
  pub(super) fn control(&mut self, control: Control) -> Result<(), ExecutionError> {
    match control {
      Control::Pause(ack) => {
        let requested = self.transition(|run, now| run.request_pause(now));
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
      return Ok(true);
    }
    if let Some(cause) = self.pending.interrupt {
      self.transition(|run, now| run.interrupt(cause, now))?;
      return Ok(true);
    }
    let limit = match self.pending.limit.take() {
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

  /// A run limit the run has reached, checked before anything new starts.
  fn reached_limit(&self) -> Result<Option<RunLimit>, ExecutionError> {
    Ok(if self.run.token_limit_reached() {
      Some(RunLimit::Tokens)
    } else if self.time_left()?.is_zero() {
      Some(RunLimit::Time)
    } else if idle::daily_budget_used(&self.db)? {
      Some(RunLimit::DailyBudget)
    } else {
      None
    })
  }

  /// A turn of `execution`'s agent completed with `tokens`: they count toward the run's budget,
  /// the project's daily budget and the step's budget. A budget it used up ends the agents it
  /// bounds once their turn is over.
  pub(super) fn on_usage(&mut self, execution: StepExecutionId, tokens: TokensUsed) -> Result<(), ExecutionError> {
    self.transition(|run, now| run.record_usage(tokens, now))?;
    if self.pending.limit.is_none() {
      let limit = if self.run.token_limit_reached() {
        Some(RunLimit::Tokens)
      } else if idle::daily_budget_used(&self.db)? {
        Some(RunLimit::DailyBudget)
      } else {
        None
      };
      if let Some(limit) = limit {
        self.pending.limit = Some(limit);
        self.stop_agents(Stop::AfterTurn(StopCause::Limit(limit)));
      }
    }
    if let Some(active) = self.agents.get_mut(&execution)
      && active.count(tokens)
    {
      active.tell(Stop::AfterTurn(StopCause::StepBudget));
    }
    Ok(())
  }
}
