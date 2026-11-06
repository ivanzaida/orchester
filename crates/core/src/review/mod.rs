//! A person's review of a run that waits for it before it merges (*Ready for review* on the task page, or
//! `WaitingForApproval` in the merge queue; `crates/desktop-app/design/orchester.pen`, board `Review · redesign`, 09i
//! `wLyFt` and the thread and tray states of `RBR9O`). A thread is anchored to lines of the run's diff at the head the
//! person read, and is a *Comment*, which asks the implementing agent to change the code, or a *Question*, which asks
//! it to answer without changing any. It holds messages in order: the person's, Implement's answers and words, and the
//! person's follow-ups (*Ask again* on a question, *Reply* on an addressed comment). A question goes to Implement at
//! once; comments wait unsent until *Send to Implement* sends every one of them in one return of the run's work.

mod prompt;
mod state;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  prompt::{question_text, send_note},
  state::{ThreadState, Viewed},
};
use crate::{
  conversation::MessageBody,
  id::{ReviewMessageId, ReviewThreadId, RunId},
  orchestrator::TurnFailure,
  pull_request::feedback::LineAnchor,
  revision::Revision,
  time::Timestamp,
};

/// Why a change of a thread is not accepted.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ReviewError {
  #[error("a thread starts with the person's message")]
  NoMessages,
  #[error("the message is not in the thread")]
  UnknownMessage,
  #[error("only a comment not sent yet changes or goes")]
  NotEditable,
  #[error("a question is asked again once its answer ended")]
  NotAnswered,
  #[error("a reply follows a comment Implement addressed")]
  NotAddressed,
  #[error("the thread asks the other kind of thing")]
  WrongKind,
}

/// What a thread asks of the implementing agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadKind {
  /// Change the code.
  Comment,
  /// Answer without changing code.
  Question,
}

impl ThreadKind {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Comment => "comment",
      Self::Question => "question",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    [Self::Comment, Self::Question]
      .into_iter()
      .find(|kind| kind.as_str() == value)
  }
}

/// Why a question was not asked, a comment not sent, or an answer ended without its words: Orchester's reason, which
/// the thread keeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum ReviewFailure {
  /// No runner of the project took the request: none runs, or it did not come online.
  RunnerUnavailable,
  /// The request went to the runner and its answer was lost: whether it was taken is not known.
  Lost,
  /// The runner refused the request; `reason` is its refusal's code.
  Refused { reason: String },
  /// The run does not wait for review any more.
  NotWaiting,
  /// The run has no step whose agent may write in its worktree: there is no Implement to ask.
  NoImplementingStep,
  /// Implement runs on Orchester's native agent, which answers no review question in this version.
  NativeAgent,
  /// The answer's turn ended without its words, as the runner tells it.
  Turn(TurnFailure),
}

/// How asking a question went.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum QuestionState {
  /// Asked; nothing from Implement yet.
  Asked,
  /// Implement is answering: its words and live activity stream into the answer after it.
  Answering,
  /// Answered; no code changed.
  Answered,
  /// The person stopped the answer; what was written stays.
  Stopped,
  /// *Not asked*, with Orchester's reason.
  NotAsked { failure: ReviewFailure },
}

/// How sending a comment went.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CommentState {
  /// Waits in the tray for *Send to Implement*; editable and deletable. `failure`: the last send did not go, and why.
  NotSent { failure: Option<ReviewFailure> },
  /// *Send to Implement* is under way since `since`, while the run had `attempts` attempts: a later one returned to
  /// Implement tells that the runner took it, when the hand-off was cut off before its answer.
  Sending { since: Timestamp, attempts: usize },
  /// Sent at `at`: Implement works on it.
  Sent { at: Timestamp },
  /// Implement's work after it ended: `commit` changed the comment's lines (`None`: none did), with Implement's words
  /// in the message after it.
  Addressed { commit: Option<Revision> },
}

/// Implement's words in a thread: an answer to the question before them, or what it says about the comment it
/// addressed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWords {
  /// Its turn still writes them.
  pub writing: bool,
  /// What it does now, while it writes: the latest tool call it made.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub activity: Option<String>,
}

/// Who wrote a message, and how it went.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum Said {
  Question(QuestionState),
  Comment(CommentState),
  Agent(AgentWords),
}

impl Said {
  /// The person wrote it.
  pub const fn is_person(&self) -> bool {
    !matches!(self, Self::Agent(_))
  }
}

/// One message of a thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewMessage {
  id: ReviewMessageId,
  said: Said,
  /// The words; Implement's may be empty while it starts writing.
  body: String,
  at: Timestamp,
}

impl ReviewMessage {
  /// The person's new message of a thread of `kind`: a question asked, or a comment that waits unsent.
  pub fn person(kind: ThreadKind, body: &MessageBody, at: Timestamp) -> Self {
    let said = match kind {
      ThreadKind::Question => Said::Question(QuestionState::Asked),
      ThreadKind::Comment => Said::Comment(CommentState::NotSent { failure: None }),
    };
    Self::restore(ReviewMessageId::generate(), said, body.as_str().to_owned(), at)
  }

  /// Implement's words, `writing` while its turn goes on.
  pub fn agent(body: String, writing: bool, at: Timestamp) -> Self {
    let said = Said::Agent(AgentWords {
      writing,
      activity: None,
    });
    Self::restore(ReviewMessageId::generate(), said, body, at)
  }

  /// A stored message.
  pub const fn restore(id: ReviewMessageId, said: Said, body: String, at: Timestamp) -> Self {
    Self { id, said, body, at }
  }

  pub const fn id(&self) -> ReviewMessageId {
    self.id
  }

  pub const fn said(&self) -> &Said {
    &self.said
  }

  pub fn body(&self) -> &str {
    &self.body
  }

  pub const fn at(&self) -> Timestamp {
    self.at
  }
}

/// What a thread is about, checked by [`ReviewThread::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadParts {
  pub id: ReviewThreadId,
  pub run: RunId,
  pub kind: ThreadKind,
  /// The lines of the run's diff the thread is about, with the code they held at `head`.
  pub anchor: LineAnchor,
  /// The run's head the person read when they wrote it.
  pub head: Revision,
  pub created_at: Timestamp,
  pub resolved_at: Option<Timestamp>,
}

/// A thread of a person's review of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewThread {
  parts: ThreadParts,
  messages: Vec<ReviewMessage>,
}

impl ReviewThread {
  /// A thread whose first message, `messages[0]`, is the person's.
  pub fn new(parts: ThreadParts, messages: Vec<ReviewMessage>) -> Result<Self, ReviewError> {
    if !messages.first().is_some_and(|first| first.said.is_person()) {
      return Err(ReviewError::NoMessages);
    }
    Ok(Self { parts, messages })
  }

  /// A new thread on `run` of `kind`, about `anchor` at `head`, that starts with the person's `body` at `now`.
  pub fn start(
    run: RunId,
    (kind, anchor, head): (ThreadKind, LineAnchor, Revision),
    body: &MessageBody,
    now: Timestamp,
  ) -> Self {
    let parts = ThreadParts {
      id: ReviewThreadId::generate(),
      run,
      kind,
      anchor,
      head,
      created_at: now,
      resolved_at: None,
    };
    Self {
      parts,
      messages: vec![ReviewMessage::person(kind, body, now)],
    }
  }

  pub const fn id(&self) -> ReviewThreadId {
    self.parts.id
  }

  pub const fn run(&self) -> RunId {
    self.parts.run
  }

  pub const fn kind(&self) -> ThreadKind {
    self.parts.kind
  }

  pub const fn anchor(&self) -> &LineAnchor {
    &self.parts.anchor
  }

  pub const fn head(&self) -> &Revision {
    &self.parts.head
  }

  pub const fn created_at(&self) -> Timestamp {
    self.parts.created_at
  }

  pub const fn resolved_at(&self) -> Option<Timestamp> {
    self.parts.resolved_at
  }

  pub fn messages(&self) -> &[ReviewMessage] {
    &self.messages
  }

  pub fn message(&self, id: ReviewMessageId) -> Option<&ReviewMessage> {
    self.messages.iter().find(|message| message.id == id)
  }

  /// The person's latest message: what the thread asks now.
  pub fn latest_ask(&self) -> &ReviewMessage {
    self
      .messages
      .iter()
      .rev()
      .find(|message| message.said.is_person())
      .expect("a thread starts with the person's message")
  }

  /// Whether the person may change or delete message `id` now: a comment not sent yet.
  pub fn editable(&self, id: ReviewMessageId) -> Result<&ReviewMessage, ReviewError> {
    let message = self.message(id).ok_or(ReviewError::UnknownMessage)?;
    match message.said {
      Said::Comment(CommentState::NotSent { .. }) => Ok(message),
      _ => Err(ReviewError::NotEditable),
    }
  }

  /// The person's next message, `body` at `now`: *Ask again* after a question's answer ended, or *Reply* on a comment
  /// Implement addressed, which waits unsent as any comment. Gives the message, to be kept after the others.
  pub fn follow_up(&self, body: &MessageBody, now: Timestamp) -> Result<ReviewMessage, ReviewError> {
    match (&self.latest_ask().said, self.kind()) {
      (
        Said::Question(QuestionState::Answered | QuestionState::Stopped | QuestionState::NotAsked { .. }),
        ThreadKind::Question,
      )
      | (Said::Comment(CommentState::Addressed { .. }), ThreadKind::Comment) => {
        Ok(ReviewMessage::person(self.kind(), body, now))
      }
      (Said::Question(_), ThreadKind::Question) => Err(ReviewError::NotAnswered),
      (Said::Comment(_), ThreadKind::Comment) => Err(ReviewError::NotAddressed),
      _ => Err(ReviewError::WrongKind),
    }
  }

  /// The comments of the thread that wait in the tray: not sent yet.
  pub fn unsent(&self) -> impl Iterator<Item = &ReviewMessage> {
    self
      .messages
      .iter()
      .filter(|message| matches!(message.said, Said::Comment(CommentState::NotSent { .. })))
  }

  /// The comment of the thread Implement works on: sent, not addressed yet.
  pub fn sent(&self) -> Option<&ReviewMessage> {
    self
      .messages
      .iter()
      .find(|message| matches!(message.said, Said::Comment(CommentState::Sent { .. })))
  }

  /// The question whose answer is asked for or under way.
  pub fn asking(&self) -> Option<&ReviewMessage> {
    self.messages.iter().find(|message| {
      matches!(
        message.said,
        Said::Question(QuestionState::Asked | QuestionState::Answering)
      )
    })
  }
}
