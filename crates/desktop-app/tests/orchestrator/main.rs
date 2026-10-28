//! The Orchestrator chat (`TrG83`) and its settings (`Xir64`) headless, at the design's 1440 × 900, with the real
//! runner and `orchester-fake-claude` (`crates/runner/tests/support/fake_claude_chat.rs`) or `orchester-fake-codex`
//! (`fake_codex_chat.rs`) as the Orchestrator's agent: a new conversation and its suggestions, a message answered and
//! kept, the agent's actions through the domain tools carried out or refused as for any client, a stopped and a
//! failed answer, a send the runner never took, the composer's drafts, History, links in its replies, choosing what
//! the Orchestrator runs on, and the turn an agent's wait starts by itself. Run with
//! `cargo test -p orchester-app --test orchestrator <filter>`; the first test builds the runner with its fakes.

mod attachments;
mod changes;
mod chat;
mod code_block;
mod connections;
mod continuation;
mod decisions;
mod drafts;
mod follow;
mod git;
mod harness;
mod history;
mod layout;
mod lines;
mod links;
mod mcp_servers;
mod open_long;
mod paste;
mod proposal;
mod queue;
mod reading;
mod references;
mod report;
mod repository;
mod review;
mod runs_on;
mod scroll;
mod settings;
mod skills;
mod standing;
mod start;
#[path = "../support/mod.rs"]
mod support;
mod tall_reply;
mod waits;
mod while_answering;
