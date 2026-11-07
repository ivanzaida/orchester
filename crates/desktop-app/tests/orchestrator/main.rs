//! The Orchestrator chat (`TrG83`) and its settings (`Xir64`) headless, at the design's 1440 × 900, with the real
//! runner and `orchester-fake-claude` (`crates/runner/tests/support/fake_claude_chat.rs`) or `orchester-fake-codex`
//! (`fake_codex_chat.rs`) as the Orchestrator's agent: a new conversation and its suggestions, a message answered and
//! kept, the agent's actions through the domain tools carried out or refused as for any client, a stopped and a
//! failed answer, a send the runner never took, the composer's drafts, History, links in its replies, choosing what
//! the Orchestrator runs on, and the turn an agent's wait starts by itself. Run with
//! `cargo test -p orchester-app --test orchestrator <filter>`; the first test builds the runner with its fakes.

mod attachments;
mod autoscroll;
mod changes;
mod chat;
mod code_block;
mod connections;
mod context;
mod continuation;
mod copy;
mod decisions;
mod drafts;
mod export;
mod first_send;
mod follow;
mod fork;
mod git;
mod harness;
mod history;
mod history_archive;
mod history_leads;
mod history_rows;
mod history_search;
mod layout;
mod leads;
mod led;
mod lines;
mod links;
mod mcp_servers;
mod mentions;
mod menu_layout;
mod open_long;
mod paste;
mod plan_only;
mod proposal;
mod queue;
mod reading;
mod references;
mod report;
mod repository;
mod review;
mod rewind;
mod runs_on;
mod runs_on_search;
mod scroll;
mod settings;
mod skills;
mod slash;
mod standing;
mod start;
#[path = "../support/mod.rs"]
mod support;
mod tall_reply;
mod task_events;
mod todo;
mod unwatch;
mod waits;
mod waits_on;
mod watch_lines;
mod watch_module;
mod watch_task;
mod watching;
mod while_answering;
