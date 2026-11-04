//! The `orchestrator` scenario with *Webhook retries*, the conversation of ORC-330's frames, as the one the
//! Orchestrator opens on (`Orchestrator · redesign` `hujxu`, ORC-333, ORC-334):
//! - `orchestrator-rewind`: three answered turns, the second of which drafted KON-31 and started its run and the third
//!   drafted KON-32: hovering the second message shows *Edit*, Copy and `…` (`09` `GYAPg`), *Edit* turns it into a
//!   field (`09b` `QJb0b`), its *Send* and *Rewind to here* ask first (`09c` `z7sgJ`), its *Fork from here* opens the
//!   fork with the message's words in its composer (`11` `RT5T7`);
//! - `orchestrator-rewound`: that conversation rewound to before its second message, which dropped four messages: the
//!   rewound line and what the Orchestrator did after it, which stays (`10` `Zm7kq`);
//! - `orchestrator-forked`: that conversation forked from before its second message, a minute ago, as *Webhook retries
//!   · fork*, which the Orchestrator opens on: the fork's line, the copied turn and what the original made after it
//!   (`11` `RT5T7`; its composer is empty, since a draft lives only while the app runs).

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{ActionOutcome, Chat, ChatAction, RewindPoint, TurnState},
  resource::ModelId,
  time::Timestamp,
};
use orchester_store::{ChatFork, ChatRewind};

use super::orchestrator;
use crate::kontur::{Kontur, SeedError};

/// The model the Orchestrator runs with.
const MODEL: &str = "claude-opus-5-5";
/// The message the rewound scenario goes back to before.
const DRAFT: &str = "Draft a task for per-endpoint retry settings and start it on Quick fix v2.";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  webhook_retries(kontur).map(drop)
}

pub fn rewound(kontur: &mut Kontur) -> Result<(), SeedError> {
  let (chat, draft) = webhook_retries(kontur)?;
  match kontur.db.rewind_chat(chat, RewindPoint::Before(draft), kontur.ago(1))? {
    ChatRewind::Rewound { .. } => Ok(()),
    other => Err(SeedError::Invalid(format!("the rewind went {other:?}"))),
  }
}

pub fn forked(kontur: &mut Kontur) -> Result<(), SeedError> {
  let (chat, draft) = webhook_retries(kontur)?;
  let fork = (ChatId::generate(), "Webhook retries · fork");
  match kontur
    .db
    .fork_chat((chat, RewindPoint::Before(draft)), fork, kontur.ago(1))?
  {
    ChatFork::Forked { .. } => Ok(()),
    other => Err(SeedError::Invalid(format!("the fork went {other:?}"))),
  }
}

/// The `orchestrator` scenario and *Webhook retries*, started 20 minutes ago; it and its message [`DRAFT`].
fn webhook_retries(kontur: &mut Kontur) -> Result<(ChatId, ChatMessageId), SeedError> {
  orchestrator::seed(kontur)?;
  let start = kontur.ago(20);
  let chat = Chat::named(ChatId::generate(), "Webhook retries", start);
  turn(
    kontur,
    &chat,
    ("What's still open on webhook delivery?", start),
    (
      "Two things: retries are one global setting, and failed deliveries have no view. Nothing open covers either.",
      &[],
    ),
  )?;
  let created = |task: &str| ChatAction::new("create_task", Some(task.to_owned()), ActionOutcome::Done);
  let draft = turn(
    kontur,
    &chat,
    (DRAFT, later(start, 3 * 60)),
    (
      "Drafted KON-31 and started it on Quick fix v2.",
      &[
        created("KON-31"),
        ChatAction::new("run_task", Some("KON-31".to_owned()), ActionOutcome::Done),
      ],
    ),
  )?;
  turn(
    kontur,
    &chat,
    ("Add a task for a failed-deliveries view too.", later(start, 11 * 60)),
    (
      "Created KON-32 as a draft. I'll leave it until KON-31 is merged.",
      &[created("KON-32")],
    ),
  )?;
  Ok((chat.id(), draft))
}

/// A turn of `chat` the person started with `text` at `at`, answered with `said` and `actions` and ended a minute
/// later; its message.
fn turn(
  kontur: &mut Kontur,
  chat: &Chat,
  (text, at): (&str, Timestamp),
  (said, actions): (&str, &[ChatAction]),
) -> Result<ChatMessageId, SeedError> {
  let message = ChatMessageId::generate();
  let body = MessageBody::new(text).map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur.db.keep_chat_message(chat, (message, &body), at)?;
  let model = ModelId::new(MODEL).map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur.db.add_chat_reply(chat.id(), (said, &model), later(at, 20))?;
  for action in actions {
    kontur.db.add_chat_action(chat.id(), action, later(at, 30))?;
  }
  kontur.db.end_chat_turn(message, &TurnState::Answered, later(at, 60))?;
  Ok(message)
}

/// `seconds` after `at`.
fn later(at: Timestamp, seconds: i64) -> Timestamp {
  Timestamp::from_unix_millis(at.unix_millis() + seconds * 1_000)
}
