//! A conversation as turns: each message a person sent, what the Orchestrator answered it with (its replies and
//! the actions it took, in the order they were kept) and how the turn ended; and each wait of an agent of the
//! project and each daily report, with the turn it started by itself, if it started one. What is kept while a turn
//! runs belongs to that turn, even when a wait kept meanwhile shows under it. A message the answer under way took
//! (*Send now*) goes on with that answer: what the answer said after it shows under it, and so does how the answer
//! ended. The messages that wait for the answer to end, queued or handed to it, show after every turn, in order.

use std::{
  collections::{BTreeMap, HashSet},
  sync::Arc,
};

use orchester_core::{
  attachment::{Attachment, is_names_only},
  id::ChatMessageId,
  orchestrator::{AgentWait, ChatAction, ChatEntry, ChatEvent, DailyReport, EntryKind, TurnState, WaitTurn},
  time::Timestamp,
};

/// A part of an answer.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Part {
  Reply {
    text: Arc<str>,
    at: Timestamp,
  },
  Action {
    action: Box<ChatAction>,
    at: Timestamp,
    /// Its entry in the conversation, which a proposed start is decided by.
    seq: i64,
  },
  Event {
    event: ChatEvent,
    at: Timestamp,
  },
}

impl Part {
  pub(super) const fn at(&self) -> Timestamp {
    match self {
      Self::Reply { at, .. } | Self::Action { at, .. } | Self::Event { at, .. } => *at,
    }
  }
}

/// How a person's message reached the answer under way, when it was written while the Orchestrator answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Arrival {
  /// It started a turn of its own, or opens a wait's or a report's row.
  Own,
  /// The answer under way took it (*Send now*): the row goes on with that answer.
  Joined,
  /// It waits for the answer to end, queued or handed to it: its [`Turn::state`] says which.
  Waiting,
}

/// One message and its answer, or one wait of an agent or one daily report and the answer of the turn it started, if
/// any.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Turn {
  /// `None` while it is being sent and not kept yet, and for a wait or a report that started no turn.
  pub(super) message: Option<ChatMessageId>,
  /// The person's words; empty for a wait or a report, and for a message of attachments only.
  pub(super) text: Arc<str>,
  /// The files the person attached to the message.
  pub(super) attachments: Arc<[Attachment]>,
  /// The wait that opens the row instead of a person's message.
  pub(super) wait: Option<Box<AgentWait>>,
  /// Whether that wait still stands: its agent still waits, or its run is still where it ended. Dealt with since,
  /// here or on its task's page, it no longer waits for anyone.
  pub(super) stands: bool,
  /// The daily report that opens the row instead of a person's message.
  pub(super) report: Option<Box<DailyReport>>,
  /// Its entry in the conversation; `None` while it is being sent.
  seq: Option<i64>,
  /// When it was kept; `None` while it is being sent.
  pub(super) at: Option<Timestamp>,
  /// When its turn ended, when the runner said.
  pub(super) ended: Option<Timestamp>,
  /// Where its turn stands; `None` for a wait or a report that started no turn. A message the answer under way took
  /// stands as that answer does; one that waits, as it waits.
  pub(super) state: Option<TurnState>,
  pub(super) arrival: Arrival,
  /// A message the answer under way took follows it: that answer goes on, and ends, under that message.
  pub(super) continued: bool,
  pub(super) answer: Vec<Part>,
}

impl Turn {
  /// A message being sent, which the conversation does not hold yet: one that starts a turn of its own (`state`
  /// thinking), or one written while the Orchestrator answers, as it will wait.
  pub(super) fn sending(text: &str, state: TurnState) -> Self {
    let arrival = if state.has_turn() {
      Arrival::Own
    } else {
      Arrival::Waiting
    };
    Self {
      message: None,
      text: Arc::from(text),
      attachments: Arc::from([]),
      wait: None,
      stands: false,
      report: None,
      seq: None,
      at: None,
      ended: None,
      state: Some(state),
      arrival,
      continued: false,
      answer: Vec::new(),
    }
  }

  /// Whether its turn, if any, was one Orchester started by itself: a wait's or a report's, with no words of the
  /// person's to send again.
  pub(super) const fn on_its_own(&self) -> bool {
    self.wait.is_some() || self.report.is_some()
  }

  /// Whether what the Orchestrator says next belongs to this row: one with a turn of its own, or a message the answer
  /// under way took.
  fn takes_parts(&self) -> bool {
    self.arrival == Arrival::Joined || (self.arrival == Arrival::Own && self.state.is_some())
  }

  /// The row of `entry`, which opens with a wait or a report (the caller sets which), and the turn it started, if any.
  fn opened(entry: &ChatEntry, turn: Option<&WaitTurn>) -> Self {
    Self {
      message: turn.map(|turn| turn.message),
      text: Arc::from(""),
      attachments: Arc::from([]),
      wait: None,
      stands: false,
      report: None,
      seq: Some(entry.seq),
      at: Some(entry.at),
      ended: turn.and_then(|turn| turn.ended),
      state: turn.map(|turn| turn.state.clone()),
      arrival: Arrival::Own,
      continued: false,
      answer: Vec::new(),
    }
  }

  /// Whether the answer did something through a tool.
  pub(super) fn acted(&self) -> bool {
    self.answer.iter().any(|part| {
      matches!(
        part,
        Part::Action { action, .. }
          if matches!(**action, ChatAction { outcome: orchester_core::orchestrator::ActionOutcome::Done, .. })
      )
    })
  }

  /// The row's key: its message, its wait's or report's entry, or *sending* until it is kept.
  pub(super) fn key(&self) -> String {
    match (self.message, self.seq) {
      (Some(message), _) => message.to_string(),
      (None, Some(seq)) => format!("wait-{seq}"),
      (None, None) => "sending".to_owned(),
    }
  }
}

/// The turns of `entries`, with the files attached to the person's messages (`attached`), oldest first, and after them
/// the messages that wait for the answer to end; the waits whose entries are among `standing` still stand. What comes
/// before the first message or wait belongs to no turn and is left out; a reply, action or event belongs to the newest
/// row that takes it.
pub(super) fn turns(
  entries: &[ChatEntry],
  attached: &BTreeMap<ChatMessageId, Vec<Attachment>>,
  standing: &HashSet<i64>,
) -> Vec<Turn> {
  let mut turns: Vec<Turn> = Vec::new();
  let mut waiting: Vec<Turn> = Vec::new();
  for entry in entries {
    let part = match &entry.kind {
      EntryKind::Person {
        message,
        text,
        turn,
        ended,
        ..
      } => {
        let row = person(entry, (*message, text.as_str(), turn), *ended, attached);
        if row.arrival == Arrival::Waiting {
          waiting.push(row);
        } else {
          turns.push(row);
        }
        continue;
      }
      EntryKind::Wait { wait, turn } => {
        turns.push(Turn {
          wait: Some(Box::new(wait.clone())),
          stands: standing.contains(&entry.seq),
          ..Turn::opened(entry, turn.as_ref())
        });
        continue;
      }
      EntryKind::Report { report, turn } => {
        turns.push(Turn {
          report: Some(report.clone()),
          ..Turn::opened(entry, turn.as_ref())
        });
        continue;
      }
      EntryKind::Reply { text, .. } => Part::Reply {
        text: Arc::from(text.as_str()),
        at: entry.at,
      },
      EntryKind::Action(action) => Part::Action {
        action: Box::new(action.clone()),
        at: entry.at,
        seq: entry.seq,
      },
      EntryKind::Event(event) => Part::Event {
        event: event.clone(),
        at: entry.at,
      },
    };
    if let Some(turn) = turns.iter_mut().rev().find(|turn| turn.takes_parts()) {
      turn.answer.push(part);
    }
  }
  join_answers(&mut turns);
  turns.extend(waiting);
  turns
}

/// The row of the person's message `message`, kept as `entry`, whose turn stands at `turn` and ended at `ended`.
fn person(
  entry: &ChatEntry,
  (message, text, turn): (ChatMessageId, &str, &TurnState),
  ended: Option<Timestamp>,
  attached: &BTreeMap<ChatMessageId, Vec<Attachment>>,
) -> Turn {
  let attachments: Arc<[Attachment]> = attached
    .get(&message)
    .map_or_else(|| Arc::from([]), |files| files.as_slice().into());
  // A message of attachments only was kept with their names: it shows by its files alone.
  let words = if is_names_only(text, &attachments) { "" } else { text };
  let arrival = match turn {
    TurnState::Joined => Arrival::Joined,
    _ if turn.is_waiting() => Arrival::Waiting,
    _ => Arrival::Own,
  };
  Turn {
    message: Some(message),
    text: Arc::from(words),
    attachments,
    wait: None,
    stands: false,
    report: None,
    seq: Some(entry.seq),
    at: Some(entry.at),
    ended,
    state: Some(turn.clone()),
    arrival,
    continued: false,
    answer: Vec::new(),
  }
}

/// A message the answer under way took stands as that answer does, and the row before it goes on under it.
fn join_answers(turns: &mut [Turn]) {
  let mut answering: Option<(Option<TurnState>, Option<Timestamp>)> = None;
  for index in 0..turns.len() {
    match turns[index].arrival {
      Arrival::Joined => {
        if let Some((state, ended)) = answering.clone() {
          turns[index].state = state;
          turns[index].ended = ended;
        }
        if let Some(before) = turns[..index].iter_mut().rev().find(|turn| turn.takes_parts()) {
          before.continued = true;
        }
      }
      Arrival::Own if turns[index].state.is_some() => {
        answering = Some((turns[index].state.clone(), turns[index].ended));
      }
      Arrival::Own | Arrival::Waiting => {}
    }
  }
}
