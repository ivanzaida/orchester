//! The Conversation tab's composer in each state of the run (ORC-61, one rounded box since `YJfPo`),
//! end to end, headlessly, on the real runner and `orchester-fake-codex`: while an agent works, Send
//! and Enter reach it after the tool call it runs (delivered mid-turn), and *Send after this turn*
//! — the send menu's, or Ctrl+Enter — queues the message until its turn ends; a message its CLI
//! refuses is kept with *Send after this turn* and *Edit in composer*; a question is answered, a
//! plan's changes requested, a run ready for review followed up, a failed run continued with
//! instructions, a paused run's message queued and a draft task's message kept for its run's start.
//! Shift+Enter adds a line. The field's placeholder says whom it writes to in each state, what it
//! answers or changes stands above it, the send button's tooltip says what sending does and its icon
//! whether the message queues, and the run's permissions chip opens their popover.

use orchester_core::{
  conversation::{Arrival, Delivery},
  resource::Step,
  run::RunStatus,
  store::{ConversationStore, RunStore, TaskStore},
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_started, agent_step, create_and_run, fill_new_task, finished_on_page, has, init_repository, latest_run,
    programs, run_finished, seed, seed_steps,
  },
};

fn desktop() -> Desktop {
  let (runner, fake) = programs();
  Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false))
}

pub(crate) fn holds(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  has(desktop, id) && shows(desktop, id, text)
}

/// Where the person's messages to the project's only task stand, as stored; an agent's message has
/// no delivery and is left out.
fn deliveries(desktop: &Desktop) -> Vec<Option<Delivery>> {
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  let task = db.tasks().expect("tasks").remove(0);
  db.messages(task.id())
    .expect("messages")
    .iter()
    .filter_map(orchester_core::conversation::Message::delivery)
    .map(Some)
    .collect()
}

/// What the empty composer field says.
fn placeholder(desktop: &mut Desktop) -> String {
  desktop
    .tree
    .get_element_by_id_mut("conversation-input")
    .and_then(|element| element.as_text_input())
    .and_then(|input| input.placeholder())
    .expect("the composer's placeholder")
}

/// Whether `upper` ends above where `lower` starts.
fn above(desktop: &mut Desktop, upper: &str, lower: &str) -> bool {
  let (upper, lower) = (desktop.bounds(upper), desktop.bounds(lower));
  upper.height > 0.0 && upper.y + upper.height <= lower.y + 0.5
}

/// An element whose id starts with `prefix` says `text`.
fn says(desktop: &mut Desktop, prefix: &str, text: &str) -> bool {
  desktop
    .tree
    .find_element(|element| {
      element.id().is_some_and(|id| id.starts_with(prefix)) && crate::github::contains_text(element, text)
    })
    .is_some()
}

/// Writes `text` into the composer: the focus in its field, then the keys.
fn write(desktop: &mut Desktop, text: &str) {
  desktop.click("conversation-input");
  desktop.type_text(text);
}

/// Whether the send button's tooltip, shown while the pointer is over it, says `text`.
pub(crate) fn tip_says(desktop: &mut Desktop, text: &str) -> bool {
  desktop.hover("conversation-send");
  holds(desktop, "conversation-send-tip", text)
}

/// Shift+Enter in the focused field.
fn new_line(desktop: &mut Desktop) {
  desktop
    .tree
    .key_down("Enter".into(), "Enter".into(), true, false, false);
  desktop.pass();
}

/// Ctrl+Enter in the focused field.
fn send_with_keyboard(desktop: &mut Desktop) {
  desktop
    .tree
    .key_down("Enter".into(), "Enter".into(), false, true, false);
  desktop.pass();
}

pub(crate) fn release(desktop: &Desktop) {
  std::fs::write(desktop.repo().join(".orchester").join("fake-release"), "go").expect("release");
}

/// The run's Conversation tab once its agent works.
pub(crate) fn working_conversation(brief: &str) -> Desktop {
  let mut desktop = desktop();
  create_and_run(&mut desktop, brief, "1400000");
  desktop.wait_until("the agent at work", |desktop| agent_started(desktop.repo()));
  desktop.click("task-tab-conversation");
  desktop.wait_until("the composer for the agent at work", |desktop| {
    has(desktop, "conversation-send-options")
  });
  desktop
}

#[test]
fn composer_sends_now_to_the_agent_at_work_after_its_running_tool_call() {
  let mut desktop = working_conversation("Wait for a message.");
  assert!(shows(&mut desktop, "conversation-recipient", "Builder"));
  assert!(shows(&mut desktop, "conversation-recipient", "working"));
  assert_eq!(placeholder(&mut desktop), "Message Builder…");
  // What sending does is the send button's tooltip; the line under the box says when Builder gets it,
  // and stays between steps (ORC-71).
  assert!(tip_says(
    &mut desktop,
    "Send now · Enter. Builder gets it at its next tool call. Ctrl+Enter sends after this turn."
  ));
  assert!(holds(
    &mut desktop,
    "conversation-explanation",
    "Builder gets your message at its next tool call."
  ));
  // Nothing to send yet.
  desktop.click("conversation-send");
  assert!(deliveries(&desktop).is_empty());

  write(&mut desktop, "Also cover the cancelled dialog.");
  desktop.click("conversation-send");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  assert_eq!(desktop.value("conversation-input"), "", "the draft is stored");
  let stored = deliveries(&desktop);
  assert!(
    matches!(
      stored[..],
      [Some(Delivery::Acknowledged {
        arrival: Arrival::MidTurn,
        ..
      })]
    ),
    "{stored:?}"
  );
  // The finished step folds; opened, it shows the message delivered mid-turn and the agent's note
  // on it, from the turn it was sent in.
  open_finished_step(&mut desktop);
  desktop.wait_until("delivered mid-turn", |desktop| {
    holds(desktop, "conversation", "Delivered mid-turn to Builder")
  });
  assert!(shows(&mut desktop, "conversation", "at its next tool call"));
  assert!(shows(
    &mut desktop,
    "conversation",
    "Noted: Also cover the cancelled dialog."
  ));
}

/// Waits until the timeline shows the run ended, the step it ended on open.
fn open_finished_step(desktop: &mut Desktop) {
  desktop.wait_until("the run's end", |desktop| has(desktop, "conversation-end"));
}

#[test]
fn composer_sends_after_this_turn_with_ctrl_enter_and_it_waits_for_the_turn() {
  let mut desktop = working_conversation("Wait for release.");
  write(&mut desktop, "Rename the test.");
  // Shift+Enter adds a line; Ctrl+Enter sends after this turn while the agent works.
  new_line(&mut desktop);
  desktop.type_text("Then run it.");
  assert_eq!(desktop.value("conversation-input"), "Rename the test.\nThen run it.");
  send_with_keyboard(&mut desktop);
  desktop.wait_until("queued for the turn's end", |desktop| {
    holds(desktop, "conversation", "Queued · Builder reads it when its turn ends")
  });
  assert_eq!(deliveries(&desktop), [Some(Delivery::Queued)]);
  release(&desktop);
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  open_finished_step(&mut desktop);
  desktop.wait_until("delivered at the end of the turn", |desktop| {
    holds(desktop, "conversation", "Delivered at end of turn to Builder")
  });
}

#[test]
fn composer_send_menu_sends_this_message_after_the_turn_and_is_not_remembered() {
  let mut desktop = working_conversation("Wait for release.");
  write(&mut desktop, "Rename the test.");
  desktop.click("conversation-send-options");
  desktop.wait_for("conversation-send-menu");
  assert!(holds(
    &mut desktop,
    "conversation-send-now",
    "Builder gets it at its next tool call"
  ));
  assert!(shows(&mut desktop, "conversation-send-now", "Enter"));
  assert!(holds(
    &mut desktop,
    "conversation-send-after-turn",
    "Waits until Builder finishes its turn"
  ));
  assert!(shows(&mut desktop, "conversation-send-after-turn", "Ctrl+Enter"));
  desktop.click("conversation-send-after-turn");
  desktop.wait_until("queued for the turn's end", |desktop| {
    holds(desktop, "conversation", "Queued · Builder reads it when its turn ends")
  });
  assert!(!has(&mut desktop, "conversation-send-menu"), "choosing closes the menu");
  assert_eq!(deliveries(&desktop), [Some(Delivery::Queued)]);
  // The next message goes now again: the choice was for that message only.
  assert!(tip_says(&mut desktop, "Send now · Enter."));
  release(&desktop);
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}

#[test]
fn composer_keeps_a_message_the_cli_did_not_take_and_offers_it_back() {
  let mut desktop = working_conversation("Refuse a message.");
  write(&mut desktop, "Use the new API.");
  desktop.click("conversation-send");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  open_finished_step(&mut desktop);
  desktop.wait_until("not delivered", |desktop| {
    holds(
      desktop,
      "conversation",
      "Could not deliver to Builder: its CLI refused to add it to the running turn. Your message is kept here.",
    )
  });
  assert!(matches!(
    deliveries(&desktop)[..],
    [Some(Delivery::NotDelivered { .. })]
  ));
  let id = desktop
    .tree
    .root()
    .and_then(|root| find_id(root, "conversation-edit-in-composer-"))
    .expect("Edit in composer");
  desktop.click(&id);
  assert_eq!(desktop.value("conversation-input"), "Use the new API.");
  assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
  let after_turn = id.replace("conversation-edit-in-composer-", "conversation-send-after-turn-");
  assert!(has(&mut desktop, &after_turn));
}

/// The id of the first element whose id starts with `prefix`.
fn find_id(element: lurq::node::element::ElementRef<'_>, prefix: &str) -> Option<String> {
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
    return Some(id.to_owned());
  }
  element.children().into_iter().find_map(|child| find_id(child, prefix))
}

#[test]
fn composer_answers_a_question_and_the_agent_goes_on() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Ask first.", "1400000");
  desktop.wait_until("the question", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.waits_for_a_person())
  });
  desktop.click("task-tab-conversation");
  desktop.wait_until("the question's card", |desktop| has(desktop, "conversation-reply"));
  // Opened on the question, the composer takes the focus to answer it.
  assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
  assert!(shows(&mut desktop, "conversation", "Builder asks you"));
  assert!(says(&mut desktop, "conversation-asks-", "asks you · "));
  assert!(shows(&mut desktop, "conversation", "Keep email sign-in?"));
  assert!(shows(&mut desktop, "conversation", "Waiting for your answer"));
  assert!(holds(
    &mut desktop,
    "conversation-rail-step-1",
    "Builder · waiting for your answer"
  ));
  assert!(holds(
    &mut desktop,
    "conversation-composer-context",
    "Answering Builder’s question from"
  ));
  assert!(above(
    &mut desktop,
    "conversation-composer-context",
    "conversation-composer"
  ));
  assert_eq!(placeholder(&mut desktop), "Answer Builder…");
  // The recipient is fixed to the agent that asked, with a lock.
  assert!(shows(&mut desktop, "conversation-recipient", "asking you"));
  assert!(tip_says(
    &mut desktop,
    "Send answer · Enter. Your reply resumes Builder's session."
  ));
  // Reply puts the focus in the composer.
  desktop.click("conversation-reply");
  assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
  desktop.type_text("Yes, keep it.");
  send_with_keyboard(&mut desktop);
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  open_finished_step(&mut desktop);
  desktop.wait_until("the answered question", |desktop| {
    holds(desktop, "conversation", "Answered")
  });
  assert!(shows(&mut desktop, "conversation", "Delivered to Builder"));
}

#[test]
fn composer_requests_changes_to_a_plan_and_its_card_approves_it() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        Step {
          approval_gate: true,
          ..agent_step(builder, "plan", "Plan", "Propose a plan.", &[])
        },
        agent_step(builder, "implement", "Implement", "Follow the plan.", &["plan"]),
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the plan", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.waits_for_a_person())
  });
  desktop.click("task-tab-conversation");
  desktop.wait_until("the plan's card", |desktop| has(desktop, "conversation-approve-plan"));
  assert!(shows(&mut desktop, "conversation", "Plan v1 · Say hello"));
  assert!(holds(
    &mut desktop,
    "conversation-rail-step-1",
    "Builder · waiting for approval"
  ));
  assert!(holds(&mut desktop, "conversation-rail-step-2", "Builder · not started"));
  assert!(shows(&mut desktop, "conversation-live-state", "Waiting for you"));
  assert!(holds(
    &mut desktop,
    "conversation-approve-meaning",
    "Approves plan v1 and starts Builder on Implement."
  ));
  assert!(holds(
    &mut desktop,
    "conversation-composer-context",
    "Changes to plan v1 · sent to Builder"
  ));
  assert!(above(
    &mut desktop,
    "conversation-composer-context",
    "conversation-composer"
  ));
  assert_eq!(placeholder(&mut desktop), "Describe the changes the plan needs…");
  assert!(tip_says(&mut desktop, "Request changes · Enter."));
  // Unavailable while empty.
  desktop.click("conversation-send");
  assert!(
    desktop
      .tree
      .get_element_by_id("conversation-composer-context")
      .is_some()
  );

  write(&mut desktop, "Greet in German too.");
  desktop.click("conversation-send");
  desktop.wait_until("plan v2", |desktop| {
    holds(desktop, "conversation", "Plan v2 · Say hello")
  });
  assert!(shows(&mut desktop, "conversation", "Changes requested"));
  desktop.wait_for("conversation-approve-plan");
  desktop.click("conversation-approve-plan");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  open_finished_step(&mut desktop);
  // Plan, the step before the one the run ended on, stays open; the rail says it was approved.
  assert!(shows(&mut desktop, "conversation-rail-step-1", "approved"));
  desktop.wait_until("the approved plan", |desktop| {
    holds(desktop, "conversation", "Plan done")
  });
  // Its result card leaves the plan to the plan's cards.
  let plan = plan_text(&desktop);
  assert!(says(&mut desktop, "conversation-result-", "Plan done"));
  assert!(!says(&mut desktop, "conversation-result-", &plan), "{plan}");
}

/// What the latest plan of the run says, as stored.
fn plan_text(desktop: &Desktop) -> String {
  let run = latest_run(desktop.repo()).expect("the run");
  ProjectDb::open(desktop.repo())
    .expect("open the project")
    .plans(run.id())
    .expect("plans")
    .pop()
    .expect("a plan")
    .text
}

#[test]
fn composer_follows_up_a_run_ready_for_review() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  finished_on_page(&mut desktop);
  desktop.click("task-ask-agent");
  desktop.wait_for("conversation-composer");
  assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
  assert!(tip_says(&mut desktop, "Send follow-up · Enter."));
  assert_eq!(placeholder(&mut desktop), "Ask a question or request a follow-up…");
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
          Some(orchester_core::run::AttemptReason::FollowUp { .. })
        )
    })
  });
  let stored = deliveries(&desktop);
  assert!(
    matches!(
      stored[..],
      [Some(Delivery::Acknowledged {
        arrival: Arrival::Prompt,
        ..
      })]
    ),
    "{stored:?}"
  );
  desktop.wait_until("ready to follow up again", |desktop| {
    tip_says(desktop, "Send follow-up")
  });
}

#[test]
fn composer_continues_a_failed_run_with_instructions() {
  let mut desktop = desktop();
  create_and_run(&mut desktop, "Report failure.", "1400000");
  desktop.wait_until("the failed run", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| matches!(run.status(), RunStatus::Failed(_)))
  });
  desktop.click("task-tab-conversation");
  desktop.wait_until("the composer of the failed run", |desktop| {
    tip_says(desktop, "Continue with instructions · Enter.")
  });
  desktop.wait_until("the run's end", |desktop| has(desktop, "conversation-end"));
  assert!(holds(&mut desktop, "conversation-end", "Run 1 failed at Implement"));
  assert!(shows(&mut desktop, "conversation", "Implement failed"));
  assert!(shows(&mut desktop, "conversation", "Builder reported failed"));
  assert!(shows(&mut desktop, "conversation-live-state", "Failed"));
  assert_eq!(placeholder(&mut desktop), "Tell Builder how to continue the run…");
  write(&mut desktop, "Continue with the corrected instruction.");
  send_with_keyboard(&mut desktop);
  desktop.wait_until("the recovered run", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Succeeded && run.attempts().len() == 2)
  });
}

#[test]
fn composer_queues_a_message_while_the_run_is_paused() {
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
    holds(desktop, "conversation-explanation", "You can write while paused.")
  });
  assert!(!has(&mut desktop, "conversation-send-options"), "no agent works");
  assert!(tip_says(&mut desktop, "Send · Enter."));
  write(&mut desktop, "Check the OAuth dialog.");
  desktop.click("conversation-send");
  desktop.wait_until("queued for the resume", |desktop| {
    holds(
      desktop,
      "conversation",
      "Queued · delivered before the next action on resume",
    )
  });
}

#[test]
fn composer_keeps_a_draft_task_s_message_for_its_run_s_start() {
  let mut desktop = desktop();
  fill_new_task(&mut desktop, "Say hello.", "1400000");
  desktop.click("save-draft");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-tab-conversation");
  desktop.click("task-tab-conversation");
  desktop.wait_until("the composer of the draft", |desktop| {
    tip_says(desktop, "Send when the run starts · Enter.")
  });
  assert!(holds(&mut desktop, "conversation-empty", "This task has not run yet"));
  assert!(shows(
    &mut desktop,
    "conversation-explanation",
    "Messages reach the first step when the run starts."
  ));
  write(&mut desktop, "Keep email sign-in.");
  desktop.click("conversation-send");
  desktop.wait_until("queued for the start", |desktop| {
    holds(desktop, "conversation", "Queued · waiting for the run to start")
  });
}

#[test]
fn composer_tab_opens_at_the_question_that_waits() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        agent_step(builder, "research", "Research", "Write own file.", &[]),
        agent_step(builder, "plan", "Plan", "Write own file.", &["research"]),
        agent_step(builder, "design", "Design", "Write own file.", &["plan"]),
        agent_step(builder, "prepare", "Prepare", "Write own file.", &["design"]),
        agent_step(builder, "implement", "Implement", "Ask first.", &["prepare"]),
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the question", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.waits_for_a_person())
  });
  // A short window: the earlier steps and the step's own items fill the timeline before the card.
  desktop.tree.resize(1440, 700);
  desktop.pass();
  for _ in 0..2 {
    desktop.click("task-tab-conversation");
    desktop.wait_until("the question's card", |desktop| has(desktop, "conversation-reply"));
    for _ in 0..4 {
      desktop.pass();
    }
    assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
    let (toolbar, dock) = (
      desktop.bounds("conversation-toolbar"),
      desktop.bounds("conversation-dock"),
    );
    let card = desktop
      .tree
      .find_element_mut(|element| element.id().is_some_and(|id| id.starts_with("conversation-question-")))
      .map(|element| element.bounds())
      .expect("the question's card");
    assert!(
      card.y >= toolbar.y + toolbar.height - 0.5 && card.y < dock.y,
      "{card:?} starts between {toolbar:?} and {dock:?}"
    );
    // Opening the tab again lands on it again.
    desktop.click("task-tab-summary");
    desktop.wait_until("the summary", |desktop| !has(desktop, "conversation-reply"));
  }
}
