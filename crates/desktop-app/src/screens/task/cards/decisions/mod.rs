//! What the cards of one Conversation tab share with the tab and its composer, kept by the tab so a card that
//! scrolls out of the chat's list and back loses nothing: the answers given and under way ([`answers`]), what
//! the person picked and typed on a question's card, whether the run lost its runner, which card takes the
//! focus when the tab opens, and the composer that takes a question's answer or a plan's changes.

mod answers;

use std::{collections::HashMap, sync::Arc};

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::question::Question;
use orchester_ipc::Command;
use orchester_workspace::{tasks::question::Draft, workspace::OpenProject};
use parking_lot::Mutex;

pub(super) use self::answers::{Given, Outcome, Refused, Stage, refusal};
use self::answers::{Results, SendAnswer, Sending};
use crate::services::Services;

/// Which card takes the focus when the tab opens: the one the chat reveals, once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Focus {
  /// The chat has shown no rows yet.
  #[default]
  Unsettled,
  /// The card with this key, the first that waited for the person, until it rendered.
  On(Arc<str>),
  /// Taken, or nothing waited when the chat first showed its rows.
  Spent,
}

/// What the person picked and typed on a question's card, kept by the tab.
#[derive(Clone)]
pub(super) struct ChoiceDraft {
  pub(super) picked: Signal<Picked>,
  /// Each question's own answer, as typed.
  pub(super) own: Arc<[Signal<String>]>,
}

/// What the person picked on a question's card.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Picked(pub(super) Draft);

impl lurq::app::component::DevtoolsInspectable for Picked {}

/// Made by the Conversation tab when it is created ([`Decisions::new`], before any future of its own),
/// kept in each render ([`Decisions::keep`], first) and provided to its cards. The composer (C2) attaches
/// itself with [`Decisions::attach_composer`] once it is mounted where it answers a question and takes a
/// plan's changes, and moves the focus into its box whenever [`Decisions::composer_focus_asks`] moves on.
#[derive(Clone)]
pub struct Decisions {
  send: SendAnswer,
  results: Signal<Results>,
  drafts: Arc<Mutex<HashMap<Arc<str>, ChoiceDraft>>>,
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
    let results = ctx.signal(Results::default());
    let (clock, recorded) = (Services::of(ctx).clock, results.clone());
    ctx.watch(&send.state(), move |state| {
      answers::record(&recorded, state, clock.now())
    });
    Self {
      send,
      results,
      drafts: Arc::default(),
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
  /// takes the focus when it renders; the first time only.
  pub fn reveal(&self, waiting: Option<Arc<str>>) {
    let mut focus = self.focus.lock();
    if *focus == Focus::Unsettled {
      *focus = waiting.map_or(Focus::Spent, Focus::On);
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
  /// at its first render. Its first render uses the chance up either way, so it never comes later.
  pub(super) fn take_focus(&self, key: &str) -> bool {
    let mut focus = self.focus.lock();
    if matches!(&*focus, Focus::On(on) if &**on == key) {
      *focus = Focus::Spent;
      return true;
    }
    false
  }

  /// An answer is under way: every card holds its answers until it ends.
  pub(super) fn busy(&self) -> bool {
    self.send.state().get().is_pending()
  }

  /// What became of the answer given on the card `key`, if one was.
  pub(super) fn outcome(&self, key: &str) -> Option<Outcome> {
    self.results.with(|results| results.0.get(key).cloned())
  }

  /// Sends `given` as `command` for the card `key` to the runner of `project`, unless an answer is under way
  /// (a second press, or Enter held, must not cancel the first).
  pub(super) fn answer(&self, key: &Arc<str>, given: Given, (project, command): (&OpenProject, Command)) {
    if self.send.state().get_untracked().is_pending() {
      return;
    }
    self.results.update(|results| {
      results.0.insert(
        key.clone(),
        Outcome {
          given,
          stage: Stage::Sending,
        },
      );
    });
    self.send.run(Sending {
      key: key.clone(),
      given,
      project: project.clone(),
      command,
    });
  }

  /// What the person picked and typed on the card `key` of `questions`, kept from its first render.
  pub(super) fn draft(&self, key: &Arc<str>, questions: &[Question]) -> ChoiceDraft {
    self
      .drafts
      .lock()
      .entry(key.clone())
      .or_insert_with(|| ChoiceDraft {
        picked: Signal::new(Picked(Draft::new(questions))),
        own: questions.iter().map(|_| Signal::new(String::new())).collect(),
      })
      .clone()
  }
}
