//! Native agents in the runner against the fake model server (`docs/architecture/native-agent.md` §8,
//! layer 4): a step run to its result with the file tools, commands that ask a person (approved, denied),
//! a conversation resumed after a pause, and *Test connection*. No test reaches a real model API. Needs `git`.
//!
//! `cargo test -p orchester-runner --features test-agent --test native native_`

#[allow(dead_code, reason = "shared with the agent crate's tests, which use all of it")]
#[path = "../../../agent/tests/anthropic_wire/events.rs"]
mod events;
#[allow(dead_code, reason = "shared with the agent crate's tests, which use all of it")]
#[path = "../../../agent/tests/fake_server/mod.rs"]
mod server;
#[path = "../support/mod.rs"]
mod support;

mod approvals;
mod check;
mod resume;
mod step;

use std::path::Path;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::RunId,
  permission::AgentPermissions,
  run::ActivityEntry,
  store::ActivityStore,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use self::{
  server::{FakeServer, Part, Script},
  support::{
    Setup, TestProject,
    native::{self, NativeApi},
    project,
  },
};

/// A project whose Builder runs on the Anthropic API at `server`, with `permissions` if given.
pub fn native_project(brief: &'static str, permissions: Option<AgentPermissions>, server: &FakeServer) -> TestProject {
  let (secret, value) = native::key_secret();
  let api = NativeApi {
    base_url: server.base_url(),
    key: secret.id(),
  };
  project(&Setup {
    provider: AgentProvider::AnthropicApi,
    native: Some(api),
    secrets: vec![(secret, value)],
    permissions,
    ..Setup::new(brief)
  })
}

/// A fake server playing `script`, with Opus 5.5 in its catalog.
pub async fn serve(script: Script) -> FakeServer {
  FakeServer::start(script, events::catalog(), 10).await
}

/// A turn that calls the tools `calls` (id, name, input).
pub fn calls(calls: &[(&str, &str, Value)]) -> Vec<Part> {
  let blocks = calls
    .iter()
    .enumerate()
    .map(|(index, (id, name, input))| {
      let index = u64::try_from(index).expect("a few calls");
      events::tool_use(index, id, name, &input.to_string(), true)
    })
    .collect();
  events::response(blocks, "tool_use")
}

/// A turn that ends the step with a passed result.
pub fn finish(id: &str) -> Vec<Part> {
  let result = json!({"outcome": "done", "summary": "Said hello.", "completed": null, "decisions": null,
    "open_questions": null, "next_agent_should_know": null});
  calls(&[(id, "finish_step", result)])
}

/// `run_command`'s input for `command`.
pub fn command(command: &str) -> Value {
  json!({"command": command, "shell": null, "workdir": null, "timeout_seconds": null})
}

/// The run's transcript.
pub fn activity(root: &Path, run: RunId) -> Vec<ActivityEntry> {
  ProjectDb::open(root).expect("open").activity(run, 0).expect("activity")
}

/// Every event of the transcript, as JSON text: what a search for a secret value reads.
pub fn activity_text(entries: &[ActivityEntry]) -> String {
  entries
    .iter()
    .map(|entry| serde_json::to_string(&entry.event).expect("an event serializes"))
    .collect::<Vec<_>>()
    .join("\n")
}

/// The events of the transcript that `pick` keeps.
pub fn picked<T>(entries: &[ActivityEntry], pick: impl Fn(&AgentEvent) -> Option<T>) -> Vec<T> {
  entries.iter().filter_map(|entry| pick(&entry.event)).collect()
}

/// The text of every tool result the request sent.
pub fn tool_results(request: &server::Recorded) -> Vec<String> {
  let body = request.json();
  body["messages"]
    .as_array()
    .expect("messages")
    .iter()
    .flat_map(|message| message["content"].as_array().cloned().unwrap_or_default())
    .filter(|block| block["type"] == json!("tool_result"))
    .map(|block| block["content"].to_string())
    .collect()
}
