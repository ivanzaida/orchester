//! What the cards of one Conversation tab share with the tab and its composer: the answers given
//! ([`answers`]), sent by the tab one at a time; what the person picked and typed on a question's card and
//! what became of each answer, kept by run and card for as long as the app runs ([`kept`]), so switching
//! tabs, leaving the task or a card scrolling out of the chat loses neither; whether the run lost its
//! runner; which card takes the focus when the tab opens; and the composer that takes a question's answer
//! or a plan's changes.

mod answers;
mod kept;

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::{id::RunId, question::Question};
use orchester_ipc::Command;
use orchester_workspace::{tasks::question::Draft, workspace::OpenProject};
use parking_lot::Mutex;

pub(super) use self::answers::{Given, Outcome, Refused, Stage, refusal};
use self::answers::{SendAnswer, Sending};

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
/// kept in each render ([`Decisions::keep`], first) and provided to its cards. The composer (C2) attaches
/// itself with [`Decisions::attach_composer`] once it is mounted where it answers a question and takes a
/// plan's changes, and moves the focus into its box whenever [`Decisions::composer_focus_asks`] moves on.
#[derive(Clone)]
pub struct Decisions {
  send: SendAnswer,
  /// Moves on whenever what is kept changes, so the cards that read it show it.
  kept: Signal<u64>,
  lost: Signal<bool>,
  focus: Arc<Mutex<Focus>>,
  composer: Signal<bool>,
  composer_focus: Signal<u64>,
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
      composer: ctx.signal(false),
      composer_focus: ctx.signal(0),
    }
  }

  /// Keeps the answers with the tab: lurq keeps a component's futures by the order it asks for them in each
  /// render, so the tab asks for this first, as it made it.
  pub fn keep(&self, ctx: &mut Ctx) {
    answers::action(ctx);
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

  /// The composer is mounted and takes a question's answer and a plan's changes: the cards send the person
  /// there.
  #[expect(
    dead_code,
    reason = "the composer (C2, built beside the cards) calls it once it is mounted"
  )]
  pub fn attach_composer(&self) {
    if !self.composer.get_untracked() {
      self.composer.set(true);
    }
  }

  /// How often a card asked the composer to take the focus; the composer reads it as it renders and moves
  /// the focus into its box each time it moves on.
  #[expect(
    dead_code,
    reason = "the composer (C2, built beside the cards) reads it as it renders"
  )]
  pub fn composer_focus_asks(&self) -> u64 {
    self.composer_focus.get()
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

  /// Whether the composer takes free answers here.
  pub(super) fn composer_attached(&self) -> bool {
    self.composer.get()
  }

  /// Asks the composer to take the focus.
  pub(super) fn ask_composer(&self) {
    self.composer_focus.update(|asks| *asks += 1);
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

  /// Sends `given` as `command` for `card` to the runner of `project`, unless an answer is on its way (a
  /// second press, or Enter held, must not cancel the first).
  pub(super) fn answer(&self, (run, key): Card<'_>, given: Given, (project, command): (&OpenProject, Command)) {
    if self.send.state().get_untracked().is_pending() {
      return;
    }
    kept::set_outcome(
      (run, key),
      Outcome {
        given,
        stage: Stage::Sending,
      },
    );
    self.kept.update(|kept| *kept += 1);
    self.send.run(Sending {
      run,
      key: key.clone(),
      given,
      project: project.clone(),
      command,
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
