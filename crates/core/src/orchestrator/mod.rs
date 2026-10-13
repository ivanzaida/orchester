//! The project's Orchestrator chat (`docs/design/orchester-workspace.md`, "Project orchestrator conversation"): a
//! conversation with an agent CLI started read-only in the project folder and given Orchester's domain tools for
//! that project only. Each message a person sends is one turn of the agent's CLI session, which the next turn
//! resumes; the person's messages, the agent's replies and the actions it took through the tools are kept with the
//! conversation in the project's database.

pub mod chat;
pub mod instructions;
pub mod settings;

pub use self::{
  chat::{ActionOutcome, Chat, ChatAction, ChatEntry, ChatSession, EntryKind, TurnFailure, TurnState},
  settings::{OrchestratorSettings, runs_orchestrator},
};
