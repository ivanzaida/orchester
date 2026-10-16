//! What the cards of one Conversation tab share with the tab and its composer: the answers given
//! ([`answers`]), sent by the tab one at a time; what the person picked and typed on a question's card and
//! what became of each answer, kept by run and card for as long as the app runs ([`kept`]), so switching
//! tabs, leaving the task or a card scrolling out of the chat loses neither; whether the run lost its
//! runner; and which card takes the focus when the tab opens.

mod answers;
mod kept;

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::{id::RunId, question::Question, run::Answerer};
use orchester_ipc::Command;
use orchester_workspace::{tasks::question::Draft, workspace::OpenProject};
use parking_lot::Mutex;

pub(super) use self::answers::{Given, Outcome, Refused, Stage, refusal};
use self::answers::{SendAnswer, Sending};
use crate::mcp::input_gate::{self, Guarded, InputGate};

/// How long after the chat first shows its rows the card it reveals may still take the focus: the frames
/// it takes the chat to scroll to it and lay it out. A card that has not shown by then (the person scrolled
/// first, or it sits in a folded step) does not take it later.
const FOCUS_WINDOW: Duration = Duration::from_millis(1500);

/// Which card takes the focus when the tab opens: the one the chat reveals, once, soon.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Focus {
  /// The chat has shown no rows yet.
  #[default]
  Unsettled,
  /// The card with this key, the first that waited for the person, if it shows before `until`.
  On { key: Arc<str>, until: Instant },
  /// Taken, missed, or nothing waited when the chat first showed its rows.
  Spent,
}

/// A card's place among the kept: its run and its key in the run's timeline.
pub(super) type Card<'a> = (RunId, &'a Arc<str>);

/// Made by the Conversation tab when it is created ([`Decisions::new`], before any future of its own),
/// kept in each render ([`Decisions::keep`], first) and provided to its cards.
#[derive(Clone)]
pub struct Decisions {
  send: SendAnswer,
  /// Moves on whenever what is kept changes, so the cards that read it show it.
  kept: Signal<u64>,
  lost: Signal<bool>,
  focus: Arc<Mutex<Focus>>,
  /// Tells a person's input from an MCP client's, and whether the person lets automation answer.
  gate: Option<InputGate>,
}

impl PartialEq for Decisions {
  /// One tab's, for as long as it is mounted.
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.focus, &other.focus)
  }
}

impl lurq::app::component::DevtoolsInspectable for Decisions {}

impl Decisions {
  /// A tab's, made when it is created, before any future of its own.
  pub fn new(ctx: &mut Ctx) -> Self {
    let send = answers::action(ctx);
    let kept = ctx.signal(0);
    let ended = kept.clone();
    // An answer that ended was kept where it ended: the cards read it again.
    ctx.watch(&send.state(), move |state| {
      if !state.is_pending() {
        ended.update(|kept| *kept += 1);
      }
    });
    Self {
      send,
      kept,
      lost: ctx.signal(false),
      focus: Arc::default(),
      gate: ctx.use_context::<InputGate>(),
    }
  }

  /// Whether the run lost its runner (it is interrupted until one looks after it): then no card waits.
  pub fn follow_runner(&self, lost: bool) {
    if self.lost.get_untracked() != lost {
      self.lost.set(lost);
    }
  }

  /// The chat shows its rows: `waiting`, the first card that waits for the person and that the chat reveals,
  /// takes the focus if it shows soon; the first time only.
  pub fn reveal(&self, waiting: Option<Arc<str>>) {
    let mut focus = self.focus.lock();
    match &*focus {
      Focus::Unsettled => {
        *focus = waiting.map_or(Focus::Spent, |key| Focus::On {
          key,
          until: Instant::now() + FOCUS_WINDOW,
        });
      }
      Focus::On { until, .. } if Instant::now() > *until => *focus = Focus::Spent,
      Focus::On { .. } | Focus::Spent => {}
    }
  }

  /// The tab's, which it provides before its cards.
  pub(super) fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the Conversation tab provides its decisions before its cards")
  }

  pub(super) fn lost(&self) -> bool {
    self.lost.get()
  }

  /// Whether the card `key` takes the focus now: the card the chat revealed when it first showed its rows,
  /// at its first render, soon. Its first render uses the chance up either way.
  pub(super) fn take_focus(&self, key: &str) -> bool {
    let mut focus = self.focus.lock();
    let Focus::On { key: on, until } = &*focus else {
      return false;
    };
    if &**on != key {
      return false;
    }
    let soon = Instant::now() <= *until;
    *focus = Focus::Spent;
    soon
  }

  /// An answer of this tab is on its way: the others wait until it arrives.
  pub(super) fn busy(&self) -> bool {
    self.send.state().get().is_pending()
  }

  /// What became of the answer given on `card`, if one was.
  pub(super) fn outcome(&self, card: Card<'_>) -> Option<Outcome> {
    self.kept.get();
    kept::outcome(card)
  }

  /// Sends `given` for `card` to the runner of `project`, as the command `command` makes of it for who gives it,
  /// unless an answer is on its way (a second press, or Enter held, must not cancel the first). The person
  /// gives what their own input gives. An MCP client's input answers only where automation may through
  /// Orchester's MCP server, under the person's consent ([`input_gate::answerer`]), and never more widely than
  /// once: the answer is then automation's, else the card says that only a person gives it here.
  pub(super) fn answer(
    &self,
    (run, key): Card<'_>,
    given: Given,
    (project, command): (&OpenProject, impl FnOnce(Answerer) -> Command),
  ) {
    if self.send.state().get_untracked().is_pending() {
      return;
    }
    let wider = matches!(given, Given::ApproveSimilar | Given::ApproveEverything);
    let by = input_gate::answerer(self.gate.as_ref(), Guarded::ApprovalWidening);
    let (by, stage) = match by {
      Some(Answerer::Automation) if wider => (Answerer::Automation, Stage::Refused(Refused::WiderPersonOnly)),
      Some(by) => (by, Stage::Sending),
      None => (Answerer::Automation, Stage::Refused(Refused::PersonOnly)),
    };
    let refused = stage != Stage::Sending;
    if refused {
      tracing::warn!("synthetic input gave an answer only a person gives here; not taken");
    }
    kept::set_outcome((run, key), Outcome { given, by, stage });
    self.kept.update(|kept| *kept += 1);
    if refused {
      return;
    }
    self.send.run(Sending {
      run,
      key: key.clone(),
      given,
      by,
      project: project.clone(),
      command: command(by),
    });
  }

  /// What the person picked and typed on `card`, asking `questions`.
  pub(super) fn draft(&self, card: Card<'_>, questions: &[Question]) -> Draft {
    self.kept.get();
    kept::draft(card, questions)
  }

  /// Changes what is picked and typed on `card` with `change`.
  pub(super) fn change_draft(&self, card: Card<'_>, questions: &[Question], change: impl FnOnce(&mut Draft)) {
    kept::change_draft(card, questions, change);
    self.kept.update(|kept| *kept += 1);
  }
}
