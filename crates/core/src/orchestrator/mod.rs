//! The project's Orchestrator chat (`docs/design/orchester-workspace.md`, "Project orchestrator conversation"): a
//! conversation with an agent CLI started read-only in the project folder and given Orchester's domain tools for
//! that project only. Each message a person sends is one turn of the agent's CLI session, which the next turn
//! resumes; the person's messages, the agent's replies and the actions it took through the tools are kept with the
//! conversation in the project's database.

use std::time::Duration;

pub mod change;
pub mod chat;
pub mod instructions;
pub mod proposal;
pub mod report;
pub mod resume;
pub mod sent;
pub mod settings;
pub mod wait;

pub use self::{
  chat::{
    ActionOutcome, Chat, ChatAction, ChatEntry, ChatEvent, ChatSession, EntryKind, HeldWork, SessionChange,
    TurnFailure, TurnState, WaitTurn,
  },
  report::{DailyReport, ReportSchedule, ReportTime},
  resume::{ChatResume, Continuing, PersonsStop},
  sent::{FromOrchestrator, SentWay},
  settings::{OrchestratorSettings, codex_held_back, runs_orchestrator},
  wait::{AgentWait, RunEnd, StopReason, WaitKind},
};

/// The agentic turns one answer may take at most (`--max-turns`): reading, searching and calling tools, each a
/// turn. No setting: an answer that needs more is asked again in parts.
pub const MAX_TURNS: u32 = 30;

/// How long one answer may run at most; the runner ends its CLI then. No setting either.
pub const TIME_LIMIT: Duration = Duration::from_secs(10 * 60);
