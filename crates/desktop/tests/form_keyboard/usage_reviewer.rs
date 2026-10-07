//! What auto mode's reviewer spent, in the desktop: Usage's *Where tokens go* gives it a row of its own
//! among the profiles (*Auto mode reviewer · claude-haiku-4-5*) and its totals count it, the invocation
//! panel of the Conversation's Details has a line for it, and `get_run` on Orchester's MCP server counts
//! it in the run's tokens with its part apart. Seeded: a run whose agent reported a turn and one of whose
//! requests the reviewer approved, both today.
//! `cargo test -p orchester-desktop --test form_keyboard usage_reviewer`.

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::RunId,
  resource::ModelId,
  run::{
    PendingToolApproval, TokenUsage,
    review::{ReviewEnd, ReviewRecord},
  },
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use crate::{
  app_mcp::{agent, ok},
  github::{contains_text, shows},
  harness::Desktop,
  narrow_answers::{details, open_conversation},
  native_run::native_run,
};

fn usage(input: u64, cached_input: u64, output: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input,
    output,
  }
}

/// A run whose agent spent 3,500 counted tokens today and whose reviewer spent 1,050 on one request.
fn reviewed_run() -> (Desktop, RunId) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project_and_app_mcp(|repo| {
    let (run, mut invocation) = native_run(repo);
    let mut db = ProjectDb::open(repo).expect("open");
    let now = Timestamp::now();
    invocation.usage = usage(4_000, 1_000, 500);
    db.save_invocation(&invocation).expect("the agent's usage");
    let turn = AgentEvent::TurnCompleted {
      usage: invocation.usage,
    };
    db.append_activity(&invocation, &[(now, turn)]).expect("its turn");
    let request = PendingToolApproval {
      id: format!("{}:call_1", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: "command".to_owned(),
      action: "cargo build".to_owned(),
      target: None,
      requested_at: now,
      native: None,
    };
    db.request_tool_approval(&request).expect("request");
    let review = ReviewRecord {
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      approval: request.id,
      provider: AgentProvider::ClaudeCodeCli,
      model: ModelId::new("claude-haiku-4-5").expect("model"),
      started_at: now,
      ended_at: now,
      usage: Some(usage(1_200, 200, 50)),
      end: ReviewEnd::Approved,
    };
    db.record_reviewer_usage(&review).expect("the review's cost");
    seeded = Some(run.id());
  });
  (desktop, seeded.expect("seeded"))
}

#[test]
fn usage_reviewer_has_a_row_of_its_own_and_counts_in_the_totals_and_the_run() {
  let (mut desktop, run) = reviewed_run();
  open_conversation(&mut desktop);
  details(&mut desktop, true);
  desktop.wait_until("the reviewer's line", |desktop| {
    desktop
      .tree
      .get_element_by_id("activity-invocation")
      .is_some_and(|element| contains_text(element, "Auto mode reviewer"))
  });
  assert!(shows(&mut desktop, "activity-invocation", "1.1k tokens"));

  desktop.press("nav-usage");
  desktop.wait_until("the reviewer's row", |desktop| {
    desktop
      .tree
      .get_element_by_id("usage-breakdown")
      .is_some_and(|element| contains_text(element, "Auto mode reviewer · claude-haiku-4-5"))
  });
  assert!(
    shows(&mut desktop, "usage-summary-spent", "4.6k"),
    "3,500 of the agent and 1,050 of the reviewer"
  );

  let mut client = agent(&desktop);
  let read = ok(
    &mut client,
    "get_run",
    &json!({"project": "Kontur", "run": run.to_string()}),
  );
  let tokens = &read["tokens"];
  assert_eq!(
    (&tokens["input"], &tokens["cached_input"], &tokens["output"]),
    (&json!(5_200), &json!(1_200), &json!(550)),
    "{tokens}"
  );
  assert_eq!(
    tokens["reviewer"],
    json!({"reviews": 1, "unknown": 0, "input": 1_200, "cached_input": 200, "output": 50})
  );
}
