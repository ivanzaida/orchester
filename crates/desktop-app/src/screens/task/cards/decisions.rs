//! What the cards of one Conversation tab share with the tab and its composer: the composer that takes a
//! question's answer or a plan's changes, and which card takes the focus when the tab opens.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::time::Timestamp;
use parking_lot::Mutex;

use crate::services::Services;

/// Provided by the Conversation tab to its cards ([`Decisions::new`] once, then `ctx.provide` on each
/// render). The composer (C2) attaches itself with [`Decisions::attach_composer`] once it is mounted where it
/// answers a question and takes a plan's changes, and moves the focus into its box whenever
/// [`Decisions::composer_focus_asks`] moves on.
#[derive(Clone)]
pub struct Decisions {
  /// When the tab opened: a card that waited by then takes the focus.
  opened_at: Timestamp,
  /// A card took the focus since the tab opened.
  focused: Arc<Mutex<bool>>,
  composer: Signal<bool>,
  composer_focus: Signal<u64>,
}

impl PartialEq for Decisions {
  /// One tab's, for as long as it is mounted.
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.focused, &other.focused)
  }
}

impl lurq::app::component::DevtoolsInspectable for Decisions {}

impl Decisions {
  /// A tab's, made when it is created.
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      opened_at: Services::of(ctx).clock.now(),
      focused: Arc::default(),
      composer: ctx.signal(false),
      composer_focus: ctx.signal(0),
    }
  }

  /// The tab's, once it provided them; a card shown elsewhere has none.
  pub(super) fn of(ctx: &mut Ctx) -> Option<Self> {
    ctx.use_context::<Self>()
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

  /// Whether the composer takes free answers here.
  pub(super) fn composer_attached(&self) -> bool {
    self.composer.get()
  }

  /// Asks the composer to take the focus.
  pub(super) fn ask_composer(&self) {
    self.composer_focus.update(|asks| *asks += 1);
  }

  /// Whether a card that waits since `since` takes the focus now: the first that already waited when the
  /// tab opened, once. A request that comes later never takes it from what the person is doing.
  pub(super) fn take_focus(&self, since: Timestamp) -> bool {
    if since > self.opened_at {
      return false;
    }
    let mut focused = self.focused.lock();
    !std::mem::replace(&mut *focused, true)
  }
}
