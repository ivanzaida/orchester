//! What the actions of the conversation's messages offer besides Copy (`06 · Message actions` `XPDAX`, ORC-706): the
//! person's own message *Edit*, *Rewind to here* and *Fork from here* (ORC-334), an answer the last two. While the
//! Orchestrator answers, compacts its session or a message waits, *Edit* and *Rewind to here* are at half and say
//! *Stop the answer to edit or rewind*, while *Fork from here* still forks what was kept so far; an answer nothing
//! follows cannot be rewound to and its *Rewind to here* says so. No second fork starts while one is being made.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::orchestrator::{ChatEvent, EntryKind, RewindPoint, TurnMode};
use orchester_workspace::workspace::OpenProject;

use super::{Editing, Rewinding};
use crate::{
  screens::orchestrator::turns::{Arrival, Turn},
  ui::chat::toolbar::{Choice, MessageExtras, ToolbarChoice},
};

/// Why nothing can be edited or rewound now, and why no fork can be made now; `None` for each that can.
pub(in crate::screens::orchestrator) type Reasons<'a> = (Option<&'a Arc<str>>, Option<&'a Arc<str>>);

/// What the rows of the conversation need for Edit, *Rewind to here* and *Fork from here*.
#[derive(Clone)]
pub(in crate::screens::orchestrator) struct RewindEnv {
  pub(super) state: Rewinding,
  /// The project a fork is kept in.
  project: OpenProject,
  /// How an edit is sent: its turn's mode and the *Plan only* switch.
  pub(super) mode: (TurnMode, bool),
  /// Why nothing can be edited or rewound now; `None` while it can.
  held: Option<Arc<str>>,
  /// Why no fork can be made now (one is being made); `None` while it can.
  forking: Option<Arc<str>>,
  /// The message edited in place, and how many messages sending drops after it.
  editing: Option<(Editing, u32)>,
}

impl RewindEnv {
  /// The rows' part of `state` in `project`, an edit sent as `mode` says; nothing can be edited or rewound while
  /// `busy` (a message is being sent) or while the conversation held is.
  pub(in crate::screens::orchestrator) fn new(
    ctx: &Ctx,
    (state, project): (&Rewinding, &OpenProject),
    mode: (TurnMode, bool),
    busy: bool,
  ) -> Self {
    let editing = state.editing.get().map(|editing| (editing, state.after(editing.seq)));
    let held = (busy || state.busy()).then(|| ctx.t("orchestrator.message.busy"));
    let forking = state
      .fork
      .state()
      .get()
      .is_pending()
      .then(|| ctx.t("orchestrator.fork.making"));
    Self {
      state: state.clone(),
      project: project.clone(),
      mode,
      held,
      forking,
      editing,
    }
  }

  /// What a row shows of it, which its comparison counts: why nothing can be edited or rewound, or forked, now, and
  /// the message edited in place.
  pub(in crate::screens::orchestrator) fn shows(&self) -> (Reasons<'_>, Option<(Editing, u32)>) {
    ((self.held.as_ref(), self.forking.as_ref()), self.editing)
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

  /// *Edit*, *Rewind to here* and *Fork from here* of the person's message that opens `turn`, once it is kept and has
  /// its turn, or the answer under way took it; `None` for one being sent or that waits.
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
    Some(self.extras(ctx, Some(edit), rewind, RewindPoint::Before(message)))
  }

  /// *Rewind to here* and *Fork from here* of the answer of `turn`, which rewind to just after its last part, or fork
  /// up to it; `None` before it said anything.
  pub(in crate::screens::orchestrator) fn on_answer(&self, ctx: &Ctx, turn: &Turn) -> Option<MessageExtras> {
    let (last, answered) = (turn.last()?, turn.answer.first()?.at());
    let point = RewindPoint::After { last, answered };
    let state = self.state.clone();
    let rewind: Arc<dyn Fn() + Send + Sync> = Arc::new(move || state.ask(point, last + 1, answered));
    let rewind = match &self.held {
      Some(why) => Err(why.clone()),
      None if !self.state.follows(last) => Err(ctx.t("orchestrator.message.nothingAfter")),
      None => Ok(rewind),
    };
    Some(self.extras(ctx, None, rewind, point))
  }

  /// `edit`, *Rewind to here* as `rewind` says, and *Fork from here* at `point`.
  fn extras(&self, ctx: &Ctx, edit: Option<ToolbarChoice>, rewind: Choice, point: RewindPoint) -> MessageExtras {
    let (state, project) = (self.state.clone(), self.project.clone());
    let fork: Choice = match &self.forking {
      Some(why) => Err(why.clone()),
      None => Ok(Arc::new(move || state.fork(&project, point))),
    };
    MessageExtras {
      edit,
      rewind: Some(ToolbarChoice {
        label: ctx.t("orchestrator.message.rewind"),
        choose: rewind,
      }),
      fork: Some(ToolbarChoice {
        label: ctx.t("orchestrator.message.fork"),
        choose: fork,
      }),
    }
  }
}

impl Rewinding {
  /// Whether the conversation held is busy: the Orchestrator answers in it, compacts its session, or a message waits
  /// there for its turn.
  pub(super) fn busy(&self) -> bool {
    let held = self.held.lock();
    let Some(holding) = held.as_ref() else {
      return false;
    };
    holding.entries.iter().any(|entry| match &entry.kind {
      EntryKind::Person { turn, .. } => turn.is_thinking() || turn.is_waiting(),
      EntryKind::Event(event) => event.is_compacting(),
      kind => kind.turn().is_some_and(|(_, turn)| turn.is_thinking()),
    })
  }

  /// Whether any message of the conversation held follows entry `seq`.
  fn follows(&self, seq: i64) -> bool {
    let held = self.held.lock();
    held.as_ref().is_some_and(|holding| {
      holding
        .entries
        .iter()
        .any(|entry| entry.seq > seq && !matches!(entry.kind, EntryKind::Event(ChatEvent::NewSession { .. })))
    })
  }
}
