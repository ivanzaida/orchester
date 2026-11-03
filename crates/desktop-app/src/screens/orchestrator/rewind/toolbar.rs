//! What the toolbars of the conversation's messages offer besides Copy (09 `GYAPg`, 09d `Pe6Di`): the person's own
//! message *Edit* and `…`, an answer `…`, whose menu holds *Rewind to here*. While the Orchestrator answers, compacts
//! its session or a message waits, *Edit* and `…` are at half and say *Stop the answer to edit or rewind*; an answer
//! nothing follows cannot be rewound to and its row says so.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::orchestrator::{ChatEvent, EntryKind, RewindPoint, TurnMode};

use super::{Editing, Rewinding};
use crate::{
  screens::orchestrator::turns::{Arrival, Turn},
  ui::{
    chat::toolbar::{MessageExtras, MoreActions, MoreRow, ToolbarChoice},
    icon::Icon,
  },
};

/// What the rows of the conversation need for Edit and *Rewind to here*.
#[derive(Clone)]
pub(in crate::screens::orchestrator) struct RewindEnv {
  pub(super) state: Rewinding,
  /// How an edit is sent: its turn's mode and the *Plan only* switch.
  pub(super) mode: (TurnMode, bool),
  /// Why nothing can be edited or rewound now; `None` while it can.
  held: Option<Arc<str>>,
  /// The message edited in place, and how many messages sending drops after it.
  editing: Option<(Editing, u32)>,
}

impl RewindEnv {
  /// The rows' part of `state`, an edit sent as `mode` says; nothing can be edited or rewound while `busy` (a message
  /// is being sent) or while the conversation held is.
  pub(in crate::screens::orchestrator) fn new(
    ctx: &Ctx,
    state: &Rewinding,
    mode: (TurnMode, bool),
    busy: bool,
  ) -> Self {
    let editing = state.editing.get().map(|editing| (editing, state.after(editing.seq)));
    let held = (busy || state.busy()).then(|| ctx.t("orchestrator.message.busy"));
    Self {
      state: state.clone(),
      mode,
      held,
      editing,
    }
  }

  /// What a row shows of it, which its comparison counts.
  pub(in crate::screens::orchestrator) fn shows(&self) -> (Option<&Arc<str>>, Option<(Editing, u32)>) {
    (self.held.as_ref(), self.editing)
  }

  /// The message being edited in place, and how many messages sending drops after it.
  pub(in crate::screens::orchestrator) const fn editing(&self) -> Option<(Editing, u32)> {
    self.editing
  }

  /// Whether `turn` comes after the message being edited: sending the edit drops it, so it is dimmed.
  pub(in crate::screens::orchestrator) fn drops(&self, turn: &Turn) -> bool {
    self
      .editing
      .zip(turn.seq())
      .is_some_and(|((editing, _), seq)| seq > editing.seq)
  }

  /// *Edit* and `…` of the person's message that opens `turn`, once it is kept and has its turn, or the answer under
  /// way took it; `None` for one being sent or that waits.
  pub(in crate::screens::orchestrator) fn on_message(&self, ctx: &Ctx, turn: &Turn) -> Option<MessageExtras> {
    let (message, seq, at) = (turn.message?, turn.seq()?, turn.at?);
    if turn.arrival == Arrival::Waiting || turn.on_its_own() {
      return None;
    }
    let editing = Editing { message, seq, at };
    let choose = |act: Arc<dyn Fn() + Send + Sync>| match &self.held {
      Some(why) => Err(why.clone()),
      None => Ok(act),
    };
    let (state, words) = (self.state.clone(), turn.text.clone());
    let edit = ToolbarChoice {
      label: ctx.t("orchestrator.message.edit"),
      choose: choose(Arc::new(move || state.edit(editing, &words))),
    };
    let state = self.state.clone();
    let rewind = choose(Arc::new(move || state.ask(RewindPoint::Before(message), seq, at)));
    Some(MessageExtras {
      edit: Some(edit),
      more: Some(self.more(ctx, rewind)),
    })
  }

  /// `…` of the answer of `turn`, which rewinds to just after its last part; `None` before it said anything.
  pub(in crate::screens::orchestrator) fn on_answer(&self, ctx: &Ctx, turn: &Turn) -> Option<MessageExtras> {
    let (last, answered) = (turn.last()?, turn.answer.first()?.at());
    let state = self.state.clone();
    let rewind: Arc<dyn Fn() + Send + Sync> =
      Arc::new(move || state.ask(RewindPoint::After { last, answered }, last + 1, answered));
    let rewind = match &self.held {
      Some(why) => Err(why.clone()),
      None if !self.state.follows(last) => Err(ctx.t("orchestrator.message.nothingAfter")),
      None => Ok(rewind),
    };
    Some(MessageExtras {
      edit: None,
      more: Some(self.more(ctx, rewind)),
    })
  }

  fn more(&self, ctx: &Ctx, rewind: crate::ui::chat::toolbar::Choice) -> MoreActions {
    MoreActions {
      label: ctx.t("orchestrator.message.more"),
      rows: vec![MoreRow {
        key: "rewind",
        label: ctx.t("orchestrator.message.rewind"),
        icon: Icon::History,
        choose: rewind,
      }],
      held: self.held.clone(),
    }
  }
}

impl Rewinding {
  /// Whether the conversation held is busy: the Orchestrator answers in it, compacts its session, or a message waits
  /// there for its turn.
  pub(super) fn busy(&self) -> bool {
    let held = self.held.lock();
    let Some((_, entries)) = held.as_ref() else {
      return false;
    };
    entries.iter().any(|entry| match &entry.kind {
      EntryKind::Person { turn, .. } => turn.is_thinking() || turn.is_waiting(),
      EntryKind::Event(event) => event.is_compacting(),
      kind => kind.turn().is_some_and(|(_, turn)| turn.is_thinking()),
    })
  }

  /// Whether any message of the conversation held follows entry `seq`.
  fn follows(&self, seq: i64) -> bool {
    let held = self.held.lock();
    held.as_ref().is_some_and(|(_, entries)| {
      entries
        .iter()
        .any(|entry| entry.seq > seq && !matches!(entry.kind, EntryKind::Event(ChatEvent::NewSession { .. })))
    })
  }
}
