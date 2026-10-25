//! The project's Orchestrator chat (`docs/design/orchester-workspace.md`, "Project orchestrator conversation";
//! `orchester_core::orchestrator`): what it runs on, its conversations, sending a message and stopping the answer,
//! writing to it while it answers (`queue`), the domain tools its agent gets, its own tools that make the project's
//! folder a git repository and work with git in it, the waits of the
//! project's agents and the ends of its runs it follows, and the project's daily report. The project's runner runs
//! the agent; the app serves its tools.

pub mod auto_merge;
mod blocked;
mod change;
pub mod chat;
mod duplicate;
pub mod facts;
pub mod git;
mod guard;
mod keep;
mod own_project;
pub mod proposal;
pub mod queue;
pub mod recover;
pub mod report;
pub mod repository;
mod run_ends;
pub mod send;
pub mod settings;
mod told;
pub mod tools;
pub mod waits;
mod widening;
