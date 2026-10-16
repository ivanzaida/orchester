use serde::{Deserialize, Serialize};

use super::{not_started::WorktreeFailure, plan::ReviewNote};
use crate::{id::StepExecutionId, resource::StepKey};

/// Why a running run stopped without a checkpoint-safe pause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptCause {
  /// The runner process dropped while the application kept running. Runs interrupted this
  /// way continue on their own once the runner reconnects.
  RunnerLost,
  /// The user chose Quit now while agents were working.
  QuitNow,
  /// The application ended abnormally (crash, power loss, process killed).
  Crashed,
  /// The runner could not record or carry on the run (the project database failed or refused a write):
  /// its agents were ended, and a person continues it from its last checkpoint or cancels it.
  RunnerFailed,
}

impl InterruptCause {
  /// Whether a reconnecting runner continues the run without asking. Runs interrupted by the
  /// end of a session wait for an explicit continue.
  pub const fn continues_on_reconnect(self) -> bool {
    matches!(self, Self::RunnerLost)
  }
}

/// Why a run failed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunFailure {
  /// The step failed and its pipeline step has no failure route.
  StepFailed(StepKey),
  /// The step failed after its failure route was taken `max_retries` times.
  RetriesExhausted(StepKey),
  /// The run never started: its worktree could not be created, for this reason.
  WorktreeNotCreated(WorktreeFailure),
}

/// A limit of a run: its token budget, its time limit on active agent time, the project's daily
/// token budget that all its runs share, or the budget of one of its steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLimit {
  Tokens,
  Time,
  /// The project's runs used today's token budget. The run continues with a plain resume once
  /// the day has room again (tomorrow, or a raised daily budget).
  DailyBudget,
  /// The reviewed task group reached its aggregate cap; raising a per-run limit cannot lift it.
  GroupBudget,
  /// `execution` used its step's budget. The run continues once that step's budget is raised for
  /// the run ([`super::Run::raise_step_budget`]); the published pipeline stays as it is.
  StepBudget {
    execution: StepExecutionId,
  },
  /// `execution`'s agent CLI refused its turn because its account reached its plan limit
  /// (`docs/design/agent-accounts.md`). The run continues the same session on the same account at
  /// the limit's reset, or the person continues the step now on another account of the same CLI as a
  /// new session for this run; a running agent never moves to another account by itself.
  PlanLimit {
    execution: StepExecutionId,
  },
}

impl RunLimit {
  /// Whether this is a limit of the whole run, which a raised run budget or time limit lifts, as
  /// opposed to a step's budget or an account's plan.
  pub const fn is_run_limit(self) -> bool {
    !matches!(
      self,
      Self::StepBudget { .. } | Self::GroupBudget | Self::PlanLimit { .. }
    )
  }
}

/// Why a run is paused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
  /// A person paused it (or chose to wait for agents to save before closing).
  #[default]
  Requested,
  /// It reached this limit, its agents were stopped and its work committed: "Stopped at limit". It
  /// continues only after the limit is raised; after the daily budget, once the day has room.
  LimitReached(RunLimit),
}

/// Why automatic execution stopped and the run waits for a person to decide how it goes on: allow
/// one more retry, return the work to an earlier agent step with a note, or stop the run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stuck {
  /// The step's work still failed after its failure route was taken `max_retries` times.
  RetriesExhausted(StepKey),
  /// The step broke again in the attempt that restarted it: its agent's turn failed without a
  /// `failed` verdict, or the step could not be started, committed or merged.
  BrokeAgain(StepKey),
  /// The step reported `unverified`: it could not check everything it is for, and its output lists what
  /// and why. No verdict on the work, so no failure route is taken and no retry is used.
  Unverified(StepKey),
  /// The step was not run again: its latest result was failed or unverified on the revision it would run
  /// on now, and nothing a person asked for changed since.
  Unchanged(StepKey),
}

impl Stuck {
  /// The step that failed.
  pub const fn step(&self) -> &StepKey {
    match self {
      Self::RetriesExhausted(step) | Self::BrokeAgain(step) | Self::Unverified(step) | Self::Unchanged(step) => step,
    }
  }

  /// How the run fails when the person stops it.
  pub fn failure(&self) -> RunFailure {
    match self {
      Self::RetriesExhausted(step) => RunFailure::RetriesExhausted(step.clone()),
      Self::BrokeAgain(step) | Self::Unverified(step) | Self::Unchanged(step) => RunFailure::StepFailed(step.clone()),
    }
  }

  /// Whether the person may accept the step as it is ([`super::StuckDecision::Accept`]): it gave no verdict on the
  /// work, or one the work has not changed since.
  pub const fn can_accept(&self) -> bool {
    matches!(self, Self::Unverified(_) | Self::Unchanged(_))
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RunStatus {
  Pending,
  Running,
  Paused(PauseReason),
  /// Its attempt ended for this reason and no automatic route is left; no agent works for it.
  WaitingForPerson(Stuck),
  /// Stopped without settling; the last checkpoint is kept and nothing after it counts as done.
  Interrupted(InterruptCause),
  Succeeded,
  Failed(RunFailure),
  Cancelled,
}

impl RunStatus {
  pub const fn is_terminal(&self) -> bool {
    matches!(self, Self::Succeeded | Self::Failed(_) | Self::Cancelled)
  }

  /// Whether an agent of the run works, waits for a person, or goes on once it resumes or a raised
  /// limit lets it: its worktrees are in use and cannot be removed. A run that ended has no live
  /// work, nor has an interrupted one, which only continues when a person asks for it (ORC-63).
  pub const fn has_live_work(&self) -> bool {
    !matches!(
      self,
      Self::Succeeded | Self::Failed(_) | Self::Cancelled | Self::Interrupted(_)
    )
  }

  /// A failed or cancelled run also permits starting a separate run ([`super::run_again`]).
  pub const fn can_run_again(&self) -> bool {
    matches!(self, Self::Failed(_) | Self::Cancelled)
  }
}

/// What a waiting step waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitReason {
  /// A tool call awaits an access approval.
  Approval,
  /// The step's result awaits a decision at its approval gate, e.g. plan approval.
  Plan,
  /// The agent asked a question and awaits a message.
  Message,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepStatus {
  Pending,
  Running,
  Waiting(WaitReason),
  Passed,
  Failed,
  /// Will not run to completion in its attempt: the attempt ended by retry, failure or cancel.
  Skipped,
}

impl StepStatus {
  pub const fn is_open(self) -> bool {
    matches!(self, Self::Pending | Self::Running | Self::Waiting(_))
  }
}

/// Why a step execution failed. Its step's failure route decides what happens next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepFailure {
  /// The agent CLI did not finish the step: a turn failed, it exited with a failure or it
  /// reported no result.
  AgentFailed,
  /// The agent finished and reported that it could not do the step.
  ReportedFailed,
  /// The agent, or a command tool's program, could not be started.
  NotStarted,
  /// Legacy: versions before step budgets stopped the run at its limit failed an execution that
  /// used its step's budget. Such failures are still read; none is recorded any more.
  StepBudget,
  /// The step's changes conflict with what other steps changed on the run's branch.
  MergeConflict,
  /// The step's changes could not be committed or merged.
  CheckpointFailed,
  /// A command tool's program exited with a code the tool does not expect, or did not finish
  /// within the tool's timeout.
  CommandFailed,
  /// The agent finished and reported `unverified`: it could not check everything the step is for.
  Unverified,
  /// The step was not run again on a revision its latest failed or unverified result was for.
  Unchanged,
}

impl StepFailure {
  /// Whether the step judged the work: its agent reported `failed`, or its command's exit code
  /// said so. Only a verdict follows the step's failure route; any other failure is a breakage,
  /// which restarts the step itself once (user decision 2026-10-02), or goes to the person.
  pub const fn is_verdict(self) -> bool {
    matches!(self, Self::ReportedFailed | Self::CommandFailed)
  }

  /// Whether the failure goes straight to the person: the step could not check the work, or was not run
  /// again on work that has not changed. Neither is a verdict nor a breakage.
  pub const fn waits_for_person(self) -> bool {
    matches!(self, Self::Unverified | Self::Unchanged)
  }

  /// Whether the failure is a breakage, which restarts the step once.
  pub const fn is_breakage(self) -> bool {
    !self.is_verdict() && !self.waits_for_person()
  }

  /// Whether a step that ended this way is not run again on the same revision: a verdict or a result its agent
  /// reported that needs a change of the work, or the person, before it can come out differently.
  pub const fn holds_for_revision(self) -> bool {
    matches!(self, Self::ReportedFailed | Self::Unverified | Self::Unchanged)
  }
}

/// Why an attempt was started.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptReason {
  Initial,
  /// `failed_step` failed and its failure route returned to `return_to`.
  Retry {
    failed_step: StepKey,
    return_to: StepKey,
  },
  /// The run had succeeded and a person asked the agent of `step` about it (Ask agent): its one
  /// execution continues the session of the step's passed one ([`super::Run::follow_up`]).
  FollowUp {
    step: StepKey,
  },
  /// A person's message resumes `step` after failure, then runs its dependent and unfinished steps.
  Recovery {
    step: StepKey,
  },
  /// `step` broke without a verdict on its work: it runs again as a fresh execution, then its
  /// dependent and unfinished steps.
  Restart {
    step: StepKey,
  },
  /// `failed_step` waited for a person, who returned the work to the agent step `return_to` with
  /// `note`, the instruction its prompt carries.
  Returned {
    failed_step: StepKey,
    return_to: StepKey,
    note: ReviewNote,
  },
  /// A person (or automation they let answer) accepted `step`, which waited for them unverified or unchanged, as
  /// it is: the attempt passes it with the accepted execution's work and result, then runs the steps after it and
  /// those that had not passed.
  Accepted {
    step: StepKey,
  },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttemptStatus {
  Active,
  /// A failure was routed back to an upstream step in a newer attempt.
  RoutedBack,
  Succeeded,
  Failed,
  Cancelled,
}

/// Who continued an interrupted run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinueTrigger {
  User,
  RunnerReconnected,
}
