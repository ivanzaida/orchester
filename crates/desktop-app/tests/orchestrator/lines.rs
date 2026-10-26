//! The chat's cards in their new sizes (`Orchestrator cards · redesign` `zuvUi`): an agent's wait as a line with its
//! status chip in each status, what the Orchestrator did with it nested under it; waits for the person kept one after
//! another gathered into one digest that counts them; an action refused as a danger line whose reason opens under
//! it; and a proposal as a decision card that, decided, folds into its line.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, RunId},
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, TurnState, WaitKind,
    change::{Change, ProposedChange},
  },
  resource::ModelId,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::harness::{Orchestrator, Setup};

/// A wait of `kind` of task `task`'s run `#number`, by agent *Builder*, answered by the Orchestrator itself when
/// `automatic`.
fn wait(kind: WaitKind, (task, number): (&str, u32), automatic: bool) -> AgentWait {
  AgentWait {
    kind,
    task: task.to_owned(),
    title: format!("Work of {task}"),
    run: RunId::generate(),
    number,
    step: None,
    place: None,
    others: 0,
    agent: Some("Builder".to_owned()),
    link: format!("/tasks/{task}"),
    own: false,
    automatic,
    continuing: None,
    merging: None,
  }
}

/// Keeps `wait` in `chat`; its entry.
fn keep(db: &mut ProjectDb, chat: &Chat, wait: &AgentWait) -> i64 {
  let key = format!("{}-{}", wait.task, wait.number);
  assert!(db.keep_agent_wait(chat, (&key, wait), Timestamp::now()).expect("kept"));
  db.chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .rev()
    .find_map(|entry| matches!(entry.kind, EntryKind::Wait { .. }).then_some(entry.seq))
    .expect("the wait's entry")
}

/// An answer to run `#number`'s agent, carried out as `outcome` says.
fn answer(number: u32, outcome: ActionOutcome) -> ChatAction {
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Answer {
        run: RunId::generate(),
        number,
        request: "question".to_owned(),
        choices: Vec::new(),
        text: Some("Decisions inside the new layout.".to_owned()),
      },
    })),
    ..ChatAction::new("answer_agent", Some("KON-1".to_owned()), outcome)
  }
}

/// Gives wait `seq` of `chat` a turn that answers the agent as `outcome` says and ends.
fn answered(db: &mut ProjectDb, (chat, seq): (ChatId, i64), (number, outcome): (u32, ActionOutcome)) {
  let message = ChatMessageId::generate();
  assert!(db.start_wait_turn((chat, seq), message).expect("started"));
  db.add_chat_reply(
    chat,
    (
      "Chose Decisions inside the new layout.",
      &ModelId::new("haiku").expect("model"),
    ),
    Timestamp::now(),
  )
  .expect("reply");
  db.add_chat_action(chat, &answer(number, outcome), Timestamp::now())
    .expect("action");
  db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
}

/// The line of wait `seq` in `chat`: named by the message of the turn it started, else by its entry.
fn row(screen: &Orchestrator, (chat, seq): (ChatId, i64)) -> String {
  let turn = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Wait { turn, .. } if entry.seq == seq => Some(turn.map(|turn| turn.message)),
      _ => None,
    })
    .expect("the wait's entry");
  turn.map_or_else(
    || format!("orchestrator-wait-wait-{seq}"),
    |message| format!("orchestrator-wait-{message}"),
  )
}

#[test]
fn orchestrator_wait_line_says_each_status_and_nests_the_answer() {
  // No runner: nothing answers the seeded turns but the store.
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let mut db = screen.db();
  let chat = Chat::named(ChatId::generate(), "Waits", Timestamp::now());
  let waits = keep(&mut db, &chat, &wait(WaitKind::Question, ("KON-1", 1), false));
  let by_it = keep(&mut db, &chat, &wait(WaitKind::Question, ("KON-1", 2), true));
  answered(&mut db, (chat.id(), by_it), (2, ActionOutcome::Done));
  let by_you = keep(&mut db, &chat, &wait(WaitKind::Message, ("KON-1", 3), true));
  answered(&mut db, (chat.id(), by_you), (3, ActionOutcome::Applied));
  let reading = keep(&mut db, &chat, &wait(WaitKind::Question, ("KON-1", 4), true));
  assert!(
    db.start_wait_turn((chat.id(), reading), ChatMessageId::generate())
      .expect("started")
  );
  let [waits, by_it, by_you, reading] = [waits, by_it, by_you, reading].map(|seq| row(&screen, (chat.id(), seq)));
  screen.go(OrchestratorChat::Latest);
  screen.until("the waits", |screen| screen.exists(&reading));
  for (line, status) in [
    (&waits, "Waits for you"),
    (&by_it, "Answered by the Orchestrator"),
    (&by_you, "Answered by you"),
    (&reading, "Orchestrator is reading"),
  ] {
    assert_eq!(screen.text(&format!("{line}-chip")), status);
  }
  let shown = screen.text(&waits);
  for part in ["Builder asks a question", "KON-1 · #1", "Open ↗"] {
    assert!(shown.contains(part), "{part} in {shown}");
  }
  let line = screen.bounds(&by_it);
  assert!((line.height - 32.0).abs() < 0.5, "a wait is one 32 line: {line:?}");
  // What the Orchestrator said nests under the wait, 46 in from the line, without its avatar.
  let turn = by_it.trim_start_matches("orchestrator-wait-");
  let nested = screen.bounds(&format!("orchestrator-answer-{turn}-parts"));
  assert!(
    screen
      .text(&format!("orchestrator-answer-{turn}"))
      .contains("Chose Decisions inside the new layout."),
    "the answer nests under the wait"
  );
  assert!(nested.x >= line.x + 46.0 - 0.5, "{nested:?} in from {line:?}");
}

#[test]
fn orchestrator_waits_for_the_person_gather_into_a_digest_that_counts_them() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let mut db = screen.db();
  let chat = Chat::named(ChatId::generate(), "While you were away", Timestamp::now());
  let first = keep(&mut db, &chat, &wait(WaitKind::Message, ("KON-13", 9), false));
  let seqs = [
    first,
    keep(&mut db, &chat, &wait(WaitKind::Question, ("KON-12", 8), false)),
    keep(&mut db, &chat, &wait(WaitKind::Plan, ("KON-16", 11), false)),
  ];
  screen.go(OrchestratorChat::Latest);
  let digest = format!("orchestrator-digest-wait-{first}");
  screen.until("the digest", |screen| screen.exists(&digest));
  assert_eq!(screen.text(&format!("{digest}-title")), "3 waiting for you");
  for seq in seqs {
    let item = format!("orchestrator-digest-item-wait-{seq}");
    assert!(screen.exists(&item), "one line per wait");
    assert!((screen.bounds(&item).height - 34.0).abs() < 0.5, "each 34 high");
    assert!(
      !screen.exists(&format!("orchestrator-wait-wait-{seq}")),
      "not a line of its own"
    );
  }
  screen.click(&format!("{digest}-head"));
  screen.until("the digest folded", |screen| {
    !screen.exists(&format!("orchestrator-digest-item-wait-{first}"))
  });
  assert_eq!(screen.text(&format!("{digest}-title")), "3 waiting for you");
}

/// A conversation that starts with `text`, answered, and the action `action` kept in its answer; the conversation's
/// route and the action's card.
fn with_action(screen: &Orchestrator, text: &str, action: &ChatAction) -> (OrchestratorChat, String) {
  let mut db = screen.db();
  let at = Timestamp::now();
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let body = MessageBody::new(text).expect("text");
  db.keep_chat_message(&Chat::start(chat, &body, at), (message, &body), at)
    .expect("kept");
  db.add_chat_reply(chat, ("Done.", &ModelId::new("haiku").expect("model")), at)
    .expect("reply");
  db.add_chat_action(chat, action, at).expect("action");
  db.end_chat_turn(message, &TurnState::Answered, at).expect("ended");
  (OrchestratorChat::Chat(chat), format!("orchestrator-action-{message}-1"))
}

#[test]
fn orchestrator_refused_action_is_a_danger_line_whose_reason_opens_under_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let refused = ChatAction::new(
    "run_task",
    Some("KON-19".to_owned()),
    ActionOutcome::Refused {
      code: "blocked".to_owned(),
      reason: "KON-19 waits for KON-18 to merge.".to_owned(),
    },
  );
  let (chat, card) = with_action(&screen, "Start KON-19", &refused);
  screen.go(chat);
  screen.until("the refused line", |screen| screen.exists(&card));
  assert_eq!(screen.text(&format!("{card}-title")), "Start KON-19 · not done");
  assert!(
    (screen.bounds(&card).height - 32.0).abs() < 0.5,
    "an event line, not a card"
  );
  assert!(
    !screen.exists(&format!("{card}-reason")),
    "its reason waits to be opened"
  );
  screen.click(&format!("{card}-line"));
  screen.until("the reason", |screen| {
    screen
      .text(&format!("{card}-reason"))
      .contains("KON-19 waits for KON-18 to merge.")
  });
}

#[test]
fn orchestrator_decided_proposal_folds_into_its_line() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let release = |outcome| ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Release { task: None },
    })),
    ..ChatAction::new("release_queue", None, outcome)
  };
  let (chat, waiting) = with_action(&screen, "Release?", &release(ActionOutcome::Waits));
  screen.go(chat);
  screen.until("the decision card", |screen| screen.exists(&format!("{waiting}-start")));
  assert_eq!(
    screen.text(&format!("{waiting}-title")),
    "Let the project's new starts go on?"
  );
  assert!(screen.text(&format!("{waiting}-state")).starts_with("Waits for you · "));
  assert_eq!(screen.text(&format!("{waiting}-person")), "Only you can apply");
  assert!(screen.exists(&format!("{waiting}-dismiss")));
  assert!(
    !screen.exists(&format!("{waiting}-effect")),
    "a release says what it does on hover only"
  );

  let (chat, applied) = with_action(&screen, "Released?", &release(ActionOutcome::Applied));
  screen.go(chat);
  screen.until("the folded line", |screen| screen.exists(&applied));
  assert_eq!(
    screen.text(&format!("{applied}-title")),
    "Released the project's new starts"
  );
  assert_eq!(screen.text(&format!("{applied}-target")), "by you");
  assert!(!screen.exists(&format!("{applied}-start")), "nothing is left to decide");
  assert!(
    (screen.bounds(&applied).height - 32.0).abs() < 0.5,
    "decided, the card is a line"
  );
}

#[test]
fn orchestrator_answer_card_quotes_the_answer_it_would_send() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (chat, card) = with_action(&screen, "Answer it", &answer(95, ActionOutcome::Waits));
  screen.go(chat);
  screen.until("the decision card", |screen| screen.exists(&format!("{card}-start")));
  assert_eq!(
    screen.text(&format!("{card}-quote")),
    "Decisions inside the new layout.",
    "the answer is the card's quote"
  );
  assert!(
    !screen.exists(&format!("{card}-effect")),
    "what sending does is the question's tooltip, not a paragraph"
  );
  assert_eq!(screen.text(&format!("{card}-start")), "Send answer");
}
