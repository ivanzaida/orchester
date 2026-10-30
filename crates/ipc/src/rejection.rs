//! Why the runner did not do what a request ([`crate::Command`]) asked, as the desktop reads it from
//! [`crate::Reply::Rejected`].

use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  permission::capability::PermissionProblem,
  question::AnswerError,
  resource::StepKey,
  run::{AcceptRefusal, AutoAsk, PreflightProblem, RunAgainBlock, RunLimit, WorktreeFailure},
  tokens::TokenBudget,
};
use serde::{Deserialize, Serialize};

/// Why the runner did not do what a request asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Rejection {
  TaskNotFound,
  /// Only queued tasks start a run.
  TaskNotQueued,
  /// The task changed since it was checked for this run: nothing started.
  TaskChangedMeanwhile,
  /// A reviewed group's order, concurrency or shared cap holds this task.
  TaskGroupWaiting {
    hold: orchester_core::task_group::GroupHold,
  },
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
  /// A step stopped at its account's plan limit continues only on an agent connection of the same CLI that runs on
  /// another account and can start now.
  NotAnotherAccount,
  /// Continuing from a checkpoint needs an interrupted run.
  RunNotInterrupted,
  /// A decision on a stuck run needs a run that waits for a person.
  RunNotWaiting,
  /// The run has ended: there is nothing to cancel, and no agent reads messages to it.
  RunEnded,
  /// Returning the work of a run needs one that ended: succeeded, or failed after it started.
  RunNotEnded,
  /// The run's work was merged into its base branch: it is not returned to a step any more.
  RunMerged,
  /// A merge waits in the run's worktree for a person to resolve it: the run does not continue until it is
  /// committed or aborted.
  WorktreeInMerge,
  /// The run changed since it was read for the decision: nothing was decided.
  RunChangedMeanwhile,
  /// The run's worktree was removed, so its agent cannot continue there: a follow-up (Ask agent)
  /// is refused before the run changes, and its messages stay queued.
  WorktreeRemoved,
  /// The task cannot run again now, as `block` says.
  RunAgainBlocked {
    block: RunAgainBlock,
  },
  /// Run again did not remove the worktree at `path` of the run before it: changes appeared in it
  /// after the person saw it. No run was started.
  WorktreeChanged {
    path: String,
  },
  /// Git did not remove the worktree at `path` of the run before Run again (its message,
  /// `detail`). No run was started.
  WorktreeNotRemoved {
    path: String,
    detail: String,
  },
  /// The run stopped at `limit`; it continues only with a raised limit.
  LimitNotRaised {
    limit: RunLimit,
  },
  /// A raised limit must be above what the run already used.
  LimitNotAboveUsed {
    limit: RunLimit,
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
  /// The disk the workspaces are on has `free_gb` gigabytes free, below the `needed_gb` the project keeps (ORC-114):
  /// no step starts until there is room.
  LowDisk {
    free_gb: u64,
    needed_gb: u64,
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
  /// The selected tool request no longer waits: its agent can no longer receive an answer, so it ended
  /// without one.
  ToolApprovalNotAwaited,
  /// The selected tool request was answered already, by a person or by Orchester.
  ToolApprovalAnswered,
  /// Automation gave a wider answer than approving once or denying, which only a person gives.
  AutomationWider,
  /// The question request does not wait any more: its agent can no longer take an answer, or there is no
  /// such request.
  QuestionNotAwaited,
  /// The question request was answered already.
  QuestionAnswered,
  /// The answers do not fit the request's questions, as `error` says.
  InvalidAnswer {
    error: AnswerError,
  },
  /// Automation tried to approve a request that auto-approval leaves to a person, for reason `ask`:
  /// only a person approves it (automation may deny it).
  PersonOnly {
    ask: AutoAsk,
  },
  /// The Orchestrator chat's message is not kept in the project's database.
  ChatMessageNotFound,
  /// Another turn of the conversation is under way; a conversation answers one message at a time.
  ChatBusy,
  /// No connection and model were chosen for the project's Orchestrator.
  NoOrchestrator,
  /// *Stop turn* needs an agent of the run at work in a turn: none works now (a command tool runs, the step waits
  /// for a person, the run is paused or stopping).
  NoTurnToStop,
  /// The agent at work runs where a turn cannot be stopped alone and continued (a native agent): Pause stops the run
  /// at its next checkpoint instead.
  TurnNotStoppable,
  /// Accepting the waiting step was refused: the work that would pass with it is not the work its result was for.
  NotAcceptable {
    why: AcceptRefusal,
  },
  /// The runner is stopping and starts no work.
  ShuttingDown,
  /// [`crate::Command::StopIfIdle`] while the runner has work.
  Busy,
  /// The run was recorded, but its worktree could not be created, `why`: it failed before it started
  /// ([`orchester_core::run::RunFailure::WorktreeNotCreated`]) and runs again only as a new run.
  RunNotStarted {
    run: RunId,
    why: WorktreeFailure,
  },
  /// Starting failed in the runner (database, git worktree); its log has the details.
  Failed {
    detail: String,
  },
  /// The project's browser cannot be used (protocol 34): no Chromium-based browser is installed, or it could not
  /// start, as `detail` says.
  BrowserUnavailable {
    detail: String,
  },
  /// The project's browser has no such tab (any more).
  NoSuchTab,
}
