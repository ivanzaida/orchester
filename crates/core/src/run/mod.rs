//! Execution of a task: a run pins its configuration in a [`RunSnapshot`], executes the
//! pipeline in attempts of step executions, records checkpoints and token usage, and changes only
//! through typed transitions. Every accepted transition yields the [`RunEvent`]s to append;
//! the event log is the source of truth and the current state is derivable from it
//! ([`Run::replay`]). See `docs/architecture/storage.md` (Execution, State machines).

mod active_clock;
mod again;
mod answer;
mod apply;
mod attempt;
mod branch;
mod error;
mod event;
mod first_edit;
mod follow_up;
mod invocation;
mod not_started;
mod number;
mod permissions;
mod phase;
mod plan;
mod preflight;
mod progress;
mod prompt;
mod recovery;
mod result;
mod return_ended;
pub mod review;
mod schedule;
mod snapshot;
mod state;
mod status;
mod step;
mod stuck;
mod tool_approval;
mod tool_step;
mod transition;
mod unreadable;
mod unverified;
mod usage;

pub use self::{
  active_clock::ActiveClock,
  again::{RunAgainBlock, RunAgainInput, can_run_again_after, holds_task, run_again},
  answer::{
    AlwaysAllowError, AlwaysUnavailable, Answerer, ApprovalAnswer, ApprovalRule, ApprovalRuleId, ApprovalSubject,
    AutoAsk, AutoVerdict, AutomationAnswerError, DecidedBy, ProjectRule, ProjectRuleId, ProjectScope, RuleScope,
    SimilarUnavailable, ToolDecision, ToolReach, UnknownAnswer, allowing_command, allowing_tool, always_prefixes,
    answering, answering_in_project, auto_approves, auto_verdict, codex_denial, denial_message, listing, permitted,
  },
  attempt::{Attempt, Checkpoint, StepExecution},
  branch::{BranchName, BranchNameError},
  error::RunError,
  event::{EventSequence, PlannedExecution, RunCreated, RunEvent, RunEventKind},
  first_edit::{EditProgress, FirstEdit, FirstEditTracker},
  invocation::{
    ActivityEntry, Invocation, InvocationEngine, InvocationKind, InvocationTiming, active_time, execution_tokens,
  },
  not_started::{GitMessage, WorktreeFailure},
  number::RunNumber,
  permissions::{AppliedPermissions, PermissionsApply, PermissionsChangedBy, RunPermissions},
  phase::TaskPhase,
  plan::{Plan, PlanDecision, PlanReview, PlanVersion, ReviewNote, ReviewNoteError},
  preflight::{PreflightInput, PreflightProblem, preflight},
  progress::{ProgressError, StepProgress},
  prompt::{
    AssembledPrompt, BaseMoved, CONTINUE_PROMPT, CodeIndexPrompt, Composition, DeliveredMessages, DependencyOutput,
    ExcludedInstruction, FailureReport, InstructionSource, InstructionVersion, InvocationPrompt, MERGE_SAFELY,
    MessageOccasion, NativePrompt, NativeSetting, PipelineInfo, PlanRevision, ProfileInfo, PromptAssemblyError,
    PromptInput, PromptPart, PromptSection, PromptSource, ResultDelivery, RetryReport, ReturnedWork, RunContext,
    SkillListing, WorkingTools, notice_text,
  },
  result::{STEP_RESULT_SCHEMA, StepOutcome, StepOutput, StepResult, UnverifiedCheck},
  snapshot::{PinnedVersion, PublishedVersions, RunSnapshot, SnapshotError, StepExecutor, StepSnapshot},
  state::{NewRun, Run, StoredRun},
  status::{
    AttemptReason, AttemptStatus, ContinueTrigger, InterruptCause, MergeConflicts, PauseReason, RunFailure, RunLimit,
    RunStatus, StepFailure, StepStatus, Stuck, WaitReason,
  },
  stuck::{AttemptStart, MAX_AUTOMATIC_ATTEMPTS, RetryReportSource, StuckDecision, attempt_starts},
  tool_approval::{
    ARGUMENTS_CEILING, CommandShell, DecidedToolApproval, NativeCommandAsk, PendingToolApproval, ToolApprovalDecision,
    UnknownShell, cut_arguments, recorded_arguments,
  },
  tool_step::{CapturedOutput, CommandEnd, OutputCapture, StepAt, ToolCommand},
  transition::RaisedLimits,
  unreadable::{ListedRun, UnreadableRun},
  unverified::AcceptRefusal,
  usage::TokenUsage,
};
