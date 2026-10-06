//! A request auto mode's reviewer is judging waits without asking the person: no approval dialog, no
//! *Needs you*, only its card in the conversation, which says auto mode is reviewing and still lets the
//! person decide. Once the reviewer asks (or a hard rule held it at once), the dialog opens and *Needs
//! you* counts it. The run's Approvals count it as reviewing, not as waiting. From requests seeded into the
//! project database.
//! `cargo test -p orchester-desktop --test form_keyboard auto_mode_waiting`.

use orchester_core::run::{
  PendingToolApproval,
  review::{Review, ReviewHold},
};
use orchester_store::ProjectDb;

use crate::{
  approval_choices::open_task, auto_mode::claude_connection, github::shows, harness::Desktop,
  narrow_answers::open_conversation, native_run::native_run, task_run::latest_run, unreadable_run::at,
};

const DIALOG: &str = "tool-approval-dialog";

/// What auto mode is doing with the one seeded request.
enum Seen {
  Reviewing,
  Reviewed(Review),
}

/// A run with one waiting MCP request in a project that reviews with Claude Code; the request's id.
fn seeded(seen: &Seen) -> (Desktop, String) {
  let mut id = None;
  let desktop = Desktop::with_seeded_project(|repo| {
    claude_connection(repo);
    let (run, invocation) = native_run(repo);
    let request = PendingToolApproval {
      id: format!("{}:call_1", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Designer".to_owned(),
      command: false,
      scope: None,
      server: "pencil".to_owned(),
      action: "batch_design".to_owned(),
      target: None,
      requested_at: at(20),
      native: None,
    };
    let mut db = ProjectDb::open(repo).expect("open");
    db.request_tool_approval(&request).expect("request");
    match seen {
      Seen::Reviewing => db.set_reviewing(run.id(), &request.id, true).expect("under review"),
      Seen::Reviewed(review) => db.record_review(run.id(), &request.id, review).expect("review"),
    }
    id = Some(request.id);
  });
  (desktop, id.expect("seeded"))
}

#[test]
fn auto_mode_waiting_request_under_review_opens_no_dialog_and_needs_you_does_not_count_it() {
  let (mut desktop, id) = seeded(&Seen::Reviewing);
  desktop.wait_for("status-needs-you");
  for _ in 0..10 {
    desktop.pass();
  }
  assert!(
    desktop.tree.get_element_by_id(DIALOG).is_none(),
    "no dialog while it is reviewed"
  );
  assert!(
    !shows(&mut desktop, "status-needs-you", "1"),
    "Needs you does not count it"
  );
  // Its card says auto mode is reviewing it, and keeps the answers: the person may decide now.
  open_conversation(&mut desktop);
  let line = format!("conversation-approval-reviewing-{id}");
  desktop.wait_for(&line);
  assert!(shows(&mut desktop, &line, "Auto mode is reviewing this request"));
  assert!(shows(
    &mut desktop,
    &format!("conversation-approval-{id}"),
    "Auto mode is reviewing"
  ));
}

#[test]
fn auto_mode_waiting_request_asks_the_person_once_the_reviewer_asks_or_a_hard_rule_holds_it() {
  let asked = Review::Asked {
    reason: "Not needed for the task.".to_owned(),
    model: "claude-haiku-4-5-20251001".to_owned(),
  };
  let held = Review::Held {
    hold: ReviewHold::Secrets,
  };
  for review in [asked, held] {
    let (mut desktop, _) = seeded(&Seen::Reviewed(review));
    desktop.wait_for(DIALOG);
    assert!(shows(&mut desktop, "status-needs-you", "1"));
  }
}

#[test]
fn auto_mode_waiting_request_under_review_counts_as_reviewing_not_waiting_in_approvals() {
  let (mut desktop, _) = seeded(&Seen::Reviewing);
  let repo = desktop.repo().to_path_buf();
  let run = latest_run(&repo).expect("the seeded run");
  let counts = ProjectDb::open(&repo)
    .expect("open")
    .decision_counts(run.id())
    .expect("counts");
  assert_eq!((counts.waiting, counts.reviewing), (0, 1));
  open_task(&mut desktop);
  desktop.wait_for("task-run-approvals");
  assert!(shows(&mut desktop, "task-run-approvals", "Auto mode is reviewing"));

  let asked = Review::Asked {
    reason: "Not needed for the task.".to_owned(),
    model: "claude-haiku-4-5-20251001".to_owned(),
  };
  let (mut desktop, _) = seeded(&Seen::Reviewed(asked));
  desktop.key("Escape", "Escape");
  open_task(&mut desktop);
  desktop.wait_for("task-run-approvals");
  assert!(!shows(&mut desktop, "task-run-approvals", "Auto mode is reviewing"));
}
