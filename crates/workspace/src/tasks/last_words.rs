//! What the agent of a step that waits for a message wrote last (ORC-42), so a wait is never unreadable: its newest
//! message in the task's conversation (the question it ended its step with, or a reply), or else the newest message in
//! its transcript, whichever is newer. Both are stored with secrets already redacted, as the Conversation tab shows
//! them. Reading blocks on SQLite.

use std::sync::Arc;

use orchester_core::{
  conversation::{AgentMessage, Message, Sender},
  id::StepExecutionId,
  run::{Run, StepResult},
  store::{ActivityStore, ConversationStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::transcript::{Entry, EntryKind, entries};

/// What a waiting step's agent wrote last, and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastWords {
  pub at: Timestamp,
  pub text: Arc<str>,
  /// It is the question the agent ended its step with.
  pub question: bool,
}

/// What the agent of `execution` wrote last among the task's `messages` and the run's `transcript`; `None` when it
/// wrote nothing either holds.
pub fn last_words(messages: &[Message], transcript: &[Entry], execution: StepExecutionId) -> Option<LastWords> {
  let said = messages.iter().rev().find_map(|message| match message.sender() {
    Sender::Agent {
      execution: wrote, kind, ..
    } if *wrote == execution => Some(LastWords {
      at: message.created_at(),
      text: message.body().as_str().into(),
      question: *kind == AgentMessage::Question,
    }),
    _ => None,
  });
  let streamed = transcript.iter().rev().find_map(|entry| match &entry.kind {
    EntryKind::Message(text) if entry.execution == execution && StepResult::parse(text).is_none() => Some(LastWords {
      at: entry.at,
      text: text.clone(),
      question: false,
    }),
    _ => None,
  });
  match (said, streamed) {
    (Some(said), Some(streamed)) if streamed.at > said.at => Some(streamed),
    (said, streamed) => said.or(streamed),
  }
}

/// Each step of `run` that waits for a message, in pipeline order, with what its agent wrote last (`None` when it
/// wrote nothing that can be read); the transcript is read only when a step waits.
pub fn waiting_words(db: &ProjectDb, run: &Run) -> Result<Vec<(StepExecutionId, Option<LastWords>)>, StoreError> {
  let waiting = run.awaiting_answer();
  if waiting.is_empty() {
    return Ok(Vec::new());
  }
  let messages = db.messages(run.task())?;
  let transcript = entries(&db.activity(run.id(), 0)?);
  Ok(
    waiting
      .into_iter()
      .map(|execution| (execution, last_words(&messages, &transcript, execution)))
      .collect(),
  )
}
