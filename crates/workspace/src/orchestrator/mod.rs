//! The project's Orchestrator chat (`docs/design/orchester-workspace.md`, "Project orchestrator conversation";
//! `orchester_core::orchestrator`): what it runs on, its conversations, sending a message and stopping the answer,
//! and the domain tools its agent gets. The project's runner runs the agent; the app serves its tools.

mod change;
pub mod chat;
mod keep;
pub mod proposal;
pub mod send;
pub mod settings;
pub mod tools;
