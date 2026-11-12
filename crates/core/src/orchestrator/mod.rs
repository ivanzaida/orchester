//! The project's Orchestrator chat (`docs/design/orchester-workspace.md`, "Project orchestrator conversation"): a
//! conversation with an agent CLI started read-only in the project folder and given Orchester's domain tools for
//! that project only. Each message a person sends is one turn of the agent's CLI session, which the next turn
//! resumes; the person's messages, the agent's replies and the actions it took through the tools are kept with the
//! conversation in the project's database.

use std::time::Duration;

pub mod change;
pub mod chat;
mod failure;
pub mod fork;
pub mod git;
pub mod instructions;
pub mod leads;
pub mod mcp;
pub mod merging;
pub mod plan;
pub mod proposal;
pub mod report;
pub mod resume;
pub mod rewind;
pub mod run_git;
pub mod sent;
pub mod settings;
pub mod task_event;
pub mod wait;
pub mod watch;

pub use self::{
  chat::{
    ActionOutcome, BlankTitle, Chat, ChatAction, ChatApproval, ChatEntry, ChatEvent, ChatSession, ChatTitle,
    CompactionState, EntryKind, HeldWork, Queued, SessionChange, ToolAnswer, TurnFailure, TurnState, WaitTurn,
    WaitsOnRow,
  },
  leads::{Lead, Leads},
  merging::{AcceptedUnverified, MergeLeft, Merging},
  plan::{ChatPlan, PlanState, PlanStep, StepKind, TurnMode},
  report::{DailyReport, ReportSchedule, ReportTime},
  resume::{ChatResume, Continuing, PersonsStop},
  rewind::{Dropped, RewindPoint, RewoundTo},
  sent::{FromOrchestrator, SentWay},
  settings::{NoProgressError, OrchestratorSettings, WatchSettings, runs_orchestrator},
  task_event::{TaskEvent, TaskEventKind},
  wait::{AgentWait, RunEnd, StopReason, WaitKind},
  watch::{Claim, Subscription, Subscriptions, WatchTarget},
};

/// The agentic turns one answer on Claude Code may take at most (`--max-turns`): reading, searching and calling tools,
/// each a turn. No setting: an answer that needs more is asked again in parts. Codex App Server has no such limit; its
/// answer is bounded by [`TIME_LIMIT`] alone.
pub const MAX_TURNS: u32 = 30;

/// How long one answer may run at most; the runner ends its CLI then. No setting either.
pub const TIME_LIMIT: Duration = Duration::from_secs(10 * 60);
