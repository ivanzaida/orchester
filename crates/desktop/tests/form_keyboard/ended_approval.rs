//! A request whose agent can no longer receive an answer, headlessly with the real runner: the step's
//! agent (`orchester-fake-claude`) asks twice, and a request recorded for the same invocation that the
//! agent does not wait on is answered. The runner ends it and refuses the answer; the dialog drops it
//! at once for the agent's own request, says the previous one ended, and Needs you counts one less. The
//! step's progress row says what its agent waits on, from the real request.

use lurq::layout::quad::QuadContent;
use orchester_core::{connection::AgentProvider, run::PendingToolApproval, store::ActivityStore};
use orchester_desktop::{activity::Target, messages::Message};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_started, agent_step, create_and_run, fake_claude, has, init_repository, latest_run, programs, seed,
    seed_agent,
  },
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

/// A run of the Builder that works until it is ended and auto-approves, with the task page open.
fn auto_approving_run() -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("the agent works", |desktop| agent_started(desktop.repo()));
  desktop.wait_for("task-auto-approve");
  desktop
}

/// *Request ended* in the status bar keeps its words beside *Auto-approving* and Stop at 960, on a run
/// that auto-approves while its agent works.
#[test]
fn ended_approval_status_line_keeps_its_words_beside_auto_approving_at_960() {
  let mut desktop = auto_approving_run();
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
    assert_whole(&mut desktop, &["Request ended", "Auto-approving", "Stop"]);
  }
}

/// A request waiting beside *Auto-approving* at 960: what it asks gives way, how many wait (briefly),
/// Review, *Auto-approving* and Stop stay whole.
#[test]
fn ended_approval_waiting_count_and_review_stay_beside_auto_approving_at_960() {
  let mut desktop = auto_approving_run();
  let run = latest_run(desktop.repo()).expect("the run");
  let mut db = ProjectDb::open(desktop.repo()).expect("open");
  let invocation = db.invocations(run.id()).expect("invocations").remove(0);
  let request = PendingToolApproval {
    id: format!("{}:waiting", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: false,
    scope: None,
    server: "pencil".to_owned(),
    action: "batch_design".to_owned(),
    target: None,
    requested_at: invocation.started_at,
    native: None,
  };
  db.request_tool_approval(&request).expect("recorded");
  // Turning auto-approval on reads the requests again; the dialog then opens on this one.
  desktop.click("task-auto-approve");
  desktop.wait_for(DIALOG);
  desktop.key("Escape", "Escape");
  desktop.wait_until("the status bar names the request", |desktop| {
    !has(desktop, DIALOG) && has(desktop, "status-auto-approving") && has(desktop, "status-approvals-review")
  });
  // Each part shows whole or not at all: below their widths (`theme::ApprovalsWidths`) the count reads
  // "1 waiting", and what the request asks, its time and the run's name are left out, never cut.
  for width in [960, 1130, 1200] {
    desktop.tree.resize(width, 700);
    desktop.pass();
    assert_whole(&mut desktop, &["1 waiting", "Review", "Auto-approving", "Stop"]);
    assert_nothing_cut(&mut desktop);
  }
}

/// No text of the status bar is cut to an ellipsis.
fn assert_nothing_cut(desktop: &mut Desktop) {
  let agents = desktop.bounds("status-agents");
  let layout = desktop.tree.last_layout().expect("laid out").clone();
  for quad in desktop.tree.resolve_quads(&layout) {
    if let QuadContent::Text { text, .. } = &quad.content {
      assert!(
        quad.y < agents.y - 4.0 || !text.ends_with('…'),
        "{text:?} is cut in the status bar"
      );
    }
  }
}

/// Each of `texts` is drawn whole in the status bar, before its right group, and Stop is whole there.
fn assert_whole(desktop: &mut Desktop, texts: &[&str]) {
  let agents = desktop.bounds("status-agents");
  let stop = desktop.bounds("status-auto-approving-stop");
  assert!(
    stop.x + stop.width <= agents.x + 0.5,
    "Stop stays whole before the right group"
  );
  let layout = desktop.tree.last_layout().expect("laid out").clone();
  let quads = desktop.tree.resolve_quads(&layout);
  // The task page has a Stop of its own; only what the status bar draws counts.
  for text in texts {
    let end = quads
      .iter()
      .find_map(|quad| match &quad.content {
        QuadContent::Text { text: drawn, .. } if drawn == text && quad.y >= agents.y - 4.0 => Some(quad.x + quad.width),
        _ => None,
      })
      .unwrap_or_else(|| panic!("{text} is drawn whole"));
    assert!(
      end <= agents.x,
      "{text} ends at {end}, past the right group at {}",
      agents.x
    );
  }
}
