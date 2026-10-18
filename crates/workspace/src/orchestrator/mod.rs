//! The project's Orchestrator chat (`docs/design/orchester-workspace.md`, "Project orchestrator conversation";
//! `orchester_core::orchestrator`): what it runs on, its conversations, sending a message and stopping the answer,
//! the domain tools its agent gets, and the waits of the project's agents it follows. The project's runner runs the
//! agent; the app serves its tools.

mod blocked;
mod change;
pub mod chat;
mod duplicate;
pub mod facts;
mod keep;
pub mod proposal;
pub mod recover;
pub mod send;
pub mod settings;
pub mod tools;
pub mod waits;
mod widening;
