//! A conversation of the Orchestrator chat and what it holds: the person's messages, each with the state of the turn
//! it started, the agent's replies, the actions the agent took through Orchester's tools, and the waits of the
//! project's agents, each with the turn it started by itself, if any.

use serde::{Deserialize, Serialize};

use super::{change::ProposedChange, proposal::ProposedStart, report::DailyReport, wait::AgentWait};
use crate::{
  connection::AccountStartProblem,
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId, IdempotencyKey, RunId},
  resource::ModelId,
  run::Answerer,
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
    Self::named(id, first.as_str(), at)
  }

  /// A new conversation named after `title`, started at `at`: the one a wait of an agent opens in a project that has
  /// none yet.
  pub fn named(id: ChatId, title: &str, at: Timestamp) -> Self {
    Self {
      id,
      title: title_of(title),
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

/// Where the turn a person's message started stands, or, for a message written while another turn of its conversation
/// was under way, how it reaches the agent (`docs/design/orchestrator-toolset.md`, "Writing while it answers").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnState {
  /// The agent is working on it.
  Thinking,
  /// The agent's turn ended on its own.
  Answered,
  /// The person stopped the answer; what the agent did until then stays.
  Stopped,
  Failed(TurnFailure),
  /// It waits for the turn under way in its conversation to end; its own turn starts then, as [`Queued`] orders.
  Queued(Queued),
  /// Sent now (*Send now*): handed to the turn under way, which takes it at its next tool call.
  Steering,
  /// The turn under way took it: that turn answers it, and it starts none of its own.
  Joined,
}

/// Why a person's message waits for the turn under way, which decides when its own turn starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queued {
  /// *Send after this answer*: its turn starts once the answer under way ended, after the messages queued before it.
  AfterAnswer,
  /// *Stop and send*: the answer under way is stopped, and its turn starts before any other message queued.
  Next,
  /// Sent now, but the turn under way did not take it (its CLI refused it, or the answer ended first): its turn starts
  /// after the answer, as [`Self::AfterAnswer`] does, and the conversation says why.
  NotTaken,
}

impl TurnState {
  /// The word the database keeps; a failure's details are kept beside it.
  pub const fn word(&self) -> &'static str {
    match self {
      Self::Thinking => "thinking",
      Self::Answered => "answered",
      Self::Stopped => "stopped",
      Self::Failed(_) => "failed",
      Self::Queued(Queued::AfterAnswer) => "queued",
      Self::Queued(Queued::Next) => "queued_next",
      Self::Queued(Queued::NotTaken) => "not_taken",
      Self::Steering => "steering",
      Self::Joined => "joined",
    }
  }

  pub const fn is_thinking(&self) -> bool {
    matches!(self, Self::Thinking)
  }

  /// Whether it still waits for a turn of its own: queued, or handed to the turn under way and not taken yet. Until
  /// then the person may change or remove a queued message.
  pub const fn is_waiting(&self) -> bool {
    matches!(self, Self::Queued(_) | Self::Steering)
  }

  /// Whether the message started a turn of its own: one that waits, or that the turn under way took, has none.
  pub const fn has_turn(&self) -> bool {
    !matches!(self, Self::Queued(_) | Self::Steering | Self::Joined)
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
  /// What a publication, cancel or merge acts on, whether it waits for a person, was settled or was done by the
  /// chat itself: its card reads its facts from it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub change: Option<Box<ProposedChange>>,
  /// The run a start the chat took itself started, or that its message that ran the task again started.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub started: Option<RunId>,
  /// The run a resume the chat did itself resumed.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub resumed: Option<RunId>,
  /// The chat acted without asking because auto mode is on for the project.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub auto_mode: bool,
  /// The chat merged a run whose success started its turn, in a turn nobody asked for, because the person lets the
  /// Orchestrator merge finished runs on its own (ORC-69): *merged by the Orchestrator on its own*.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub on_its_own: bool,
  /// When a person settled the proposal: started, applied or dismissed it, or it was refused then.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub settled: Option<Timestamp>,
  /// When a person's decision claimed the proposal, which is being carried out until it is settled: it still shows
  /// what it proposes, and no other decision is taken on it. A claim made before the app's session started has no
  /// work behind it any more and is recovered (`orchester_workspace::orchestrator::recover`).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub claimed: Option<Timestamp>,
  /// The person decided it by saying so in the chat (ORC-96): the message of theirs that approved it, which the chat
  /// applied it for, rather than with a click on its card.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub approved_in: Option<ChatApproval>,
}

impl ChatAction {
  /// Whether it waits for a person's decision: a proposal no decision took yet.
  pub const fn waits(&self) -> bool {
    self.outcome.is_proposed() && self.claimed.is_none()
  }

  /// Tool `tool`'s action on task `task`, gone as `outcome`, about nothing more.
  pub fn new(tool: &str, task: Option<String>, outcome: ActionOutcome) -> Self {
    Self {
      tool: tool.to_owned(),
      task,
      outcome,
      held: None,
      change: None,
      started: None,
      resumed: None,
      auto_mode: false,
      on_its_own: false,
      settled: None,
      claimed: None,
      approved_in: None,
    }
  }
}

/// The person's message in the chat that approved a proposal: its id, its entry in the conversation (`seq`) and its
/// words as its card quotes them ([`title_of`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatApproval {
  pub message: ChatMessageId,
  pub seq: i64,
  pub words: String,
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
  /// A publication, cancel or merge the agent proposed ([`ChatAction::change`]): nothing changed. It waits for a
  /// person to apply it, as it was proposed, or to dismiss it; once applied it is [`Self::Applied`] or
  /// [`Self::Refused`].
  Waits,
  /// A person applied the proposed change.
  Applied,
  /// A person dismissed the proposal: nothing ran or changed.
  Dismissed,
}

impl ActionOutcome {
  /// Whether it is a proposal, waiting for a person or being carried out ([`ChatAction::claimed`]).
  pub const fn is_proposed(&self) -> bool {
    matches!(self, Self::Proposed(_) | Self::Waits)
  }

  /// Whether what it names was carried out: by the chat itself, or applied by a person.
  pub const fn answered(&self) -> bool {
    matches!(self, Self::Done | Self::Applied)
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
    /// Who sent it: the person, or automation driving the window (lurq's synthetic input), whose message never
    /// approves a proposal for the person (ORC-96).
    sent_by: Answerer,
  },
  Reply {
    text: String,
    model: ModelId,
  },
  Action(ChatAction),
  Event(ChatEvent),
  /// An agent of the project waits ([`AgentWait`]), and the turn it started by itself, once it started one.
  Wait {
    wait: AgentWait,
    turn: Option<WaitTurn>,
  },
  /// The project's report of a day ([`DailyReport`]), and the turn it started by itself to add a summary, once it
  /// started one.
  Report {
    report: Box<DailyReport>,
    turn: Option<WaitTurn>,
  },
}

/// The Orchestrator turn a wait or a daily report started by itself: under the id its prompt was kept by, where it
/// stands and when it ended, as a person's message's turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitTurn {
  pub message: ChatMessageId,
  pub state: TurnState,
  pub ended: Option<Timestamp>,
}

impl EntryKind {
  /// The turn this entry started, and where it stands: a person's message's, or a wait's or a report's that started
  /// one. A person's message that waits for the turn under way, or that turn took, started none.
  pub fn turn(&self) -> Option<(ChatMessageId, &TurnState)> {
    match self {
      Self::Person { message, turn, .. } if turn.has_turn() => Some((*message, turn)),
      Self::Wait {
        turn: Some(started), ..
      }
      | Self::Report {
        turn: Some(started), ..
      } => Some((started.message, &started.state)),
      _ => None,
    }
  }
}

/// One entry of a conversation, in the order it was kept (`seq`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatEntry {
  pub seq: i64,
  pub at: Timestamp,
  pub kind: EntryKind,
}
