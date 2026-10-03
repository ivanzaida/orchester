//! A request whose agent can no longer receive an answer, headlessly with the real runner: the step's
//! agent (`orchester-fake-claude`) asks twice, and a request recorded for the same invocation that the
//! agent does not wait on is answered. The runner ends it and refuses the answer; the dialog drops it
//! at once for the agent's own request, says the previous one ended, and Needs you counts one less. The
//! step's progress row says what its agent waits on, from the real request.

use lurq::layout::quad::QuadContent;
use orchester_core::{connection::AgentProvider, run::PendingToolApproval};
use orchester_desktop::{activity::Target, messages::Message};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, fake_claude, has, init_repository, programs, seed, seed_agent},
};

const DIALOG: &str = "tool-approval-dialog";

/// The waiting requests of the project, oldest first.
fn waiting(desktop: &Desktop) -> Vec<PendingToolApproval> {
  ProjectDb::open(desktop.repo())
    .expect("open")
    .pending_tool_approvals_all()
    .expect("requests")
}

#[test]
fn ended_approval_leaves_the_dialog_for_the_next_and_needs_you_counts_one_less() {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(
        builder,
        "implement",
        "Implement",
        "Run `cargo add serde` twice.",
        &[],
      )]
    });
  });
  create_and_run(&mut desktop, "Add serde.", "1400000");
  desktop.wait_for(DIALOG);
  // The step's row says what its agent waits on, though a tool request leaves the step working.
  desktop.wait_until("the step row waits on the request", |desktop| {
    has(desktop, "task-progress-steps")
      && shows(
        desktop,
        "task-progress-steps",
        "Waiting for you · cargo add serde · since",
      )
  });
  let first = waiting(&desktop).remove(0);
  // Recorded for the agent's invocation, but not the call it waits on: no agent can take its answer.
  let unheld = PendingToolApproval {
    id: format!("{}:unheld", first.invocation),
    action: "pnpm unheld-check".to_owned(),
    ..first.clone()
  };
  ProjectDb::open(desktop.repo())
    .expect("open")
    .request_tool_approval(&unheld)
    .expect("recorded");
  // Approving the agent's first call reads the requests again; the agent then asks a second time.
  desktop.press("tool-approval-approve-once");
  desktop.wait_until("the unheld request and the agent's second", |desktop| {
    has(desktop, DIALOG) && shows(desktop, "tool-approval-position", "1 of 2 waiting")
  });
  assert!(shows(&mut desktop, DIALOG, "pnpm unheld-check"));
  assert!(shows(&mut desktop, "status-needs-you", "2"));

  desktop.press("tool-approval-approve-once");
  desktop.wait_until("the dialog moves to the agent's request", |desktop| {
    has(desktop, DIALOG) && shows(desktop, "tool-approval-position", "1 of 1 waiting")
  });
  // The fake's second call adds another package.
  assert!(shows(&mut desktop, DIALOG, "cargo add tokio"));
  assert!(!shows(&mut desktop, DIALOG, "unheld-check"));
  assert!(shows(
    &mut desktop,
    "tool-approval-actions",
    "The previous request ended without an answer"
  ));
  desktop.wait_until("Needs you counts the agent's request only", |desktop| {
    shows(desktop, "status-needs-you", "1")
  });
  let left = waiting(&desktop);
  assert_eq!(left.len(), 1);
  assert_ne!(left[0].id, unheld.id);
  let decided = ProjectDb::open(desktop.repo())
    .expect("open")
    .decided_tool_approvals(first.run)
    .expect("decided");
  let ended = decided
    .iter()
    .find(|decided| decided.request.id == unheld.id)
    .expect("recorded");
  assert_eq!(ended.decision, None, "it ended without an answer");

  // Esc closes the dialog whatever happened before.
  desktop.key("Escape", "Escape");
  desktop.wait_until("Esc closes the dialog", |desktop| !has(desktop, DIALOG));
}

/// *Request ended* in the status bar keeps its words beside *Auto-approving* and Stop at 960, on a run
/// that auto-approves while its agent works.
#[test]
fn ended_approval_status_line_keeps_its_words_beside_auto_approving_at_960() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_for("task-auto-approve");
  desktop.click("task-auto-approve");
  desktop.wait_for("status-auto-approving");
  desktop.activity.record(
    Some("Kontur".into()),
    Message::new("statusBar.approvals.ended"),
    Target::Nothing,
  );
  // The smallest window the app opens, and narrower, as a longer MCP address leaves less room: which
  // run auto-approves gives way, then the time; the activity keeps its words.
  for width in [960, 930] {
    desktop.tree.resize(width, 700);
    desktop.wait_until("the ended line", |desktop| {
      has(desktop, "status-activity") && shows(desktop, "status-activity", "Request ended")
    });
    assert_keeps_its_words(&mut desktop);
  }
}

fn assert_keeps_its_words(desktop: &mut Desktop) {
  let agents = desktop.bounds("status-agents");
  let stop = desktop.bounds("status-auto-approving-stop");
  assert!(
    stop.x + stop.width <= agents.x + 0.5,
    "Stop stays whole before the right group"
  );
  let layout = desktop.tree.last_layout().expect("laid out").clone();
  let drawn = |text: &str| {
    desktop
      .tree
      .resolve_quads(&layout)
      .into_iter()
      .find_map(|quad| match quad.content {
        QuadContent::Text { text: drawn, .. } if drawn == text => Some(quad.x + quad.width),
        _ => None,
      })
  };
  let ended = drawn("Request ended").expect("the ended line keeps its words");
  assert!(
    ended <= agents.x,
    "the ended line ends at {ended}, past the right group at {}",
    agents.x
  );
  assert!(drawn("Auto-approving").is_some(), "Auto-approving keeps its word");
  assert!(drawn("Stop").is_some(), "Stop keeps its word");
}
