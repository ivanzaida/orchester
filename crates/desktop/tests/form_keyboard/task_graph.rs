//! A pipeline graph run end to end, headlessly, on the real runner and `orchester-fake-codex`: the
//! task page lists the progress of parallel steps (02.12 `FjbKY`), and a step that keeps failing
//! after its retries shows the retry limit state (02.B `J3JZaT`) with the actions this version
//! cannot carry out yet unavailable. Windows only, like the runner's named pipe.
#![cfg(windows)]

use orchester_core::{
  resource::{FailureRoute, StepKey},
  run::{RunFailure, RunStatus},
};

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_step, create_and_run, has, init_repository, latest_run, programs, run_finished, seed_steps, wait_long,
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
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
  wait_long(&mut desktop, "the finished progress", |desktop| {
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
fn task_graph_retry_limit_shows_the_failing_step_and_unavailable_actions() {
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
  wait_long(&mut desktop, "the run to fail", |desktop| {
    latest_run(desktop.repo())
      .is_some_and(|run| matches!(run.status(), RunStatus::Failed(RunFailure::RetriesExhausted(_))))
  });
  wait_long(&mut desktop, "the retry limit state", |desktop| {
    has(desktop, "task-retry-limit")
  });
  assert!(shows(&mut desktop, "task-notice", "Review still fails after 1 retry"));
  assert!(shows(&mut desktop, "task-page", "Automatic retries exhausted"));
  assert!(shows(&mut desktop, "task-retry-limit", "Review · Builder"));
  assert!(shows(&mut desktop, "task-retry-limit", "Reported failed · attempt 2"));
  assert!(shows(&mut desktop, "task-failure-excerpt", "Finished Review"));
  assert!(shows(
    &mut desktop,
    "task-retry-settings",
    "is not available in this version yet"
  ));
  // Allowing another attempt is not possible yet: pressing it changes nothing.
  desktop.click("task-allow-retry");
  let run = latest_run(desktop.repo()).expect("run");
  assert_eq!(run.attempts().len(), 2);
  assert!(has(&mut desktop, "task-retry-limit"));
}
