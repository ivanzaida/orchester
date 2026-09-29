//! The task conversation (02.12 `jgqPe`) end to end, headlessly, on the real runner and
//! `orchester-fake-codex`: an agent's question shows in the conversation, in Needs you and in the
//! status bar, and a reply written in the composer (Enter adds a line, Ctrl+Enter sends) resumes its
//! session; Ask agent on a run ready for review puts the focus in the composer, and the message
//! sent follows the run up until it is ready for review again.

use orchester_core::{
  run::{AttemptReason, RunStatus},
  store::{ConversationStore, TaskStore},
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{create_and_run, finished_on_page, has, init_repository, latest_run, programs, run_finished, seed},
};

fn desktop() -> Desktop {
  let (runner, fake) = programs();
  Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false))
}

fn asked_in_status_bar(desktop: &mut Desktop) -> bool {
  has(desktop, "status-activity") && shows(desktop, "status-activity", "Builder asks · KON-1")
}

/// Ctrl+Enter in the focused field.
fn send_with_keyboard(desktop: &mut Desktop) {
  desktop
    .tree
    .key_down("Enter".into(), "Enter".into(), false, true, false);
  desktop.pass();
}

/// The messages of the project's only task, a person's and the agent's.
fn messages(desktop: &Desktop) -> usize {
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  let task = db.tasks().expect("tasks").remove(0);
  db.messages(task.id()).expect("messages").len()
}

#[test]
fn conversation_layout_matches_pencil_rhythm() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  finished_on_page(&mut desktop);
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-composer");

  // `C3nu0z` is 83 high; `Lr8Dg` is 42 below the Budget row `yT3k7` in 02.S `JOG1i`.
  let composer_height = desktop.bounds("conversation-composer").height;
  assert!((composer_height - 83.0).abs() < 1.0, "composer: {composer_height}px");
  let budget = desktop.bounds("task-conversation-budget");
  let permissions = desktop.bounds("task-permissions");
  assert_eq!(permissions.y - budget.y, 42.0);
}

#[test]
fn conversation_a_question_is_answered_from_the_composer() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Ask first.", "1400000");
  desktop.wait_until("the question", |desktop| {
    has(desktop, "task-notice") && shows(desktop, "task-notice", "Builder asks you a question")
  });
  desktop.wait_until("the status bar", |desktop| {
    shows(desktop, "status-needs-you", "1") && asked_in_status_bar(desktop)
  });
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-reply");
  assert!(shows(&mut desktop, "task-conversation-panel", "Keep email sign-in?"));
  assert!(shows(
    &mut desktop,
    "task-conversation-panel",
    "Question · waiting for your answer"
  ));
  assert!(shows(&mut desktop, "conversation-status", "Waiting for you"));
  assert!(shows(
    &mut desktop,
    "conversation-explanation",
    "Your reply resumes Builder's session."
  ));
  // Nothing to send yet: Send does nothing.
  desktop.click("conversation-send");
  assert_eq!(messages(&desktop), 1, "the question only");

  // Reply puts the focus in the composer; Enter adds a line, Ctrl+Enter sends.
  desktop.click("conversation-reply");
  assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
  desktop.type_text("Yes");
  desktop.enter();
  desktop.type_text("keep it");
  assert_eq!(desktop.value("conversation-input"), "Yes\nkeep it");
  send_with_keyboard(&mut desktop);
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("the acknowledged answer", |desktop| {
    shows(desktop, "task-conversation-panel", "Acknowledged by Builder")
      && shows(desktop, "task-conversation-panel", "Reply · Implement")
      && shows(desktop, "conversation-status", "Ready for review")
  });
  assert!(shows(&mut desktop, "task-conversation-panel", "Question · answered"));
  // The run's readiness replaces the question as the latest event.
  desktop.wait_until("the ready event", |desktop| {
    shows(desktop, "status-activity", "Run #1 ready for review")
  });
  assert_eq!(desktop.value("conversation-input"), "");
  desktop.wait_until("Needs you", |desktop| shows(desktop, "status-needs-you", "0"));
}

#[test]
fn conversation_ask_agent_follows_up_a_run_ready_for_review() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  finished_on_page(&mut desktop);
  desktop.click("task-ask-agent");
  desktop.wait_for("task-conversation-panel");
  assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
  assert!(shows(&mut desktop, "conversation-status", "Ready for review"));
  assert!(shows(
    &mut desktop,
    "conversation-explanation",
    "Asking continues Builder's session in the run's worktree."
  ));
  desktop.type_text("Add a second line.");
  desktop.click("conversation-send");
  desktop.wait_until("the follow-up to finish", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| {
      run.status() == &RunStatus::Succeeded
        && matches!(
          run.current_attempt().map(|attempt| attempt.reason()),
          Some(AttemptReason::FollowUp { .. })
        )
    })
  });
  desktop.wait_until("the reply", |desktop| {
    shows(desktop, "task-conversation-panel", "Acknowledged by Builder")
      && shows(desktop, "task-conversation-panel", "Reply · Implement")
      && shows(desktop, "conversation-status", "Ready for review")
  });
  // Open conversation on the Summary goes back to it.
  desktop.click("task-tab-summary");
  finished_on_page(&mut desktop);
  desktop.click("task-open-conversation");
  desktop.wait_for("task-conversation-panel");
}

#[test]
fn conversation_failed_run_continues_from_the_composer_without_run_again() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Report failure.", "1400000");
  desktop.wait_until("failed run on the page", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| matches!(run.status(), RunStatus::Failed(_)))
      && shows(desktop, "task-notice", "The run failed at Implement")
  });
  let failed = latest_run(desktop.repo()).expect("failed run");
  assert!(matches!(failed.status(), RunStatus::Failed(_)));
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-composer");
  assert!(shows(&mut desktop, "conversation-status", "Failed"));
  assert!(shows(&mut desktop, "conversation-send", "Send & continue"));
  desktop.click("conversation-input");
  desktop.type_text("Continue with the corrected instruction.");
  send_with_keyboard(&mut desktop);
  desktop.wait_until("recovered run to succeed", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Succeeded && run.attempts().len() == 2)
  });
  let done = latest_run(desktop.repo()).expect("recovered");
  assert_eq!(done.id(), failed.id());
  assert_eq!(done.branch(), failed.branch());
  desktop.wait_until("delivered instructions", |desktop| {
    shows(desktop, "task-conversation-panel", "Acknowledged by Builder")
  });
}
