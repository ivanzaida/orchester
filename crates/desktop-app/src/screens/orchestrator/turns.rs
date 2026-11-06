//! A conversation as turns: each message a person sent, what the Orchestrator answered it with (its replies, the
//! actions it took and the plans it answered with in plan-only mode, in the order they were kept) and how the turn
//! ended; and each wait of an agent of the project and each daily report, with the turn it started by itself, if it
//! started one. What is kept while a turn runs belongs to that turn, even when a wait kept meanwhile shows under it. A
//! message the answer under way took (*Send now*) goes on with that answer: what the answer said after it shows under
//! it, and so does how the answer ended. The turn that carries out an approved plan has no message of the person's to
//! show: it goes on under the plan it carries out (`12c` `j7Oa9v`). The messages that wait for the answer to end,
//! queued or handed to it, show after every turn, in order.

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  attachment::{Attachment, is_names_only},
  connection::stream::TodoList,
  id::ChatMessageId,
  orchestrator::{
    AgentWait, ChatAction, ChatEntry, ChatEvent, ChatPlan, DailyReport, EntryKind, PlanState, TurnMode, TurnState,
    WaitTurn, watch::WatchSummary,
  },
  time::Timestamp,
};
use orchester_workspace::orchestrator::{
  chat::{ChatView, EndMoved, EventFacts},
  waits_on::view::RunWaitView,
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
  /// A plan, its entry, and which plan of the conversation it is, from 1.
  Plan {
    plan: Box<ChatPlan>,
    at: Timestamp,
    seq: i64,
    number: usize,
  },
}

impl Part {
  pub(super) const fn at(&self) -> Timestamp {
    match self {
      Self::Reply { at, .. } | Self::Action { at, .. } | Self::Event { at, .. } | Self::Plan { at, .. } => *at,
    }
  }

  /// Whether it is a plan the person approved at `at`.
  fn approved_at(&self, at: Timestamp) -> bool {
    matches!(self, Self::Plan { plan, .. } if plan.state() == PlanState::Approved { at })
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
  /// The id a message being sent is kept under: its row keeps its key, and so its element, once it is kept.
  sending: Option<ChatMessageId>,
  /// The person's words; empty for a wait or a report, and for a message of attachments only.
  pub(super) text: Arc<str>,
  /// The files the person attached to the message.
  pub(super) attachments: Arc<[Attachment]>,
  /// The wait that opens the row instead of a person's message.
  pub(super) wait: Option<Box<AgentWait>>,
  /// Whether that wait still stands: its agent still waits, or its run is still where it ended. Dealt with since,
  /// here or on its task's page, it no longer waits for anyone.
  pub(super) stands: bool,
  /// What became of the run of a run's end that no longer stands.
  pub(super) moved: Option<EndMoved>,
  /// What the line of an event of the wait's run names beside the wait.
  pub(super) event: Option<EventFacts>,
  /// The daily report that opens the row instead of a person's message.
  pub(super) report: Option<Box<DailyReport>>,
  /// The end of a subscription that ended by itself, whose line opens the row instead of a person's message.
  pub(super) watch_ended: Option<Box<WatchSummary>>,
  /// An event that is the whole row: a compaction of the session (`super::compaction`), or where a rewind went back
  /// to (`super::rewind::line`).
  pub(super) compaction: Option<ChatEvent>,
  /// Its entry in the conversation; `None` while it is being sent.
  seq: Option<i64>,
  /// The entry of the last part of its answer; `None` before its answer said or did anything.
  last: Option<i64>,
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
  /// How the waits its answer's *waits on* rows name stand now (ORC-784).
  pub(super) waits_on: Vec<RunWaitView>,
  /// The todo list of the turn it started, as the agent's latest update left it; `None` when the agent keeps none.
  pub(super) todo: Option<TodoList>,
  /// How the person's message's turn went: it planned only, or it acted.
  pub(super) mode: TurnMode,
}

impl Turn {
  /// Message `id` being sent, which the conversation does not hold yet: one that starts a turn of its own (`state`
  /// thinking), or one written while the Orchestrator answers, as it will wait.
  pub(super) fn sending(id: ChatMessageId, text: &str, state: TurnState) -> Self {
    let arrival = if state.has_turn() {
      Arrival::Own
    } else {
      Arrival::Waiting
    };
    Self {
      message: None,
      sending: Some(id),
      text: Arc::from(text),
      attachments: Arc::from([]),
      wait: None,
      stands: false,
      moved: None,
      event: None,
      report: None,
      watch_ended: None,
      compaction: None,
      seq: None,
      last: None,
      at: None,
      ended: None,
      state: Some(state),
      arrival,
      continued: false,
      answer: Vec::new(),
      waits_on: Vec::new(),
      todo: None,
      mode: TurnMode::Acts,
    }
  }

  /// Whether its turn, if any, was one Orchester started by itself: a wait's, a report's or a subscription's end's,
  /// with no words of the person's to send again.
  pub(super) const fn on_its_own(&self) -> bool {
    self.wait.is_some() || self.report.is_some() || self.watch_ended.is_some()
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
      sending: None,
      text: Arc::from(""),
      attachments: Arc::from([]),
      wait: None,
      stands: false,
      moved: None,
      event: None,
      report: None,
      watch_ended: None,
      compaction: None,
      seq: Some(entry.seq),
      last: None,
      at: Some(entry.at),
      ended: turn.and_then(|turn| turn.ended),
      state: turn.map(|turn| turn.state.clone()),
      arrival: Arrival::Own,
      continued: false,
      answer: Vec::new(),
      waits_on: Vec::new(),
      todo: None,
      mode: TurnMode::Acts,
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

  /// The row's key: its message, the one it is being sent as, or its wait's or report's entry.
  pub(super) fn key(&self) -> String {
    match (self.message.or(self.sending), self.seq) {
      (Some(message), _) => message.to_string(),
      (None, Some(seq)) => format!("wait-{seq}"),
      (None, None) => "sending".to_owned(),
    }
  }

  /// Its entry in the conversation; `None` while it is being sent.
  pub(super) const fn seq(&self) -> Option<i64> {
    self.seq
  }

  /// The entry of the last part of its answer; `None` before its answer said or did anything.
  pub(super) const fn last(&self) -> Option<i64> {
    self.last
  }
}

/// The key of the row of `turns` that shows entry `seq`: the row it opens, or the latest row before it whose answer
/// it is part of; a message that waits shows only itself.
pub(super) fn row_of(turns: &[Turn], seq: i64) -> Option<String> {
  turns
    .iter()
    .filter(|turn| {
      turn
        .seq
        .is_some_and(|at| at == seq || (at < seq && turn.arrival != Arrival::Waiting))
    })
    .max_by_key(|turn| turn.seq)
    .map(Turn::key)
}

/// The turns of conversation `shown`, oldest first, and after them the messages that wait for the answer to end; its
/// waits stand as it read them. What comes before the first message or wait belongs to no turn and is left out; a
/// reply, action, event or plan belongs to the newest row that takes it.
pub(super) fn turns(shown: &ChatView) -> Vec<Turn> {
  let mut turns: Vec<Turn> = Vec::new();
  let mut waiting: Vec<Turn> = Vec::new();
  let mut plans = 0;
  for entry in &shown.entries {
    if let Some(row) = own_row(entry, shown) {
      turns.push(row);
      continue;
    }
    let part = match &entry.kind {
      EntryKind::Person { .. } => {
        add_person((&mut turns, &mut waiting), entry, shown);
        continue;
      }
      EntryKind::Wait { .. } | EntryKind::Report { .. } | EntryKind::WatchEnded { .. } => continue,
      EntryKind::Plan(plan) => {
        plans += 1;
        Part::Plan {
          plan: plan.clone(),
          at: entry.at,
          seq: entry.seq,
          number: plans,
        }
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
      turn.last = Some(entry.seq);
    }
  }
  join_answers(&mut turns);
  // A turn's list is the one its agent keeps for the message, wait or report that started it.
  for turn in &mut turns {
    turn.todo = turn
      .message
      .filter(|_| turn.arrival == Arrival::Own)
      .and_then(|message| shown.todo_of(message).cloned());
    turn.waits_on = turn
      .answer
      .iter()
      .filter_map(|part| match part {
        Part::Action { action, .. } => action.waits_on.as_ref(),
        _ => None,
      })
      .filter_map(|row| shown.wait_of(row.run).cloned())
      .collect();
  }
  turns.extend(waiting);
  turns
}

/// The row `entry` of `shown` opens by itself: a wait of an agent, a daily report or a subscription that ended by
/// itself, with the turn it started, if any; and a compaction (`06b` `j7eSQO`), where a rewind went back to (`10`
/// `Zm7kq`), a fork's first line and what its original made after it (`11` `RT5T7`), each a row between the turns.
/// `None` for an entry that is a person's message or part of an answer.
fn own_row(entry: &ChatEntry, shown: &ChatView) -> Option<Turn> {
  Some(match &entry.kind {
    EntryKind::Wait { wait, turn } => Turn {
      wait: Some(Box::new(wait.clone())),
      stands: shown.standing.contains(&entry.seq),
      moved: shown.moved_of(entry.seq),
      event: shown.event_of(entry.seq).cloned(),
      ..Turn::opened(entry, turn.as_ref())
    },
    EntryKind::Report { report, turn } => Turn {
      report: Some(report.clone()),
      ..Turn::opened(entry, turn.as_ref())
    },
    EntryKind::WatchEnded { summary, turn } => Turn {
      watch_ended: Some(summary.clone()),
      ..Turn::opened(entry, turn.as_ref())
    },
    EntryKind::Event(
      event @ (ChatEvent::Compaction { .. }
      | ChatEvent::Rewound { .. }
      | ChatEvent::Forked { .. }
      | ChatEvent::MadeInOriginal { .. }),
    ) => Turn {
      compaction: Some(event.clone()),
      ..Turn::opened(entry, None)
    },
    _ => return None,
  })
}

/// The row of the person's message kept as `entry` of `shown`, among the `turns` or those `waiting`; the turn that
/// carries out an approved plan goes on under that plan instead.
fn add_person((turns, waiting): (&mut Vec<Turn>, &mut Vec<Turn>), entry: &ChatEntry, shown: &ChatView) {
  let EntryKind::Person {
    message,
    text,
    turn,
    ended,
    mode,
    ..
  } = &entry.kind
  else {
    return;
  };
  if *mode == TurnMode::CarriesOut {
    carry_on(turns, entry.at, (turn, *ended));
    return;
  }
  let row = Turn {
    mode: *mode,
    ..person(entry, (*message, text.as_str(), turn), *ended, &shown.attachments)
  };
  if row.arrival == Arrival::Waiting {
    waiting.push(row);
  } else {
    turns.push(row);
  }
}

/// The turn that carries out the plan approved at `at`, which stands at `turn` and ended at `ended`, goes on in the row
/// holding that plan, or else in the newest row that takes parts: what it does shows under the plan, and its end is
/// that row's.
fn carry_on(turns: &mut [Turn], at: Timestamp, (turn, ended): (&TurnState, Option<Timestamp>)) {
  let holder = turns
    .iter()
    .rposition(|row| row.answer.iter().any(|part| part.approved_at(at)))
    .or_else(|| turns.iter().rposition(Turn::takes_parts));
  if let Some(row) = holder.map(|index| &mut turns[index]) {
    row.state = Some(turn.clone());
    row.ended = ended;
  }
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
    sending: None,
    text: Arc::from(words),
    attachments,
    wait: None,
    stands: false,
    moved: None,
    event: None,
    report: None,
    watch_ended: None,
    compaction: None,
    seq: Some(entry.seq),
    last: None,
    at: Some(entry.at),
    ended,
    state: Some(turn.clone()),
    arrival,
    continued: false,
    answer: Vec::new(),
    waits_on: Vec::new(),
    todo: None,
    mode: TurnMode::Acts,
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
