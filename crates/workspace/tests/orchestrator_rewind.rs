//! *Edit* and *Rewind to here* in the Orchestrator chat (ORC-333): a rewind drops every entry from its point on, keeps
//! what the agent did in them on the rewound line, forgets the session that heard them, and gives back the words of
//! the person's message it went back to before; the next turn is told only the conversation as it was kept, with what
//! stays done. A rewind of a busy conversation, or to a point that is not in it, changes nothing and says why.

use std::fs;

use orchester_core::{
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId, ProjectId},
  name::Name,
  orchestrator::{ActionOutcome, Chat, ChatAction, ChatEntry, ChatEvent, EntryKind, RewindPoint, RewoundTo, TurnState},
  project::{Project, ProjectKey},
  resource::ModelId,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::rewind::rewind_in;
use tempfile::TempDir;

/// A project, Kontur, with one conversation of its chat.
struct Conversation {
  repo: TempDir,
  chat: ChatId,
}

impl Conversation {
  fn new() -> Self {
    let repo = TempDir::new().expect("repository");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    ProjectDb::create(repo.path(), project).expect("project");
    Self {
      repo,
      chat: ChatId::generate(),
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("project database")
  }

  /// The person says `text`, and its turn thinks.
  fn says(&self, text: &str) -> ChatMessageId {
    let mut db = self.db();
    let body = MessageBody::new(text).expect("text");
    let chat = db
      .chat(self.chat)
      .expect("read")
      .unwrap_or_else(|| Chat::start(self.chat, &body, Timestamp::now()));
    let message = ChatMessageId::generate();
    assert!(
      db.keep_chat_message(&chat, (message, &body), Timestamp::now())
        .expect("kept")
    );
    message
  }

  /// The Orchestrator answers `message` with `text` and `actions`, on its session, and the turn ends.
  fn answers(&self, message: ChatMessageId, text: &str, actions: &[ChatAction]) {
    let mut db = self.db();
    let model = ModelId::new("haiku").expect("model");
    db.add_chat_reply(self.chat, (text, &model), Timestamp::now())
      .expect("reply");
    for action in actions {
      db.add_chat_action(self.chat, action, Timestamp::now()).expect("action");
    }
    db.set_chat_session(
      self.chat,
      "session-1",
      (ConnectionId::generate(), AgentAccountId::generate()),
    )
    .expect("session");
    db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
      .expect("ended");
  }

  fn entries(&self) -> Vec<ChatEntry> {
    self.db().chat_entries(self.chat).expect("entries")
  }

  /// What the conversation's person and Orchestrator said, in order.
  fn said(&self) -> Vec<String> {
    self
      .entries()
      .into_iter()
      .filter_map(|entry| match entry.kind {
        EntryKind::Person { text, .. } => Some(text.as_str().to_owned()),
        EntryKind::Reply { text, .. } => Some(text),
        _ => None,
      })
      .collect()
  }

  /// The rewound lines the conversation keeps.
  fn rewound(&self) -> Vec<ChatEvent> {
    self
      .entries()
      .into_iter()
      .filter_map(|entry| match entry.kind {
        EntryKind::Event(event @ ChatEvent::Rewound { .. }) => Some(event),
        _ => None,
      })
      .collect()
  }
}

fn created(task: &str) -> ChatAction {
  ChatAction::new("create_task", Some(task.to_owned()), ActionOutcome::Done)
}

/// Two turns, the second of which created a task and was refused another.
fn two_turns(conversation: &Conversation) -> (ChatMessageId, ChatMessageId) {
  let first = conversation.says("What's still open on webhook delivery?");
  conversation.answers(first, "Two things: retries and failed deliveries.", &[]);
  let second = conversation.says("Draft a task for per-endpoint retry settings.");
  let refused = ChatAction::new(
    "run_task",
    Some("KON-31".to_owned()),
    ActionOutcome::Refused {
      code: "no_pipeline".to_owned(),
      reason: "No pipeline.".to_owned(),
    },
  );
  conversation.answers(second, "Drafted KON-31.", &[created("KON-31"), refused]);
  (first, second)
}

#[test]
fn rewind_to_before_a_message_drops_it_and_what_follows_and_keeps_what_was_done() {
  let conversation = Conversation::new();
  let (_, second) = two_turns(&conversation);
  let at = conversation
    .entries()
    .into_iter()
    .find(|entry| matches!(&entry.kind, EntryKind::Person { message, .. } if *message == second))
    .expect("the second message")
    .at;

  let rewound = rewind_in(conversation.repo.path(), conversation.chat, RewindPoint::Before(second)).expect("rewound");

  assert_eq!(
    rewound.words.as_deref(),
    Some("Draft a task for per-endpoint retry settings."),
    "its words go back to the composer"
  );
  assert_eq!((rewound.dropped.mine, rewound.dropped.theirs), (1, 1));
  assert_eq!(
    conversation.said(),
    [
      "What's still open on webhook delivery?",
      "Two things: retries and failed deliveries."
    ]
  );
  assert_eq!(
    conversation.rewound(),
    [ChatEvent::Rewound {
      to: RewoundTo::BeforeMessage { at },
      dropped: 2,
      done: vec![created("KON-31")],
    }],
    "what was done stays, said on the rewound line; what was refused is not"
  );
  let db = conversation.db();
  assert_eq!(
    db.chat(conversation.chat).expect("read").expect("kept").session(),
    None,
    "the session that heard the dropped messages is never resumed"
  );
}

#[test]
fn rewind_after_an_answer_keeps_it_and_drops_what_follows() {
  let conversation = Conversation::new();
  let (first, _) = two_turns(&conversation);
  let entries = conversation.entries();
  let reply = entries
    .iter()
    .find(|entry| matches!(entry.kind, EntryKind::Reply { .. }))
    .expect("the first answer");
  let point = RewindPoint::After {
    last: reply.seq,
    answered: reply.at,
  };

  let rewound = rewind_in(conversation.repo.path(), conversation.chat, point).expect("rewound");

  assert_eq!(rewound.words, None, "nothing goes back to the composer");
  assert_eq!(rewound.dropped.count(), 2);
  assert_eq!(
    conversation.said(),
    [
      "What's still open on webhook delivery?",
      "Two things: retries and failed deliveries."
    ]
  );
  assert!(conversation.db().chat_turn(first).expect("read").is_some());
}

#[test]
fn the_next_turn_after_a_rewind_is_told_only_what_was_kept() {
  let conversation = Conversation::new();
  let (_, second) = two_turns(&conversation);
  rewind_in(conversation.repo.path(), conversation.chat, RewindPoint::Before(second)).expect("rewound");
  let next = conversation.says("Which tasks did you draft?");

  let prompt = conversation.db().chat_turn(next).expect("read").expect("kept").text;

  let prompt = prompt.as_str();
  assert!(prompt.contains("What's still open on webhook delivery?"), "{prompt}");
  assert!(
    prompt.contains("Two things: retries and failed deliveries."),
    "{prompt}"
  );
  assert!(
    prompt.contains("create_task on KON-31 (done)"),
    "it is told what stays done: {prompt}"
  );
  assert!(prompt.ends_with("Which tasks did you draft?"), "{prompt}");
  assert!(
    !prompt.contains("per-endpoint") && !prompt.contains("Drafted KON-31.") && !prompt.contains("No pipeline."),
    "the dropped messages never reach it: {prompt}"
  );

  // Once the new session is kept, the turns after it resume it and are told nothing again.
  conversation.answers(next, "KON-31.", &[]);
  let after = conversation.says("Thanks.");
  let prompt = conversation.db().chat_turn(after).expect("read").expect("kept").text;
  assert_eq!(prompt.as_str(), "Thanks.");
}

#[test]
fn rewinding_past_an_earlier_rewind_keeps_what_it_kept() {
  let conversation = Conversation::new();
  let (first, second) = two_turns(&conversation);
  rewind_in(conversation.repo.path(), conversation.chat, RewindPoint::Before(second)).expect("rewound");

  let rewound = rewind_in(conversation.repo.path(), conversation.chat, RewindPoint::Before(first)).expect("rewound");

  assert_eq!(rewound.dropped.done, [created("KON-31")]);
  assert!(conversation.said().is_empty());
  assert!(
    matches!(conversation.rewound().as_slice(), [ChatEvent::Rewound { done, .. }] if *done == [created("KON-31")]),
    "{:?}",
    conversation.rewound()
  );
}

#[test]
fn a_rewind_while_the_orchestrator_answers_or_to_a_point_not_kept_changes_nothing() {
  let conversation = Conversation::new();
  let (first, _) = two_turns(&conversation);
  let thinking = conversation.says("And what waits?");
  let before = conversation.entries();

  let busy = rewind_in(conversation.repo.path(), conversation.chat, RewindPoint::Before(first));
  assert_eq!(busy.map_err(|why| why.key), Err("orchestrator.rewind.busy"));
  assert_eq!(conversation.entries(), before, "a busy conversation stays as it was");

  conversation.answers(thinking, "Nothing waits.", &[]);
  let before = conversation.entries();
  let gone = rewind_in(
    conversation.repo.path(),
    conversation.chat,
    RewindPoint::Before(ChatMessageId::generate()),
  );
  assert_eq!(gone.map_err(|why| why.key), Err("orchestrator.rewind.notThere"));
  let last = before.last().expect("an entry").seq;
  let nothing_after = rewind_in(
    conversation.repo.path(),
    conversation.chat,
    RewindPoint::After {
      last,
      answered: Timestamp::now(),
    },
  );
  assert_eq!(
    nothing_after.map_err(|why| why.key),
    Err("orchestrator.rewind.notThere")
  );
  assert_eq!(conversation.entries(), before, "nothing changed");
  assert!(
    conversation
      .db()
      .chat(conversation.chat)
      .expect("read")
      .expect("kept")
      .session()
      .is_some(),
    "and the session is kept"
  );
}
