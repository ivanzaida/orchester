//! *Fork from here* in the Orchestrator chat (ORC-334): a fork is a new conversation holding copies of the messages up
//! to its point, opened by its line and, when the original did something after the point, closed by what stays there;
//! the original stays exactly as it was. The fork has no session: its first turn is told the copied conversation and
//! nothing the original said or did after the point. It may be made while the Orchestrator answers, from what was kept
//! so far; the attachments of its messages are shared with the original's, not written again.

use std::fs;

use orchester_core::{
  attachment::{AttachmentName, NewAttachment},
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId, ProjectId},
  name::Name,
  orchestrator::{ActionOutcome, Chat, ChatAction, ChatEntry, ChatEvent, EntryKind, RewindPoint, TurnState},
  project::{Project, ProjectKey},
  resource::ModelId,
  run::Answerer,
  time::Timestamp,
};
use orchester_store::{AttachmentOwner, ProjectDb};
use orchester_workspace::orchestrator::fork::{Forked, fork_in};
use tempfile::TempDir;

const TITLE: &str = "Webhook retries · fork";

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

  /// The person says `text` in conversation `chat` with `attachments`, and its turn thinks.
  fn says_in(&self, chat: ChatId, text: &str, attachments: &[NewAttachment]) -> ChatMessageId {
    let mut db = self.db();
    let body = MessageBody::new(text).expect("text");
    let kept = db
      .chat(chat)
      .expect("read")
      .unwrap_or_else(|| Chat::start(chat, &body, Timestamp::now()));
    let message = ChatMessageId::generate();
    assert!(
      db.keep_chat_message_with(
        &kept,
        (message, &body),
        (attachments, Answerer::Person),
        Timestamp::now()
      )
      .expect("kept")
    );
    message
  }

  fn says(&self, text: &str) -> ChatMessageId {
    self.says_in(self.chat, text, &[])
  }

  /// The Orchestrator replies `text` with `actions` in the conversation, on its session.
  fn replies(&self, text: &str, actions: &[ChatAction]) {
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
  }

  /// The Orchestrator answers `message` with `text` and `actions`, and the turn ends.
  fn answers(&self, message: ChatMessageId, text: &str, actions: &[ChatAction]) {
    self.replies(text, actions);
    self
      .db()
      .end_chat_turn(message, &TurnState::Answered, Timestamp::now())
      .expect("ended");
  }

  fn entries_of(&self, chat: ChatId) -> Vec<ChatEntry> {
    self.db().chat_entries(chat).expect("entries")
  }

  /// Forks the conversation at `point`.
  fn fork(&self, point: RewindPoint) -> Forked {
    fork_in(self.repo.path(), (self.chat, point), TITLE).expect("forked")
  }
}

/// What a conversation's person and Orchestrator said, in order.
fn said(entries: &[ChatEntry]) -> Vec<String> {
  entries
    .iter()
    .filter_map(|entry| match &entry.kind {
      EntryKind::Person { text, .. } => Some(text.as_str().to_owned()),
      EntryKind::Reply { text, .. } => Some(text.clone()),
      _ => None,
    })
    .collect()
}

fn created(task: &str) -> ChatAction {
  ChatAction::new("create_task", Some(task.to_owned()), ActionOutcome::Done)
}

/// Two turns, the second of which created a task.
fn two_turns(conversation: &Conversation) -> (ChatMessageId, ChatMessageId) {
  let first = conversation.says("What's still open on webhook delivery?");
  conversation.answers(first, "Two things: retries and failed deliveries.", &[]);
  let second = conversation.says("Draft a task for per-endpoint retry settings.");
  conversation.answers(second, "Drafted KON-31.", &[created("KON-31")]);
  (first, second)
}

#[test]
fn a_fork_before_a_message_copies_what_came_before_it_and_leaves_the_original_as_it_was() {
  let conversation = Conversation::new();
  let (_, second) = two_turns(&conversation);
  let original = conversation.entries_of(conversation.chat);
  let at = original
    .iter()
    .find(|entry| matches!(&entry.kind, EntryKind::Person { message, .. } if *message == second))
    .expect("the second message")
    .at;

  let forked = conversation.fork(RewindPoint::Before(second));

  assert_eq!(
    forked.words.as_deref(),
    Some("Draft a task for per-endpoint retry settings."),
    "its words go to the fork's composer"
  );
  assert_eq!(
    conversation.entries_of(conversation.chat),
    original,
    "the original stays exactly as it was"
  );
  let db = conversation.db();
  assert!(
    db.chat(conversation.chat)
      .expect("read")
      .expect("kept")
      .session()
      .is_some(),
    "and keeps its session"
  );
  let fork = db.chat(forked.chat).expect("read").expect("the fork is kept");
  assert_eq!(fork.title(), TITLE);
  assert_eq!(fork.session(), None, "the fork starts a session of its own");
  let entries = conversation.entries_of(forked.chat);
  assert_eq!(
    entries.first().map(|entry| &entry.kind),
    Some(&EntryKind::Event(ChatEvent::Forked {
      from: conversation.chat,
      title: "What's still open on webhook delivery?".to_owned(),
      at,
    })),
    "it opens with where it came from"
  );
  assert_eq!(
    said(&entries),
    [
      "What's still open on webhook delivery?",
      "Two things: retries and failed deliveries."
    ]
  );
  assert_eq!(
    entries.last().map(|entry| &entry.kind),
    Some(&EntryKind::Event(ChatEvent::MadeInOriginal {
      made: vec![created("KON-31")],
    })),
    "what the original made after the point stays there, and the fork says so"
  );
  let copied = entries
    .iter()
    .find_map(|entry| match &entry.kind {
      EntryKind::Person { message, .. } => Some(*message),
      _ => None,
    })
    .expect("the copied message");
  assert!(
    !original
      .iter()
      .any(|entry| matches!(&entry.kind, EntryKind::Person { message, .. } if *message == copied)),
    "a copy has an id of its own"
  );
  let listed = db.chats().expect("History");
  assert_eq!(listed[0].chat.id(), forked.chat, "History lists the fork first");
  assert!(listed.iter().any(|listing| listing.chat.id() == conversation.chat));
}

#[test]
fn a_fork_after_an_answer_copies_it_and_what_came_before() {
  let conversation = Conversation::new();
  two_turns(&conversation);
  let last = conversation
    .entries_of(conversation.chat)
    .into_iter()
    .rfind(|entry| matches!(entry.kind, EntryKind::Action(_)))
    .expect("the second answer's last part");

  let forked = conversation.fork(RewindPoint::After {
    last: last.seq,
    answered: last.at,
  });

  assert_eq!(forked.words, None, "nothing goes to the composer");
  let entries = conversation.entries_of(forked.chat);
  assert_eq!(said(&entries).len(), 4, "every message up to the answer");
  assert!(
    entries
      .iter()
      .any(|entry| entry.kind == EntryKind::Action(created("KON-31"))),
    "with what the answer did"
  );
  assert!(
    !entries
      .iter()
      .any(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::MadeInOriginal { .. }))),
    "nothing was made after it"
  );
}

#[test]
fn the_forks_first_turn_is_told_the_copied_conversation_and_not_what_came_after() {
  let conversation = Conversation::new();
  let (_, second) = two_turns(&conversation);
  let forked = conversation.fork(RewindPoint::Before(second));
  let next = conversation.says_in(forked.chat, "Draft it for the failed deliveries view instead.", &[]);

  let prompt = conversation.db().chat_turn(next).expect("read").expect("kept").text;

  let prompt = prompt.as_str();
  assert!(prompt.contains("forked"), "{prompt}");
  assert!(prompt.contains("What's still open on webhook delivery?"), "{prompt}");
  assert!(
    prompt.contains("Two things: retries and failed deliveries."),
    "{prompt}"
  );
  assert!(
    prompt.ends_with("Draft it for the failed deliveries view instead."),
    "{prompt}"
  );
  assert!(
    !prompt.contains("per-endpoint") && !prompt.contains("KON-31"),
    "what the original said and did after the point never reaches it: {prompt}"
  );
  let original = conversation.says("Thanks.");
  assert_eq!(
    conversation
      .db()
      .chat_turn(original)
      .expect("read")
      .expect("kept")
      .text
      .as_str(),
    "Thanks.",
    "the original resumes its own session, told nothing"
  );
}

#[test]
fn a_fork_while_the_orchestrator_answers_takes_what_was_kept_so_far() {
  let conversation = Conversation::new();
  let first = conversation.says("What's still open on webhook delivery?");
  conversation.replies("Looking at the webhook tasks…", &[]);
  let partial = conversation
    .entries_of(conversation.chat)
    .into_iter()
    .rfind(|entry| matches!(entry.kind, EntryKind::Reply { .. }))
    .expect("what it said so far");

  let forked = conversation.fork(RewindPoint::After {
    last: partial.seq,
    answered: partial.at,
  });

  let entries = conversation.entries_of(forked.chat);
  assert_eq!(
    said(&entries),
    [
      "What's still open on webhook delivery?",
      "Looking at the webhook tasks…"
    ]
  );
  assert!(
    entries.iter().any(|entry| matches!(
      &entry.kind,
      EntryKind::Person {
        turn: TurnState::Stopped,
        ..
      }
    )),
    "the copy of the turn under way stopped where the fork was made: {entries:?}"
  );
  assert!(
    conversation
      .db()
      .chat_turn(first)
      .expect("read")
      .expect("kept")
      .turn
      .is_thinking(),
    "the original's answer goes on"
  );
  let next = conversation.says_in(forked.chat, "Go on.", &[]);
  assert!(
    conversation
      .db()
      .chat_turn(next)
      .expect("read")
      .expect("kept")
      .turn
      .is_thinking(),
    "the fork is not busy: it takes a message of its own"
  );
}

#[test]
fn a_fork_shares_the_attachments_of_its_messages_with_the_original() {
  let conversation = Conversation::new();
  let file = NewAttachment::new(
    AttachmentName::new("retries.txt").expect("name"),
    b"Retry each webhook 5 times.".to_vec(),
  )
  .expect("attachment");
  let first = conversation.says_in(conversation.chat, "Read this.", &[file]);
  conversation.answers(first, "Read it.", &[]);
  let second = conversation.says("And now?");
  conversation.answers(second, "Nothing more.", &[]);

  let forked = conversation.fork(RewindPoint::Before(second));

  let db = conversation.db();
  let copy = conversation
    .entries_of(forked.chat)
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Person { message, .. } => Some(message),
      _ => None,
    })
    .expect("the copied message");
  let theirs = db
    .attached_files(AttachmentOwner::ChatMessage(first))
    .expect("the original's");
  let ours = db
    .attached_files(AttachmentOwner::ChatMessage(copy))
    .expect("the fork's");
  assert_eq!(ours.len(), 1);
  assert_ne!(
    ours[0].attachment.id(),
    theirs[0].attachment.id(),
    "each message has its own row"
  );
  assert_eq!(ours[0].attachment.hash(), theirs[0].attachment.hash());
  assert_eq!(ours[0].attachment.name(), theirs[0].attachment.name());
  drop(db);

  conversation
    .db()
    .delete_chat(conversation.chat)
    .expect("the original is deleted");
  assert_eq!(
    fs::read(&ours[0].path).expect("the fork's copy is still there"),
    b"Retry each webhook 5 times."
  );
}

#[test]
fn a_fork_from_a_point_or_a_conversation_not_kept_keeps_nothing() {
  let conversation = Conversation::new();
  two_turns(&conversation);
  let before = conversation.db().chats().expect("History").len();

  let not_there = fork_in(
    conversation.repo.path(),
    (conversation.chat, RewindPoint::Before(ChatMessageId::generate())),
    TITLE,
  );
  assert_eq!(not_there.map_err(|why| why.key), Err("orchestrator.fork.notThere"));
  let gone = fork_in(
    conversation.repo.path(),
    (ChatId::generate(), RewindPoint::Before(ChatMessageId::generate())),
    TITLE,
  );
  assert_eq!(gone.map_err(|why| why.key), Err("orchestrator.conversation.gone"));
  assert_eq!(
    conversation.db().chats().expect("History").len(),
    before,
    "no fork was kept"
  );
}
