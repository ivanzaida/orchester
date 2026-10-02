//! A pipeline graph run end to end, headlessly, on the real runner and `orchester-fake-codex`: the
//! task page lists the progress of parallel steps (02.12 `FjbKY`), and a step that keeps failing
//! after its retries shows the retry limit state (02.B `J3JZaT`) while the run waits for a person,
//! whose decisions — one more retry, the work returned with a note, a stop — the runner carries out.

use orchester_core::{
  resource::{FailureRoute, StepKey},
  run::{AttemptReason, RunFailure, RunStatus, Stuck},
};

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_step, create_and_run, has, init_repository, latest_run, programs, run_finished, seed_steps, set_text,
  },
};

fn retry(to: &str) -> Option<FailureRoute> {
  Some(FailureRoute {
    return_to: StepKey::new(to).expect("key"),
    max_retries: std::num::NonZeroU32::MIN,
  })
}

#[test]
fn task_graph_progress_lists_parallel_steps_with_their_attempt_and_usage() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        agent_step(builder, "implement", "Implement", "Write own file.", &[]),
        agent_step(builder, "checks", "Checks", "Write own file.", &["implement"]),
        agent_step(builder, "review", "Review", "Write own file.", &["implement"]),
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("the finished progress", |desktop| {
    has(desktop, "task-progress-steps")
      && has(desktop, "task-ready-notice")
      && shows(desktop, "task-ready-notice", "Ready for your review")
  });
  for step in ["Implement · Builder", "Checks · Builder", "Review · Builder"] {
    assert!(shows(&mut desktop, "task-progress-steps", step), "{step}");
  }
  assert!(shows(
    &mut desktop,
    "task-progress-steps",
    "Complete · attempt 1 · 9.7k tokens ·"
  ));
  // Complete is a check inside a circle: lucide `circle-check`, the font's `check-circle-2`.
  assert!(shows(&mut desktop, "task-progress-steps", "\u{e226}"));
  assert!(!shows(&mut desktop, "task-progress-steps", "\u{e07c}"));
}

#[test]
fn task_graph_retry_limit_waits_for_a_person_and_offers_the_decisions() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      let mut review = agent_step(builder, "review", "Review", "Report failure.", &["implement"]);
      review.on_failure = retry("implement");
      vec![
        agent_step(builder, "implement", "Implement", "Write own file.", &[]),
        review,
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  let waiting = |attempts: usize| {
    move |desktop: &mut Desktop| {
      latest_run(desktop.repo()).is_some_and(|run| {
        matches!(run.status(), RunStatus::WaitingForPerson(Stuck::RetriesExhausted(_)))
          && run.attempts().len() == attempts
      })
    }
  };
  desktop.wait_until("the run to wait for a person", waiting(2));
  desktop.wait_until("the retry limit state", |desktop| has(desktop, "task-retry-settings"));
  waiting_page_states(&mut desktop);
  assert!(shows(&mut desktop, "task-notice", "Review still fails after 1 retry"));
  assert!(shows(&mut desktop, "task-page", "Automatic retries exhausted"));
  assert!(shows(&mut desktop, "task-retry-limit", "Review · Builder"));
  assert!(shows(&mut desktop, "task-retry-limit", "Reported failed · attempt 2"));
  assert!(shows(&mut desktop, "task-failure-excerpt", "Finished Review"));
  assert!(shows(
    &mut desktop,
    "task-retry-resume",
    "Implement · latest checkpoint"
  ));
  // The work returns by default to the step a retry resumes (02.B4 `K7hbj`).
  assert!(shows(&mut desktop, "task-return-step", "Implement"));
  assert!(shows(
    &mut desktop,
    "task-return-with-note",
    "Return to Implement with note"
  ));
  assert!(!shows(
    &mut desktop,
    "task-retry-settings",
    "not available in this version"
  ));

  desktop.click("task-allow-retry");
  desktop.wait_until("one more attempt to run and wait again", waiting(3));

  desktop.wait_until("the decisions again", |desktop| has(desktop, "task-return-note"));
  set_text(&mut desktop, "task-return-note", "Keep the greeting short.");
  desktop.click("task-return-with-note");
  desktop.wait_until("the returned attempt to wait again", waiting(4));
  let run = latest_run(desktop.repo()).expect("run");
  assert!(matches!(
    run.attempts()[3].reason(),
    AttemptReason::Returned { return_to, note, .. }
      if return_to.as_str() == "implement" && note.as_str() == "Keep the greeting short."
  ));

  shows_the_marks(&mut desktop);
  desktop.wait_until("the decisions again", |desktop| has(desktop, "task-stop-run"));
  desktop.click("task-stop-run");
  desktop.wait_until("the run to fail", |desktop| {
    latest_run(desktop.repo())
      .is_some_and(|run| matches!(run.status(), RunStatus::Failed(RunFailure::RetriesExhausted(_))))
  });
  desktop.wait_until("the evidence without decisions", |desktop| {
    has(desktop, "task-retry-limit") && !has(desktop, "task-retry-settings")
  });
}

/// The transcript marks the automatic return and the person's decisions (02.B7 `zyusO`).
fn shows_the_marks(desktop: &mut Desktop) {
  desktop.click("task-tab-activity");
  desktop.wait_until("the marks", |desktop| {
    has(desktop, "activity-mark-2") && has(desktop, "activity-mark-4")
  });
  assert!(shows(
    desktop,
    "activity-mark-2",
    "Returned to Implement · Review failed (attempt 1)"
  ));
  assert!(shows(desktop, "activity-mark-2", "automatic · retry 1 of 1"));
  assert!(shows(desktop, "activity-mark-3", "by you"));
  assert!(shows(
    desktop,
    "activity-mark-4",
    "Returned to Implement with your note"
  ));
  assert!(shows(desktop, "activity-mark-4", "Keep the greeting short."));
  desktop.click("task-tab-summary");
}

/// The waiting run's top bar, its narrow layout, and how Conversation and Activity read it.
fn waiting_page_states(desktop: &mut Desktop) {
  // Open conversation and Cancel run, no Edit (02.B3 `A3JNZ`).
  assert!(has(desktop, "task-open-conversation"));
  assert!(has(desktop, "task-cancel-run"));
  assert!(!has(desktop, "task-edit"));
  // At 960 the panel goes under the evidence, across the page, at the evidence's inset; a note
  // being written keeps the focus.
  desktop.click("task-return-note");
  assert_eq!(desktop.focused().as_deref(), Some("task-return-note"));
  desktop.tree.resize(960, 960);
  desktop.pass();
  desktop.pass();
  let evidence = desktop.bounds("task-notice");
  let panel = desktop.bounds("task-retry-settings");
  let title = desktop.bounds("task-allow-retry");
  assert!(panel.y > evidence.y + evidence.height, "{panel:?} under {evidence:?}");
  assert!(panel.width > 600.0, "{panel:?}");
  assert!((title.x - evidence.x).abs() < 1.0, "{title:?} starts with {evidence:?}");
  assert_eq!(desktop.focused().as_deref(), Some("task-return-note"));
  // The Activity toolbar keeps the live state beside the step filter, left of the inspector.
  desktop.click("task-tab-activity");
  desktop.wait_until("the activity", |desktop| has(desktop, "activity-live-state"));
  let state = desktop.bounds("activity-live-state");
  let inspector = desktop.bounds("activity-invocation-content");
  assert!(state.x + state.width <= inspector.x, "{state:?} left of {inspector:?}");
  desktop.click("task-tab-summary");
  desktop.wait_until("the decisions", |desktop| has(desktop, "task-retry-settings"));
  desktop.tree.resize(1440, 960);
  desktop.pass();
  // The conversation says the run waits for a decision, not that it stopped at a limit.
  desktop.click("task-tab-conversation");
  desktop.wait_until("the conversation", |desktop| has(desktop, "conversation-status"));
  assert!(shows(desktop, "conversation-status", "Waiting for your decision"));
  assert!(has(desktop, "conversation-open-decision"));
  // Activity has not ended.
  desktop.click("task-tab-activity");
  desktop.wait_until("the activity", |desktop| has(desktop, "activity"));
  assert!(shows(desktop, "activity", "Waits for your decision"));
  assert!(!shows(desktop, "activity", "Run ended"));
  desktop.click("task-tab-summary");
  desktop.wait_until("the decisions", |desktop| has(desktop, "task-retry-settings"));
}
