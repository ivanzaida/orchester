//! Messages between the desktop and a runner. The desktop sends [`Request`]s; the runner answers
//! each with an [`Event::Reply`] carrying the request's id, and on its own sends change
//! notifications. Notifications only say *that* something changed: the project database is the
//! source of truth, and the desktop reads the changed run from it (`docs/architecture/storage.md`,
//! "Write ownership").

use std::{fmt, num::NonZeroU32};

use orchester_core::{
  id::{IdempotencyKey, ResourceId, RunId, StepExecutionId, TaskId},
  permission::{AgentPermissions, capability::PermissionProblem},
  resource::StepKey,
  run::{InterruptCause, PermissionsApply, PlanDecision, PlanVersion, PreflightProblem, RunLimit, RunNumber},
  tokens::TokenBudget,
};
use serde::{Deserialize, Serialize};

use crate::work::WorkSummary;

/// Identifies a request within one connection, so its reply can be matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(u64);

impl RequestId {
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  pub const fn get(self) -> u64 {
    self.0
  }
}

impl fmt::Display for RequestId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "request {}", self.0)
  }
}

/// Desktop to runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
  pub id: RequestId,
  pub command: Command,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Command {
  /// The first request of a connection: the desktop starts listening to notifications, says
  /// which session this runner serves, which decides what happens to runs still recorded as
  /// running (`docs/architecture/storage.md`, "Lifecycle"), and how many agents may work at
  /// once. Answered with [`Reply::Attached`], followed by an [`Event::WorkSummary`].
  Attach {
    session: AttachSession,
    agent_slots: NonZeroU32,
  },
  /// Liveness check, answered with [`Reply::Pong`] carrying the same nonce.
  Ping { nonce: u64 },
  /// Starts a run of the queued task `task`. Idempotent by `key`: repeating it (a retry after
  /// a lost reply) answers with the run the first request created. Answered with
  /// [`Reply::RunStarted`] or [`Reply::Rejected`].
  StartRun { key: IdempotencyKey, task: TaskId },
  /// Pauses run `run` at its next checkpoint: the active agent invocation finishes, its work is
  /// committed, then the run is paused ("Pausing…" until then, [`Event::RunChanged`] tells when).
  /// Answered with [`Reply::Accepted`] once the pause is requested; asking again is harmless.
  PauseRun { run: RunId },
  /// Continues paused run `run`; a step it stopped in continues the agent's CLI session.
  /// Answered with [`Reply::Accepted`] once it runs again.
  ResumeRun { run: RunId },
  /// Cancels run `run`: its agent's process tree is ended, its worktree and branch stay. Answered
  /// with [`Reply::Accepted`] once the run is recorded as cancelled.
  CancelRun { run: RunId },
  /// Continues interrupted runs `runs` from their last checkpoints, as far as the agent slots
  /// and the daily budget allow. Answered with [`Reply::Continued`].
  ContinueRuns { runs: Vec<RunId> },
  /// Raises the limits of run `run`, stopped at a limit, and continues it. Answered with
  /// [`Reply::Accepted`] once it runs again.
  RaiseLimit {
    run: RunId,
    budget: TokenBudget,
    max_duration_ms: u64,
  },
  /// Raises, for run `run` only, the budget of the step it stopped at to `budget`, and continues
  /// it: the stopped agent's CLI session continues. Answered with [`Reply::Accepted`] once it
  /// runs again.
  RaiseStepBudget { run: RunId, budget: TokenBudget },
  /// Run `run` uses `permissions` for agent profile `profile` from the next invocation of each of
  /// its steps; with [`PermissionsApply::Now`] the profile's agents at work also restart at their
  /// next checkpoint, resuming their CLI sessions. Answered with [`Reply::Accepted`] once the
  /// change is recorded.
  ChangeRunPermissions {
    run: RunId,
    profile: ResourceId,
    permissions: AgentPermissions,
    apply: PermissionsApply,
  },
  /// Cancels the pending restarts of an Apply now for `profile` in run `run`: the change applies
  /// from the next invocations. Answered with [`Reply::Accepted`].
  KeepPermissionsForNextInvocation { run: RunId, profile: ResourceId },
  /// A person's `decision` on plan `version` of gated step execution `execution` of run `run`.
  /// Approval continues the run's graph; requested changes resume the step's agent session with
  /// the reviewer's note. Answered with [`Reply::Accepted`] once the decision is recorded, or
  /// [`Rejection::PlanNotAwaited`] when that plan does not wait for one (any more).
  DecidePlan {
    run: RunId,
    execution: StepExecutionId,
    version: PlanVersion,
    decision: PlanDecision,
  },
  /// The desktop stored a person's messages to the task of run `run`: the runner delivers them at
  /// the run's next invocation boundary. A step waiting for an answer to its agent's question
  /// resumes now; a run that succeeded works again on a follow-up of its last agent step (Ask
  /// agent); an agent at work reads them once its turn is over; a paused or stopped run keeps them
  /// queued until it continues. Answered with [`Reply::Accepted`] once that is arranged, or
  /// [`Rejection::RunEnded`] for a run that failed or was cancelled.
  DeliverMessages { run: RunId },
  /// Stop the runner. It answers with [`Event::Stopped`] and exits once the desktop closes
  /// the connection.
  Shutdown { mode: ShutdownMode },
}

/// Which session a runner serves, from the desktop's side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttachSession {
  /// The project's first runner in the desktop's session. Runs still recorded as running were
  /// cut off by the end of the previous session and wait for a person to continue them.
  Fresh { previous_end: SessionEnd },
  /// The desktop started this runner after the project's previous runner dropped. Runs that
  /// runner left running were interrupted by its loss and continue on their own.
  Reconnect,
}

impl AttachSession {
  /// Why a run still recorded as running when this runner attaches stopped: the previous
  /// runner's loss, Quit now, or else a crash. A run left running by a session that says it
  /// ended cleanly or with paused runs lost its runner without anyone recording it.
  pub const fn interrupt_cause(self) -> InterruptCause {
    match self {
      Self::Reconnect => InterruptCause::RunnerLost,
      Self::Fresh {
        previous_end: SessionEnd::QuitNow,
      } => InterruptCause::QuitNow,
      Self::Fresh {
        previous_end: SessionEnd::Crashed | SessionEnd::Clean | SessionEnd::RunsPaused,
      } => InterruptCause::Crashed,
    }
  }
}

/// How the desktop's previous session ended, as its session log recorded it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEnd {
  /// There was none, or it closed without active runs.
  Clean,
  /// The user waited for agents to save; the runs were paused.
  RunsPaused,
  /// The user chose Quit now.
  QuitNow,
  /// It never recorded its end: a crash, power loss or a killed process.
  Crashed,
}

/// How the runner stops, matching the close confirmation in `docs/architecture/storage.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownMode {
  /// "Wait for agents to save": stop scheduling and let active steps reach a checkpoint.
  FinishAtCheckpoints,
  /// "Quit now": terminate agents without a checkpoint.
  Immediate,
}

/// Runner to desktop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Event {
  /// The answer to request `id`.
  Reply { id: RequestId, reply: Reply },
  /// The last event of a connection: the runner has stopped all work and exits.
  Stopped { mode: ShutdownMode },
  /// Run `run` of task `task` changed state (status, step, attempt); read it from the database.
  RunChanged { run: RunId, task: TaskId },
  /// Run `run`'s transcript grew to `sequence` entries. Coalesced: one notification can stand
  /// for many entries.
  AgentActivity { run: RunId, sequence: u64 },
  /// The runs this runner executes and its agent slots. Sent after attaching and whenever they
  /// change; each one replaces the one before.
  WorkSummary(WorkSummary),
  /// Gated step execution of run `run` proposed plan `version`, which waits for a person: the
  /// status bar says so ("Plan ready for review · ORC-12", `task_key`). The run itself changed as
  /// an [`Event::RunChanged`] says.
  PlanProposed {
    run: RunId,
    task: TaskId,
    task_key: String,
    version: PlanVersion,
  },
  /// Run `run` (number `number`) of task `task` succeeded: it is ready for review, and the status
  /// bar says so ("Run #128 ready for review"). The run itself changed as an [`Event::RunChanged`]
  /// says.
  RunReady {
    run: RunId,
    task: TaskId,
    number: RunNumber,
  },
  /// The agent (`agent`, its role) of a step of run `run` asked a question and its step waits for
  /// the answer: the status bar says so ("Builder asks · ORC-12", `task_key`). The run itself
  /// changed as an [`Event::RunChanged`] says.
  QuestionAsked {
    run: RunId,
    task: TaskId,
    task_key: String,
    agent: String,
  },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Reply {
  /// Runs this runner is executing right now; `interrupted`, the runs attaching recorded as
  /// interrupted, and `continued`, the interrupted runs it continued because the previous
  /// runner dropped.
  Attached {
    active_runs: Vec<RunId>,
    interrupted: Vec<RunId>,
    continued: Vec<RunId>,
  },
  Pong {
    nonce: u64,
  },
  /// The run of a [`Command::StartRun`], newly created or created by an earlier request with
  /// the same key.
  RunStarted {
    run: RunId,
  },
  /// The run control command for `run` was carried out.
  Accepted {
    run: RunId,
  },
  /// The answer to [`Command::ContinueRuns`]: the runs that run again, and the ones that do not
  /// with why.
  Continued {
    continued: Vec<RunId>,
    refused: Vec<RefusedRun>,
  },
  Rejected(Rejection),
}

/// A run [`Command::ContinueRuns`] did not continue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefusedRun {
  pub run: RunId,
  pub rejection: Rejection,
}

/// Why the runner did not do what a request asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Rejection {
  TaskNotFound,
  /// Only queued tasks start a run.
  TaskNotQueued,
  /// The run cannot start until these are resolved.
  Preflight {
    problems: Vec<PreflightProblem>,
  },
  RunNotFound,
  /// The run's stored record cannot be read back, so nothing can change it; the record is kept
  /// unchanged. `detail` says why, for the log.
  RunUnreadable {
    detail: String,
  },
  /// Pause needs a running run.
  RunNotRunning,
  /// Resume and a raised limit need a paused run; a raised step budget one stopped at a step's
  /// budget.
  RunNotPaused,
  /// Continuing from a checkpoint needs an interrupted run.
  RunNotInterrupted,
  /// The run has ended: there is nothing to cancel, and no agent reads messages to it.
  RunEnded,
  /// The run stopped at `limit`; it continues only with a raised limit.
  LimitNotRaised {
    limit: RunLimit,
  },
  /// A raised limit must be above what the run already used.
  LimitNotAboveUsed {
    limit: RunLimit,
  },
  /// A run budget can be at most the project's maximum run budget.
  BudgetAboveMaximum {
    max: TokenBudget,
  },
  /// The project's runs used today's token budget `budget`: runs start or continue again once
  /// the day has room (tomorrow, or a raised daily budget).
  DailyBudgetUsed {
    budget: TokenBudget,
  },
  /// All `slots` agent slots of the runner are in use.
  NoAgentSlot {
    slots: NonZeroU32,
  },
  /// No step of the run uses the agent profile whose permissions were to change.
  ProfileNotInRun,
  /// The connection's CLI cannot enforce the permissions for `step`.
  PermissionsNotEnforceable {
    step: StepKey,
    problem: PermissionProblem,
  },
  /// Apply now needs an agent of the profile at work in a running run.
  NoAgentToRestart,
  /// No change of the profile's permissions waits for a checkpoint.
  NoPendingPermissions,
  /// The plan does not wait for a decision: a newer version exists, it was decided already, or
  /// its step works on it again.
  PlanNotAwaited,
  /// The runner is stopping and starts no work.
  ShuttingDown,
  /// Starting failed in the runner (database, git worktree); its log has the details.
  Failed {
    detail: String,
  },
}
