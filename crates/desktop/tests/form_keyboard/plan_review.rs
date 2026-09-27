//! Plan review (02.11 `x3lVC`) end to end, headlessly, on the real runner and
//! `orchester-fake-codex`: a gated Plan step waits with plan v1, the task page reviews it, the
//! Overview's Needs you panel and the status bar count it and the status bar's latest event names
//! it; approving continues the run, requesting changes with a note brings plan v2. Windows only,
//! like the runner's named pipe.
#![cfg(windows)]

use orchester_core::{resource::Step, run::RunStatus};

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_step, create_and_run, has, init_repository, latest_run, programs, run_finished, seed_steps, set_text,
  },
};

/// Plan (gated), then Implement.
fn gated_desktop() -> Desktop {
  let (runner, fake) = programs();
  Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        Step {
          approval_gate: true,
          ..agent_step(builder, "plan", "Plan", "Propose a plan.", &[])
        },
        agent_step(builder, "implement", "Implement", "Follow the plan.", &["plan"]),
      ]
    });
  })
}

/// Creates and runs the task and waits until plan `version` is under review.
fn reviewing(desktop: &mut Desktop, version: &str) {
  let title = format!("Plan v{version} · Say hello");
  desktop.wait_until("the plan review", |desktop| {
    has(desktop, "plan-review-title") && shows(desktop, "plan-review-title", &title)
  });
}

fn waiting_run(desktop: &Desktop) -> bool {
  latest_run(desktop.repo()).is_some_and(|run| run.waits_for_a_person())
}

#[test]
fn plan_review_shows_the_plan_and_approving_continues_the_run() {
  let mut desktop = gated_desktop();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  reviewing(&mut desktop, "1");
  assert!(shows(&mut desktop, "plan-review-notice", "Your approval is needed"));
  assert!(shows(
    &mut desktop,
    "plan-review-notice",
    "Review plan v1 from Builder. Builder has not started."
  ));
  // The agent's result, rendered from Markdown.
  assert!(shows(&mut desktop, "plan-review-text", "Finished Plan"));
  assert!(shows(&mut desktop, "plan-review-aside", "README.md ends with hello"));
  assert!(shows(&mut desktop, "plan-review-aside", "v1 · awaiting approval"));
  assert!(shows(
    &mut desktop,
    "task-page",
    "Run 1 · Attempt 1 · Delivery v1 · Builder finished"
  ));
  // Nothing works while the run waits: Cancel stays, Pause does not.
  assert!(has(&mut desktop, "task-cancel-run"));
  assert!(!has(&mut desktop, "task-pause-run"));
  // The status bar counts the run and its latest event names the plan.
  desktop.wait_until("the status bar", |desktop| {
    shows(desktop, "status-needs-you", "1") && shows(desktop, "status-activity", "Plan ready for review · KON-1")
  });
  assert!(waiting_run(&desktop));

  desktop.click("plan-approve");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("the review to go", |desktop| {
    !has(desktop, "plan-review") && shows(desktop, "status-needs-you", "0")
  });
}

#[test]
fn plan_review_requests_changes_with_a_note_and_reviews_the_revision() {
  let mut desktop = gated_desktop();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  reviewing(&mut desktop, "1");
  // Requesting changes needs a note: until there is one, the button does nothing.
  desktop.click("plan-request-changes");
  assert!(waiting_run(&desktop));
  assert!(shows(&mut desktop, "plan-review-title", "Plan v1"));
  set_text(&mut desktop, "plan-review-note", "Keep email sign-in.");
  desktop.click("plan-request-changes");
  reviewing(&mut desktop, "2");
  // Both versions can be read; v2 waits.
  assert!(has(&mut desktop, "plan-review-version"));
  assert!(shows(
    &mut desktop,
    "plan-review-notice",
    "Review plan v2 from Builder."
  ));
  // The note of v1 went to the agent's resumed session; the revision is its reply.
  assert!(shows(&mut desktop, "plan-review-text", "Done."));
  assert_eq!(desktop.value("plan-review-note"), "");

  desktop.click("plan-approve");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  let run = latest_run(desktop.repo()).expect("run");
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[test]
fn plan_review_waits_in_the_overview_needs_you_panel() {
  let mut desktop = gated_desktop();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  reviewing(&mut desktop, "1");
  desktop.click("nav-overview");
  desktop.wait_until("the review card", |desktop| has(desktop, "overview-decision-KON-1"));
  assert!(shows(&mut desktop, "overview-decision-KON-1", "REVIEW PLAN"));
  assert!(shows(&mut desktop, "overview-decision-KON-1", "Say hello"));
  assert!(shows(
    &mut desktop,
    "overview-decision-KON-1",
    "Plan v1 from Builder waits for your decision."
  ));
  assert!(shows(&mut desktop, "overview-attention", "1"));
  desktop.click("overview-review-plan-KON-1");
  reviewing(&mut desktop, "1");
  // The status bar's latest event opens the task too.
  desktop.click("nav-overview");
  desktop.wait_until("the overview", |desktop| has(desktop, "overview-decision-KON-1"));
  desktop.click("status-activity");
  reviewing(&mut desktop, "1");
}
