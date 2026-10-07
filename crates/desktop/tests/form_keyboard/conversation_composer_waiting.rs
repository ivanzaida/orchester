//! The chat always answers (ORC-79), end to end, headlessly, on the real runner and the fake CLIs: in
//! every state where a run waits for the person, the composer says before sending who reads the
//! message and when, and its action lets the run go on with it. A run waiting for a decision after
//! its retries ran out continues the step it stopped at with the message as instructions; a run
//! stopped at its limit keeps the message and opens the Summary to raise the limit; a paused run
//! resumes with the message, or keeps it until it resumes; a cancelled run runs again with it.

use orchester_core::{
  conversation::Delivery,
  resource::{FailureRoute, StepKey},
  run::{AttemptReason, RunStatus, Stuck},
};

use crate::{
  conversation_composer::{deliveries, holds, placeholder, release, send_with_keyboard, tip_says, write},
  github::shows,
  harness::Desktop,
  task_run::{
    agent_started, agent_step, create_and_run, has, init_repository, latest_run, programs, run_finished, seed,
    seed_claude, seed_steps,
  },
};

/// Whether every message the person sent was read by an agent.
fn all_read(desktop: &mut Desktop) -> bool {
  let stored = deliveries(desktop);
  !stored.is_empty()
    && stored
      .iter()
      .all(|delivery| matches!(delivery, Some(Delivery::Acknowledged { .. })))
}

#[test]
fn composer_continues_a_run_waiting_for_a_decision_from_the_step_it_stopped_at() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      let mut review = agent_step(builder, "review", "Review", "Report failure.", &["implement"]);
      review.on_failure = Some(FailureRoute {
        return_to: StepKey::new("implement").expect("key"),
        max_retries: std::num::NonZeroU32::MIN,
      });
      vec![
        agent_step(builder, "implement", "Implement", "Append one line.", &[]),
        review,
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the run to wait for a decision", |desktop| {
    latest_run(desktop.repo())
      .is_some_and(|run| matches!(run.status(), RunStatus::WaitingForPerson(Stuck::RetriesExhausted(_))))
  });
  desktop.click("task-tab-conversation");
  // Before sending, the composer says the message continues the step the run stopped at; the
  // decisions stay a click away.
  desktop.wait_until("the composer that continues the run", |desktop| {
    holds(
      desktop,
      "conversation-composer-context",
      "Continue 02 Review with your instructions · attempt 3",
    )
  });
  assert!(has(&mut desktop, "conversation-open-decision"));
  assert!(shows(&mut desktop, "conversation-recipient", "continues the run"));
  assert_eq!(placeholder(&mut desktop), "Tell Builder how to continue the run…");
  assert!(tip_says(&mut desktop, "Continue with instructions · Enter."));
  assert!(holds(
    &mut desktop,
    "conversation-explanation",
    "Your message continues this run from its saved work."
  ));
  write(&mut desktop, "So what do you suggest?");
  desktop.click("conversation-send");
  desktop.wait_until("the continued run to finish", |desktop| run_finished(desktop.repo()));
  let run = latest_run(desktop.repo()).expect("the run");
  assert_eq!(run.attempts().len(), 3);
  assert!(matches!(
    run.current_attempt().map(|attempt| attempt.reason()),
    Some(AttemptReason::Recovery { step }) if step.as_str() == "review"
  ));
  desktop.wait_until("the message read by Review's agent", all_read);
}

#[test]
fn composer_keeps_the_message_of_a_run_stopped_at_its_limit_and_opens_the_raise() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed_claude(repo, &fake));
  create_and_run(&mut desktop, "Work in turns.", "5000");
  desktop.wait_until("the run stopped at its limit", |desktop| has(desktop, "raise-limit"));
  desktop.click("task-tab-conversation");
  // What limit was reached, and what the action does, before sending.
  desktop.wait_until("the composer of the stopped run", |desktop| {
    holds(
      desktop,
      "conversation-composer-context",
      "Stopped at the token limit · used 9.7k of 5k tokens",
    )
  });
  assert!(shows(&mut desktop, "conversation-recipient", "Builder"));
  assert!(shows(&mut desktop, "conversation-recipient", "stopped at limit"));
  assert_eq!(placeholder(&mut desktop), "Message Builder…");
  assert!(tip_says(&mut desktop, "Raise the limit and continue · Enter."));
  assert!(has(&mut desktop, "conversation-send-icon-queues"));
  write(&mut desktop, "Then add a test.");
  desktop.click("conversation-send");
  // Stored, the message waits for the run, and the Summary opens on the raise.
  desktop.wait_until("the raise on the Summary", |desktop| {
    has(desktop, "raise-limit") && !has(desktop, "conversation-composer")
  });
  assert_eq!(deliveries(&desktop), [Some(Delivery::Queued)]);
  assert_eq!(desktop.focused().as_deref(), Some("raise-limit-tokens"));
  desktop.click("raise-limit-continue");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("the message read once the run continued", all_read);
}

#[test]
fn composer_resumes_a_paused_run_with_the_message_or_keeps_it_for_the_resume() {
  // Two steps: the pause lands at the checkpoint between them.
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, true));
  create_and_run(&mut desktop, "Work slowly.", "1400000");
  desktop.wait_until("Pause run while the agent works", |desktop| {
    has(desktop, "task-pause-run") && agent_started(desktop.repo())
  });
  desktop.click("task-pause-run");
  desktop.wait_until("Resume run", |desktop| has(desktop, "task-resume-run"));
  desktop.click("task-tab-conversation");
  desktop.wait_until("the composer of the paused run", |desktop| {
    holds(
      desktop,
      "conversation-explanation",
      "Resume and send resumes the run, and its agent reads your message before its next action.",
    )
  });
  assert!(tip_says(&mut desktop, "Resume and send · Enter."));
  assert!(has(&mut desktop, "conversation-send-icon-now"));
  assert!(shows(&mut desktop, "conversation-recipient", "paused"));
  assert_eq!(placeholder(&mut desktop), "Message Builder…");
  desktop.click("conversation-send-options");
  desktop.wait_for("conversation-send-menu");
  assert!(holds(&mut desktop, "conversation-send-now", "Resume and send"));
  assert!(holds(
    &mut desktop,
    "conversation-send-after-turn",
    "Send without resuming"
  ));
  assert!(shows(&mut desktop, "conversation-send-after-turn", "Ctrl+Enter"));
  desktop.click("conversation-send-options");

  // Ctrl+Enter keeps the message for the resume; the run stays paused.
  write(&mut desktop, "Check the OAuth dialog.");
  send_with_keyboard(&mut desktop);
  desktop.wait_until("queued for the resume", |desktop| {
    holds(
      desktop,
      "conversation",
      "Queued · Builder reads it before its next action on resume",
    )
  });
  assert!(matches!(
    latest_run(desktop.repo()).map(|run| run.status().clone()),
    Some(RunStatus::Paused(_))
  ));

  // Resume and send: the run goes on, and its agent reads both.
  write(&mut desktop, "Also cover the logout.");
  desktop.click("conversation-send");
  desktop.wait_until("the resumed run to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("both messages read", all_read);
  assert_eq!(deliveries(&desktop).len(), 2);
}

#[test]
fn composer_runs_a_cancelled_task_again_with_the_message() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Wait for release.", "1400000");
  desktop.wait_until("Cancel run while the agent works", |desktop| {
    has(desktop, "task-cancel-run") && agent_started(desktop.repo())
  });
  desktop.click("task-cancel-run");
  desktop.wait_until("the run to be cancelled", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Cancelled)
  });
  desktop.click("task-tab-conversation");
  // No agent of the run reads it, and the composer says so before sending.
  desktop.wait_until("the composer of the ended run", |desktop| {
    holds(
      desktop,
      "conversation-composer-context",
      "No agent of this run reads messages any more",
    )
  });
  assert!(tip_says(&mut desktop, "Run again with this message · Enter."));
  assert_eq!(placeholder(&mut desktop), "Write to the first agent of the next run…");
  assert!(holds(
    &mut desktop,
    "conversation-explanation",
    "Its first agent reads your message."
  ));
  write(&mut desktop, "Start over and keep it short.");
  desktop.click("conversation-send");
  desktop.wait_until("run #2", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.number().get() == 2)
  });
  release(&desktop);
  desktop.wait_until("run #2 to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("the message read by run #2's agent", all_read);
}
