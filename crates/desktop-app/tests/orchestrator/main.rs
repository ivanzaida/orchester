//! The Orchestrator chat (`TrG83`) and its settings (`Xir64`) headless, at the design's 1440 × 900, with the real
//! runner and `orchester-fake-claude` (`crates/runner/tests/support/fake_claude_chat.rs`) as the Orchestrator's
//! agent: a new conversation and its suggestions, a message answered and kept, the agent's actions through the
//! domain tools carried out or refused as for any client, a stopped and a failed answer, a send the runner never
//! took, History, and choosing what the Orchestrator runs on. Run with
//! `cargo test -p orchester-app --test orchestrator <filter>`; the first test builds the runner with its fakes.

mod changes;
mod chat;
mod decisions;
mod harness;
mod history;
mod proposal;
mod queue;
mod review;
mod settings;
mod start;
#[path = "../support/mod.rs"]
mod support;
