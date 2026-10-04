//! The composer's box (`YJfPo`) end to end, headlessly, on the real runner and
//! `orchester-fake-codex`: the send button's icon says what happens to the message (arrow-up: the
//! agent gets it now; clock: it queues), *Attach* is unavailable with its tooltip, the run's
//! permissions chip opens their popover, and at 960 the box keeps its controls in the chat, the chip
//! shortened to its shield.

use crate::{
  conversation_composer::{release, working_conversation},
  github::shows,
  harness::Desktop,
  task_run::{
    agent_started, create_and_run, fill_new_task, finished_on_page, has, init_repository, programs, run_finished, seed,
  },
};

fn desktop() -> Desktop {
  let (runner, fake) = programs();
  Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false))
}

#[test]
fn composer_send_icon_says_whether_the_message_queues() {
  // A draft task: the message waits for the run's start.
  let mut desktop = desktop();
  fill_new_task(&mut desktop, "Say hello.", "1400000");
  desktop.click("save-draft");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-tab-conversation");
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-composer");
  assert!(has(&mut desktop, "conversation-send-icon-queues"));
  assert!(!has(&mut desktop, "conversation-send-options"), "one way to send");
}

#[test]
fn composer_send_icon_sends_now_to_the_agent_at_work_and_its_chip_opens_the_permissions() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Wait for release.", "1400000");
  desktop.wait_until("the agent at work", |desktop| agent_started(desktop.repo()));
  desktop.click("task-tab-conversation");
  desktop.wait_until("the composer for the agent at work", |desktop| {
    has(desktop, "conversation-send-options")
  });
  assert!(has(&mut desktop, "conversation-send-icon-now"));
  // Attach is not built: pointed at, it says so.
  desktop.hover("conversation-attach");
  desktop.wait_for("conversation-attach-tip");
  assert!(shows(
    &mut desktop,
    "conversation-attach-tip",
    "Attaching files is not available yet."
  ));
  // The chip names what the step's agent may do in this run and opens the run's permissions.
  assert!(has(&mut desktop, "conversation-permissions"));
  desktop.click("conversation-permissions");
  desktop.wait_for("task-permissions-popover");
  release(&desktop);
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}

#[test]
fn composer_of_a_run_ready_for_review_sends_the_follow_up_at_once() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  finished_on_page(&mut desktop);
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-composer");
  assert!(has(&mut desktop, "conversation-send-icon-now"));
  assert!(shows(&mut desktop, "conversation-recipient", "resumes its session"));
  assert!(shows(
    &mut desktop,
    "conversation-explanation",
    "Asking continues Builder's session in the run's worktree."
  ));
}

/// Whether `id` lies within `right`, the main column's right edge less its 32 padding, from side to
/// side.
fn fits(desktop: &mut Desktop, id: &str, right: f32) -> bool {
  let bounds = desktop.bounds(id);
  bounds.width > 0.0 && bounds.x >= 0.0 && bounds.x + bounds.width <= right + 0.5
}

#[test]
fn composer_keeps_its_buttons_and_the_search_usable_at_960() {
  let mut desktop = working_conversation("Wait for release.");
  desktop.wait_until("the timeline's toolbar", |desktop| {
    has(desktop, "conversation-live-state")
  });
  assert!(shows(&mut desktop, "conversation-live-state", "Following live"));
  desktop.tree.resize(960, 960);
  desktop.pass();
  desktop.pass();
  // The rail is its strip at 960: the chat ends 56 from the window's edge.
  for id in [
    "conversation-attach",
    "conversation-permissions",
    "conversation-recipient",
    "conversation-send-options",
    "conversation-send",
  ] {
    assert!(fits(&mut desktop, id, 872.0), "{id} at 960");
  }
  // The permissions chip shortens to its shield (`vOF1a`); the recipient keeps its name.
  assert!(desktop.bounds("conversation-permissions").width <= 40.0);
  assert!(shows(&mut desktop, "conversation-recipient", "Builder"));
  // Details takes 320 at the right: the composer and the toolbar's search fit what is left.
  desktop.click("conversation-details");
  desktop.wait_for("activity-invocation");
  desktop.pass();
  for id in [
    "conversation-recipient",
    "conversation-attach",
    "conversation-send",
    "conversation-send-options",
    "conversation-search",
    "conversation-live-state",
  ] {
    assert!(fits(&mut desktop, id, 608.0), "{id} beside Details at 960");
  }
  release(&desktop);
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}
