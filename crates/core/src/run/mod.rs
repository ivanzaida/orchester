//! Execution of a task: a run pins its configuration in a [`RunSnapshot`], executes the
//! pipeline in attempts of step executions, records checkpoints and token usage, and changes only
//! through typed transitions. Every accepted transition yields the [`RunEvent`]s to append;
//! the event log is the source of truth and the current state is derivable from it
//! ([`Run::replay`]). See `docs/architecture/storage.md` (Execution, State machines).

mod again;
mod apply;
mod attempt;
mod branch;
mod counts;
mod error;
mod event;
mod first_edit;
mod follow_up;
mod invocation;
mod number;
mod permissions;
mod phase;
mod plan;
mod preflight;
mod prompt;
mod recovery;
mod result;
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
mod usage;

pub use self::{
  again::{RunAgainBlock, RunAgainInput, run_again},
  attempt::{Attempt, Checkpoint, StepExecution},
  branch::{BranchName, BranchNameError},
  counts::RunCounts,
  error::RunError,
  event::{EventSequence, PlannedExecution, RunCreated, RunEvent, RunEventKind},
  first_edit::{EditProgress, FirstEdit, FirstEditTracker},
  invocation::{ActivityEntry, Invocation, InvocationKind, InvocationTiming, active_time, execution_tokens},
  number::RunNumber,
  permissions::{AppliedPermissions, PermissionsApply, PermissionsChangedBy, RunPermissions},
  phase::TaskPhase,
  plan::{Plan, PlanDecision, PlanReview, PlanVersion, ReviewNote, ReviewNoteError},
  preflight::{PreflightInput, PreflightProblem, preflight},
  prompt::{
    AssembledPrompt, CONTINUE_PROMPT, CodeIndexPrompt, Composition, DeliveredMessages, DependencyOutput,
    ExcludedInstruction, FailureReport, InstructionSource, InstructionVersion, InvocationPrompt, MessageOccasion,
    PipelineInfo, PlanRevision, ProfileInfo, PromptAssemblyError, PromptInput, PromptPart, PromptSection, PromptSource,
    RetryReport, RunContext, SkillListing,
  },
  result::{STEP_RESULT_SCHEMA, StepOutcome, StepOutput, StepResult},
  snapshot::{PinnedVersion, PublishedVersions, RunSnapshot, SnapshotError, StepExecutor, StepSnapshot},
  state::{NewRun, Run, StoredRun},
  status::{
    AttemptReason, AttemptStatus, ContinueTrigger, InterruptCause, PauseReason, RunFailure, RunLimit, RunStatus,
    StepFailure, StepStatus, Stuck, WaitReason,
  },
  stuck::{AttemptStart, MAX_AUTOMATIC_ATTEMPTS, RetryReportSource, StuckDecision, attempt_starts},
  tool_approval::{
    ARGUMENTS_CEILING, CommandShell, NativeCommandAsk, PendingToolApproval, ToolApprovalDecision, UnknownShell,
    cut_arguments, recorded_arguments,
  },
  tool_step::{CapturedOutput, CommandEnd, OutputCapture, StepAt, ToolCommand},
  transition::RaisedLimits,
  unreadable::{ListedRun, UnreadableRun},
  usage::TokenUsage,
};
