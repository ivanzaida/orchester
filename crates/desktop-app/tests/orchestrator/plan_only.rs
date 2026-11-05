//! Plan-only mode (ORC-646, `12`–`12d` in `hujxu`): the composer's *Plan only* switch puts the *Mode* line on top of
//! the box and is kept with the conversation, also as a later session finds it; a message sent with it on is
//! *Delivered · plan only*, its tool calls that change something are refused and the refusal shows, and its answer is
//! a plan card. Only a person approves the plan (automation's input is refused, visibly); *Approve and do it* folds the
//! card and carries the plan out as an ordinary turn; *Change it* holds the plan as a chip in the composer and sends
//! the change as a turn that plans; *Dismiss* folds it.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{ActionOutcome, Chat, ChatPlan, EntryKind, PlanState, TurnMode, TurnState},
  run::Answerer,
  store::TaskStore,
  time::Timestamp,
};

use super::harness::{Orchestrator, Setup};

/// A plan of two steps, as the agent proposes it.
const PLAN: &str = r#"Call propose_plan {"project":"Kontur","steps":[{"kind":"run","action":"Continue run #8","target":"KON-12 · from its last checkpoint"},{"kind":"draft","action":"Create a draft","target":"Per-endpoint retry settings"}],"left_out":"Run #11: its branch conflicts with main","uses":"1 run and 1 draft"}"#;

/// Writes `text` and sends it with Ctrl+Enter once the composer is free, whatever its placeholder says.
fn send(screen: &mut Orchestrator, text: &str) {
  screen.until("the composer to be free", |screen| {
    screen.text("orchestrator-input").is_empty() || !screen.exists("orchestrator-thinking")
  });
  screen.set("orchestrator-input", text);
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
  screen.key(("Enter", "Enter"), true);
}

/// The conversation's plans, by entry, oldest first.
fn plans(screen: &Orchestrator) -> Vec<(i64, ChatPlan)> {
  let db = screen.db();
  let Some(listing) = db.chats().expect("chats").pop() else {
    return Vec::new();
  };
  db.chat_entries(listing.chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Plan(plan) => Some((entry.seq, *plan)),
      _ => None,
    })
    .collect()
}

/// The codes of the conversation's refused actions.
fn refusals(screen: &Orchestrator) -> Vec<String> {
  let db = screen.db();
  let Some(listing) = db.chats().expect("chats").pop() else {
    return Vec::new();
  };
  db.chat_entries(listing.chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Action(action) => match action.outcome {
        ActionOutcome::Refused { code, .. } => Some(code),
        _ => None,
      },
      _ => None,
    })
    .collect()
}

/// The modes of the conversation's person's messages, in order.
fn modes(screen: &Orchestrator) -> Vec<TurnMode> {
  let db = screen.db();
  let chat = db.chats().expect("chats").remove(0).chat.id();
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Person { mode, .. } => Some(mode),
      _ => None,
    })
    .collect()
}

/// A new conversation with the switch on, whose first turn proposed [`PLAN`] and ended; the plan's card.
fn planned(screen: &mut Orchestrator) -> String {
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-plan-only");
  screen.until("the mode line", |screen| screen.exists("orchestrator-plan-mode"));
  send(screen, PLAN);
  screen.until("the plan", |screen| !plans(screen).is_empty());
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let card = format!("orchestrator-plan-{}", plans(screen)[0].0);
  screen.until("the plan's card", |screen| screen.exists(&format!("{card}-approve")));
  card
}

#[test]
fn orchestrator_plan_only_refuses_what_changes_and_answers_with_a_plan() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  assert!(!screen.exists("orchestrator-plan-mode"), "the switch starts off");
  screen.click("orchestrator-plan-only");
  screen.until("the mode line", |screen| screen.exists("orchestrator-plan-mode"));
  assert!(
    screen.text("orchestrator-input").starts_with("Ask for a plan"),
    "the placeholder asks for a plan: {}",
    screen.text("orchestrator-input")
  );
  send(
    &mut screen,
    r#"Call create_task {"project":"Kontur","title":"Per-endpoint retry settings"}"#,
  );
  screen.until("the refusal", |screen| !refusals(screen).is_empty());
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  assert_eq!(refusals(&screen), ["plan_only"]);
  assert!(screen.db().tasks().expect("tasks").is_empty(), "nothing was created");
  let shown = screen.text("orchestrator-conversation");
  assert!(shown.contains("plan only"), "the delivery line says plan only: {shown}");
  assert!(shown.contains("not done"), "the refusal shows: {shown}");
  let chat = screen.db().chats().expect("chats").remove(0).chat;
  assert!(chat.plan_only(), "the conversation keeps the switch");

  send(&mut screen, PLAN);
  screen.until("the plan", |screen| !plans(screen).is_empty());
  let card = format!("orchestrator-plan-{}", plans(&screen)[0].0);
  screen.until("the plan's card", |screen| screen.exists(&format!("{card}-approve")));
  assert_eq!(screen.text(&format!("{card}-title")), "Plan · 2 steps");
  let steps = screen.text(&format!("{card}-steps"));
  assert!(
    steps.contains("Continue run #8") && steps.contains("KON-12 · from its last checkpoint"),
    "{steps}"
  );
  assert!(
    screen
      .text(&format!("{card}-facts"))
      .contains("Run #11: its branch conflicts with main")
  );
  assert!(screen.text(&format!("{card}-state")).starts_with("Waits for you"));
  assert_eq!(modes(&screen), [TurnMode::Plans, TurnMode::Plans]);
}

#[test]
fn orchestrator_plan_is_approved_by_a_person_only_and_carried_out() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let card = planned(&mut screen);
  let approve = format!("{card}-approve");
  with_synthetic_input(|| screen.click(&approve));
  screen.until("the refusal", |screen| screen.exists(&format!("{card}-person-only")));
  assert!(plans(&screen)[0].1.waits(), "automation's input approves nothing");

  screen.click(&approve);
  screen.until("the approval", |screen| {
    matches!(plans(screen)[0].1.state(), PlanState::Approved { .. })
  });
  screen.until("the turn that carries it out", |screen| {
    modes(screen).last() == Some(&TurnMode::CarriesOut) && !screen.exists("orchestrator-thinking")
  });
  screen.until("the folded card", |screen| {
    screen.text(&format!("{card}-title")) == "Plan approved · 2 steps"
  });
  assert!(!screen.exists(&approve), "nothing is left to decide");
  // The fake agent echoes the message it takes as `Noted: <message>`, so the hidden message's words
  // appear once, inside the carry-out turn's answer, and never as a message row of their own.
  let approved = "[Orchester] The person approved your plan";
  let noted = format!("Noted: {approved}");
  screen.until("the carry-out turn's answer", |screen| {
    screen.text("orchestrator-conversation").contains(&noted)
  });
  let shown = screen.text("orchestrator-conversation");
  assert_eq!(
    shown.matches(approved).count(),
    shown.matches(&noted).count(),
    "the turn that carries it out shows no message of its own: {shown}"
  );
  screen.click(&format!("{card}-show"));
  screen.until("the plan under its line", |screen| {
    screen.text(&format!("{card}-steps")).contains("Create a draft")
  });
  assert!(screen.exists("orchestrator-plan-mode"), "the mode stays on");
}

#[test]
fn orchestrator_plan_changed_is_replaced_and_dismissed_does_nothing() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let card = planned(&mut screen);
  screen.click(&format!("{card}-change"));
  screen.until("the chip", |screen| screen.exists("orchestrator-plan-changing"));
  assert!(
    screen
      .text("orchestrator-plan-changing")
      .starts_with("Changing: Plan · 2 steps")
  );
  screen.until("being changed", |screen| {
    screen.text(&format!("{card}-state")) == "Being changed"
  });
  send(&mut screen, PLAN);
  screen.until("the new plan", |screen| plans(screen).len() == 2);
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  assert!(
    !screen.exists("orchestrator-plan-changing"),
    "the change went with the message"
  );
  assert!(matches!(plans(&screen)[0].1.state(), PlanState::Replaced { .. }));
  screen.until("the replaced line", |screen| {
    screen.text(&format!("{card}-title")) == "Plan replaced by plan 2"
  });

  let second = format!("orchestrator-plan-{}", plans(&screen)[1].0);
  screen.until("the new card", |screen| screen.exists(&format!("{second}-dismiss")));
  screen.click(&format!("{second}-dismiss"));
  screen.until("the dismissal", |screen| {
    matches!(plans(screen)[1].1.state(), PlanState::Dismissed { .. })
  });
  screen.until("the dismissed line", |screen| {
    screen.text(&format!("{second}-title")) == "Plan dismissed · nothing done"
  });
  assert!(screen.db().tasks().expect("tasks").is_empty(), "nothing was done");
}

#[test]
fn orchestrator_plan_only_is_kept_with_the_conversation_for_a_later_session() {
  let mut screen = Orchestrator::new(Setup::Ready);
  // A conversation an earlier session left with its switch on.
  let chat = ChatId::generate();
  let mut db = screen.db();
  let body = MessageBody::new("Plan the webhook retry work").expect("text");
  let kept = Chat::start(chat, &body, Timestamp::now()).with_plan_only(true);
  let message = ChatMessageId::generate();
  db.keep_chat_message_in(
    &kept,
    (message, &body),
    (&[], Answerer::Person),
    (&TurnState::Thinking, Timestamp::now()),
    TurnMode::Plans,
  )
  .expect("kept");
  db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  drop(db);
  screen.go(OrchestratorChat::Chat(chat));
  screen.until("the mode line", |screen| screen.exists("orchestrator-plan-mode"));
  screen.click("orchestrator-plan-only");
  screen.until("the switch off", |screen| {
    !screen.db().chat(chat).expect("read").expect("kept").plan_only()
  });
  assert!(!screen.exists("orchestrator-plan-mode"));
  // Alt+P in the message flips it too (the person's choice over the design's Shift Tab).
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
  screen
    .window
    .tree
    .key_down("p".to_owned(), "KeyP".to_owned(), false, false, true);
  screen
    .window
    .tree
    .key_up("p".to_owned(), "KeyP".to_owned(), false, false, true);
  screen.window.pass();
  screen.until("the switch on", |screen| {
    screen.db().chat(chat).expect("read").expect("kept").plan_only()
  });
  assert!(screen.exists("orchestrator-plan-mode"));
}
