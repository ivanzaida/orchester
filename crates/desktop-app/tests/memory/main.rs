//! The Memory screens headless, on the QA seed's `memory` and `tasks` scenarios: a task's Memory tab, the project's
//! Memory page with its tabs and its link to the memory settings, their empty states, their failure and refusal states,
//! a task's Artifacts tab, and their theme roles. No runner is connected (its program does not exist). Needs `git`.
//!
//! `cargo test -p orchester-app --test memory memory`

mod empty;
mod harness;
#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod project;
mod states;
#[path = "../support/mod.rs"]
mod support;
mod tabs;
mod task_tab;
mod theme;
