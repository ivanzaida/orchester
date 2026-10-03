//! A request whose agent can no longer receive an answer, headlessly with the real runner: the step's
//! agent (`orchester-fake-claude`) asks twice, and a request recorded for the same invocation that the
//! agent does not wait on is answered. The runner ends it and refuses the answer; the dialog drops it
//! at once for the agent's own request, says the previous one ended, and Needs you counts one less. The
//! step's progress row says what its agent waits on, from the real request.

use lurq::layout::quad::QuadContent;
use orchester_core::{connection::AgentProvider, run::PendingToolApproval, store::ActivityStore};
use orchester_desktop::{activity::Target, messages::Message, theme::APPROVALS_ASKS_MIN};
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
/// The padding of the part that holds the time (6 before it, 8 after), which stays when the time drops.
const TIME_PART_PADDING: f32 = 20.0;

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

/// A request waiting beside *Auto-approving*, from 930 to 1600, and without it at 960: the parts give
/// way in their order and never to a stub.
#[test]
fn ended_approval_waiting_parts_give_way_in_order() {
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
  // As the window narrows, what the request asks trims its end down to its minimum, then the run's
  // name, the time and the words after the count drop out whole; the count, Review, Auto-approving and
  // Stop always stay, and nothing is cut to a stub.
  for width in [930, 960, 1130, 1200, 1300, 1440, 1600] {
    desktop.tree.resize(width, 700);
    desktop.pass();
    assert_whole(&mut desktop, &["1", "Review", "Auto-approving", "Stop"]);
    assert_no_stub(&mut desktop);
  }
  // With more room, more of it shows.
  desktop.tree.resize(1300, 700);
  desktop.pass();
  assert_whole(&mut desktop, &[" approval waiting"]);
  assert!(time_shown(&mut desktop), "the time shows at 1300");
  desktop.tree.resize(1600, 700);
  desktop.pass();
  assert_whole(&mut desktop, &[" approval waiting", " · Run 1 · KON-1"]);
  assert!(time_shown(&mut desktop), "the time shows at 1600");
  // Without auto-approval, at 960 the words after the count show; what the request asks shows with at
  // least its minimum, or has dropped out before the count's words would (the time goes before them).
  desktop.click("task-auto-approve");
  desktop.wait_until("auto-approval is off", |desktop| !has(desktop, "status-auto-approving"));
  desktop.tree.resize(960, 700);
  desktop.pass();
  assert_whole(&mut desktop, &["1", " approval waiting", "Review"]);
  assert_no_stub(&mut desktop);
  let asks = desktop.bounds("status-approvals-asks");
  let review = desktop.bounds("status-approvals-review");
  assert!(
    asks.x + asks.width <= review.x + 0.5,
    "what it asks stays before Review"
  );
}

/// Whether the waiting request's time is drawn: its part is wider than its padding alone.
fn time_shown(desktop: &mut Desktop) -> bool {
  desktop.bounds("status-approvals-time").width > TIME_PART_PADDING
}

/// No text of the status bar is cut to a stub: only what the request asks may trim its end, never below
/// its minimum; below it, it drops out.
fn assert_no_stub(desktop: &mut Desktop) {
  let agents = desktop.bounds("status-agents");
  let asks = desktop.bounds("status-approvals-asks");
  assert!(
    asks.width == 0.0 || asks.width >= APPROVALS_ASKS_MIN,
    "what it asks has {} px",
    asks.width
  );
  let layout = desktop.tree.last_layout().expect("laid out").clone();
  for quad in desktop.tree.resolve_quads(&layout) {
    if let QuadContent::Text { text, .. } = &quad.content {
      let in_bar = quad.y >= agents.y - 4.0;
      assert!(
        !in_bar || !text.ends_with('…') || text.starts_with(" · "),
        "{text:?} is cut in the status bar"
      );
    }
  }
}

/// Each of `texts` is drawn whole in the status bar, before its right group.
fn assert_whole(desktop: &mut Desktop, texts: &[&str]) {
  let agents = desktop.bounds("status-agents");
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
