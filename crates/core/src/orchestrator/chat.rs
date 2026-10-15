//! A conversation of the Orchestrator chat and what it holds: the person's messages, each with the state of the turn
//! it started, the agent's replies, and the actions the agent took through Orchester's tools.

use serde::{Deserialize, Serialize};

use super::proposal::ProposedStart;
use crate::{
  connection::AccountStartProblem,
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId, IdempotencyKey},
  resource::ModelId,
  time::Timestamp,
};

/// Characters a conversation's title keeps of its first message; History lists it by that.
pub const TITLE_CHARS: usize = 80;

/// The agent CLI session a conversation's turns continue, and the connection and account it lives on: a session is
/// kept in the CLI's own folder of the account, so another connection, or the connection on another account, starts
/// a new one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSession {
  pub id: String,
  pub connection: ConnectionId,
  pub account: AgentAccountId,
}

/// Why a turn starts a new session where the conversation had one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionChange {
  /// The Orchestrator runs on another connection now.
  Connection,
  /// Its connection runs on another account now.
  Account,
}

/// Something that happened in a conversation besides what was said and done in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEvent {
  /// The turn after it started a new session: the agent does not remember what was said before.
  NewSession { because: SessionChange },
}

/// One conversation of a project's Orchestrator chat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chat {
  id: ChatId,
  title: String,
  started: Timestamp,
  session: Option<ChatSession>,
}

impl Chat {
  /// A new conversation, named after its first message `first`, sent at `at`.
  pub fn start(id: ChatId, first: &MessageBody, at: Timestamp) -> Self {
    Self {
      id,
      title: title_of(first.as_str()),
      started: at,
      session: None,
    }
  }

  /// A conversation as it was stored.
  pub const fn restore(id: ChatId, title: String, started: Timestamp, session: Option<ChatSession>) -> Self {
    Self {
      id,
      title,
      started,
      session,
    }
  }

  pub const fn id(&self) -> ChatId {
    self.id
  }

  pub fn title(&self) -> &str {
    &self.title
  }

  pub const fn started(&self) -> Timestamp {
    self.started
  }

  pub const fn session(&self) -> Option<&ChatSession> {
    self.session.as_ref()
  }

  /// The session a turn on `connection` and its `account` resumes; `None` starts a new one, as the first turn does
  /// and a turn on another connection or account must.
  pub fn resumes_on(&self, connection: ConnectionId, account: AgentAccountId) -> Option<&str> {
    self
      .session
      .as_ref()
      .filter(|session| session.connection == connection && session.account == account)
      .map(|session| session.id.as_str())
  }

  /// Why a turn on `connection` and its `account` cannot resume the conversation's session; `None` when it has none
  /// yet or resumes it.
  pub fn session_change(&self, connection: ConnectionId, account: AgentAccountId) -> Option<SessionChange> {
    let session = self.session.as_ref()?;
    if session.connection != connection {
      Some(SessionChange::Connection)
    } else if session.account != account {
      Some(SessionChange::Account)
    } else {
      None
    }
  }
}

/// The first line of `text` that says something, cut to [`TITLE_CHARS`] characters with an ellipsis.
pub fn title_of(text: &str) -> String {
  let line = text
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .unwrap_or_default();
  if line.chars().count() <= TITLE_CHARS {
    return line.to_owned();
  }
  let kept: String = line.chars().take(TITLE_CHARS - 1).collect();
  format!("{}…", kept.trim_end())
}

/// Where the turn a person's message started stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnState {
  /// The agent is working on it.
  Thinking,
  /// The agent's turn ended on its own.
  Answered,
  /// The person stopped the answer; what the agent did until then stays.
  Stopped,
  Failed(TurnFailure),
}

impl TurnState {
  /// The word the database keeps; a failure's details are kept beside it.
  pub const fn word(&self) -> &'static str {
    match self {
      Self::Thinking => "thinking",
      Self::Answered => "answered",
      Self::Stopped => "stopped",
      Self::Failed(_) => "failed",
    }
  }

  pub const fn is_thinking(&self) -> bool {
    matches!(self, Self::Thinking)
  }
}

/// Why a turn ended without its answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TurnFailure {
  /// The CLI is not signed in on the connection's account.
  NotSignedIn,
  /// The Orchestrator's connection is gone, or is not one it runs on.
  ConnectionGone,
  /// The connection's account cannot start its CLI.
  Account(AccountStartProblem),
  /// The CLI could not be started, as `reason` tells the runner's log.
  CannotStart { reason: String },
  /// The CLI started and failed: what it or the runner said.
  Failed { reason: String },
  /// The runner ended before the turn did (the app or the runner was closed).
  Interrupted,
  /// The answer ran longer than one answer may ([`super::TIME_LIMIT`], `minutes`), and the runner ended it.
  TimeLimit { minutes: u64 },
}

/// An action the agent took through one of Orchester's tools, and how it went. The task it names, when it names one,
/// is its key as people read it (`KON-41`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatAction {
  pub tool: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub task: Option<String>,
  pub outcome: ActionOutcome,
  /// What a hold on new starts held, when the action held them.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub held: Option<HeldWork>,
}

/// The new starts a hold held: in one reviewed group (its key), or in the whole project; and the keys of the tasks
/// that waited to start then.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldWork {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub group: Option<IdempotencyKey>,
  pub waiting: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionOutcome {
  Done,
  /// The tool refused it with `code`, for `reason`: the words the tool answered the agent with.
  Refused {
    code: String,
    reason: String,
  },
  /// A start the agent proposed: nothing ran. It waits for a person to start it, as it was proposed, or to dismiss
  /// it; once started it is [`Self::Done`] or [`Self::Refused`].
  Proposed(Box<ProposedStart>),
  /// A person dismissed the proposed start: nothing ran.
  Dismissed,
}

impl ActionOutcome {
  /// Whether it waits for a person.
  pub const fn is_proposed(&self) -> bool {
    matches!(self, Self::Proposed { .. })
  }
}

/// What one entry of a conversation is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
  Person {
    message: ChatMessageId,
    text: MessageBody,
    turn: TurnState,
    /// When its turn ended; `None` while it thinks, and for one cut short without a runner to say when.
    ended: Option<Timestamp>,
  },
  Reply {
    text: String,
    model: ModelId,
  },
  Action(ChatAction),
  Event(ChatEvent),
}

/// One entry of a conversation, in the order it was kept (`seq`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatEntry {
  pub seq: i64,
  pub at: Timestamp,
  pub kind: EntryKind,
}
