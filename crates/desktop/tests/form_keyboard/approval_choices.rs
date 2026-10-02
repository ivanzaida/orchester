//! The approval choices of 2026-10-02 in the desktop, from requests and runs seeded into the project
//! database as the runner records them: the dialog's layout (wider answers above a footer of Decide later,
//! Deny and Approve once, which has the focus; what Deny tells the agent), the keyboard (Tab in the visual
//! order, wrapping; Esc is Decide later), one request at a time with "1 of 3", Decide later leaving the
//! requests in Needs you and the status bar with Review, Needs you counting while the dialog is open, a
//! similar rule's Revoke, and auto-approval's switch, indicator and Stop. This harness has no runner, so its status
//! bar shows that problem in place of the waiting requests (`tests/status_bar/approvals.rs` shows them).

use orchester_core::{
  id::RunId,
  run::{PendingToolApproval, RuleScope},
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::{Desktop, expected},
  native_run::native_run,
  task_run::has,
  unreadable_run::at,
};

const DIALOG: &str = "tool-approval-dialog";

/// `count` MCP requests of a seeded run, the oldest first; the run's id.
fn waiting(count: usize) -> (Desktop, RunId) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let mut db = ProjectDb::open(repo).expect("open the project");
    for number in 0..count {
      let approval = PendingToolApproval {
        id: format!("{}:call_{number}", invocation.id),
        run: run.id(),
        execution: invocation.execution,
        invocation: invocation.id,
        agent: "Designer".to_owned(),
        command: false,
        scope: None,
        server: "pencil".to_owned(),
        action: format!("batch_design_{number}"),
        target: None,
        requested_at: at(20 + i64::try_from(number).expect("small")),
        native: None,
      };
      db.request_tool_approval(&approval).expect("pending approval");
    }
    seeded = Some(run.id());
  });
  (desktop, seeded.expect("seeded"))
}

fn open(count: usize) -> (Desktop, RunId) {
  let (mut desktop, run) = waiting(count);
  desktop.wait_for(DIALOG);
  (desktop, run)
}

#[test]
fn approval_choices_dialog_shows_the_wider_answers_and_what_deny_tells() {
  let (mut desktop, _) = open(1);
  for id in [
    "tool-approval-position",
    "tool-approval-similar",
    "tool-approval-always",
    "tool-approval-approve-run",
    "tool-approval-decide-later",
    "tool-approval-deny",
    "tool-approval-approve-once",
  ] {
    assert!(has(&mut desktop, id), "{id} is shown");
  }
  assert!(shows(&mut desktop, "tool-approval-position", "1 of 1 waiting"));
  assert!(shows(
    &mut desktop,
    "tool-approval-similar",
    "pencil batch_design_0, every call until Run 1 ends"
  ));
  assert!(shows(
    &mut desktop,
    "tool-approval-deny-tells",
    "The person reviewing this run denied pencil batch_design_0. Continue without it."
  ));
  // The server is not one of the project's, so Always allow says why it cannot list the tool.
  assert!(shows(&mut desktop, "tool-approval-always", "can no longer be read"));
  assert_eq!(desktop.focused().as_deref(), Some("tool-approval-approve-once"));
  // The wider answers sit between the request and the footer; Decide later is the footer's leftmost.
  let wider = desktop.bounds("tool-approval-wider");
  let footer = desktop.bounds("tool-approval-actions");
  assert!(wider.y + wider.height <= footer.y + 0.5);
  let later = desktop.bounds("tool-approval-decide-later");
  let deny = desktop.bounds("tool-approval-deny");
  let once = desktop.bounds("tool-approval-approve-once");
  assert!(later.x < deny.x && deny.x < once.x);
}

#[test]
fn approval_choices_tab_follows_the_visual_order_and_wraps() {
  let (mut desktop, _) = open(2);
  desktop.tab_to("tool-approval-next", 12);
  assert_eq!(
    desktop.stops(6, false),
    expected(&[
      Some("tool-approval-similar"),
      Some("tool-approval-approve-run"),
      Some("tool-approval-decide-later"),
      Some("tool-approval-deny"),
      Some("tool-approval-approve-once"),
      Some("tool-approval-next"),
    ]),
    "‹ is unavailable on the first request, Always allow on a server the project lacks"
  );
}

#[test]
fn approval_choices_one_request_at_a_time_oldest_first() {
  let (mut desktop, _) = open(3);
  assert!(shows(&mut desktop, "tool-approval-position", "1 of 3 waiting"));
  assert!(shows(&mut desktop, DIALOG, "batch_design_0"));
  desktop.press("tool-approval-next");
  desktop.wait_until("the second request", |desktop| {
    shows(desktop, "tool-approval-position", "2 of 3 waiting")
  });
  assert!(shows(&mut desktop, DIALOG, "batch_design_1"));
  desktop.press("tool-approval-previous");
  desktop.wait_until("the first request again", |desktop| {
    shows(desktop, "tool-approval-position", "1 of 3 waiting")
  });
}

#[test]
fn approval_choices_needs_you_counts_requests_while_the_dialog_is_open() {
  let (mut desktop, _) = open(3);
  assert!(shows(&mut desktop, "status-needs-you", "3"));
}

#[test]
fn approval_choices_decide_later_leaves_the_requests_waiting_with_review() {
  let (mut desktop, _) = open(3);
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  assert!(shows(&mut desktop, "status-needs-you", "3"));
  // Needs you on the Overview lists each request, oldest first.
  assert!(shows(
    &mut desktop,
    "overview-attention",
    "Designer asks to use pencil batch_design_0"
  ));
  // Nothing reopens it by itself.
  desktop.pass();
  assert!(!has(&mut desktop, DIALOG));
  // This harness has no runner, so the status bar shows that first; Needs you's row opens the request.
  let review = first_review(&desktop).expect("a request row with Review");
  desktop.click(&review);
  desktop.wait_for(DIALOG);
  assert!(shows(&mut desktop, "tool-approval-position", "1 of 3 waiting"));
  // Esc is Decide later, never Deny.
  desktop.key("Escape", "Escape");
  desktop.wait_until("Esc closes the dialog", |desktop| !has(desktop, DIALOG));
  let db = ProjectDb::open(desktop.repo()).expect("open");
  assert_eq!(db.pending_tool_approvals_all().expect("requests").len(), 3);
}

#[test]
fn approval_choices_revoke_ends_a_similar_rule() {
  let (mut desktop, run) = waiting(0);
  let rule = ProjectDb::open(desktop.repo())
    .expect("open")
    .add_approval_rule(
      run,
      &RuleScope::SimilarMcpTool {
        server: "pencil".to_owned(),
        tool: "batch_design".to_owned(),
      },
      at(30),
    )
    .expect("rule");
  open_task(&mut desktop);
  let revoke = format!("task-revoke-rule-{}", rule.id.get());
  desktop.wait_for(&revoke);
  assert!(shows(&mut desktop, "task-run-approvals", "pencil · batch_design"));
  // The panel scrolls; a click on the element itself, wherever it is scrolled to.
  desktop.click(&revoke);
  desktop.wait_until("the rule is revoked", |desktop| {
    ProjectDb::open(desktop.repo())
      .expect("open")
      .approval_rules(run)
      .expect("rules")[0]
      .revoked_at
      .is_some()
  });
  desktop.wait_until("Revoke goes away", |desktop| !has(desktop, &revoke));
}

#[test]
fn approval_choices_auto_approval_switch_indicator_and_stop() {
  let (mut desktop, run) = waiting(0);
  open_task(&mut desktop);
  desktop.wait_for("task-auto-approve");
  assert!(!has(&mut desktop, "task-auto-approving"));
  desktop.click("task-auto-approve");
  let chosen = |desktop: &mut Desktop| {
    ProjectDb::open(desktop.repo())
      .expect("open")
      .run_auto_approve(run)
      .expect("read")
  };
  desktop.wait_until("the run auto-approves", |desktop| chosen(desktop) == Some(true));
  desktop.wait_for("task-auto-approving");
  desktop.wait_for("status-auto-approving");
  assert!(shows(&mut desktop, "status-auto-approving", "Auto-approving · Run 1"));
  desktop.press("status-auto-approving-stop");
  desktop.wait_until("the run stops auto-approving", |desktop| chosen(desktop) == Some(false));
  desktop.wait_until("the indicators go away", |desktop| {
    !has(desktop, "status-auto-approving") && !has(desktop, "task-auto-approving")
  });
}

/// The id of the first request's Review on the Overview's Needs you.
fn first_review(desktop: &Desktop) -> Option<String> {
  let db = ProjectDb::open(desktop.repo()).expect("open");
  let oldest = db.pending_tool_approvals_all().expect("requests").into_iter().next()?;
  Some(format!("overview-request-review-{}", oldest.id))
}

fn open_task(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
}
