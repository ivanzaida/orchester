//! The project's Orchestrator chat (`docs/design/orchester-workspace.md`, "Project orchestrator conversation";
//! `orchester_core::orchestrator`): what it runs on, its conversations, sending a message and stopping the answer,
//! writing to it while it answers (`queue`), the domain tools its agent gets, its own tools that make the project's
//! folder a git repository, work with git in it, change its MCP servers and apply a proposal the person approved in
//! their own words, the waits of the
//! project's agents and the ends of its runs it follows, the project's daily report, and the person's watch commands
//! it applies itself, or its agent's `watch` and `unwatch` at the person's request (`watch`). The project's runner
//! runs the agent; the app serves its tools.

mod acts;
pub mod approval;
pub mod auto_merge;
mod blocked;
mod change;
pub mod chat;
pub mod compact;
mod created;
mod duplicate;
mod events;
pub mod facts;
pub mod fork;
pub mod git;
mod guard;
mod keep;
mod leads;
mod led_start;
mod mcp_servers;
mod merge_conflicts;
mod own_project;
pub mod plan;
pub mod proposal;
pub mod queue;
pub mod recover;
pub mod report;
pub mod repository;
pub mod rewind;
mod run_ends;
pub mod runs_on;
pub mod search;
pub mod send;
pub mod settings;
pub mod stalls;
pub mod team_status;
mod told;
pub mod tools;
pub mod waits;
pub mod waits_on;
pub mod watch;
pub mod watch_ends;
pub mod watching;
mod widening;
