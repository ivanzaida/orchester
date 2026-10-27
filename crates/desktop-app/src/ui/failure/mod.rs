//! What shows where a read or a save failed (`Failure states · redesign` `SsC16`). The failure replaces only what failed;
//! the header, the tabs, the sidebar and what was read keep working:
//!
//! - a page that can't be read: a card on `surface_panel` in a `border` stroke, its icon `danger` on `danger_muted` and
//!   no other danger, saying what couldn't be read, why in plain words and what still works, with Try again, Open log
//!   and its Details ([`Failure::page`], `01` `urbxm`, `01b` `K76n04`); after three tries it says *Still can't be read*
//!   and leads with Open log;
//! - a section that can't be read: one row in place of the group's rows, under its heading ([`Failure::section`]);
//! - an item that can't be read: its row keeps its place and its name ([`Failure::row`]);
//! - a change that wasn't saved: the header's save status names it ([`not_saved`]) and the field keeps the value with a
//!   `danger` stroke and a hint ([`field_hint`]);
//! - a wait on something that comes back (the database busy): information, not an error, and it tries again on its
//!   own ([`is_transient`]).
//!
//! Its words are `ui.failure.*`; the why comes from the workspace's typed [`Message`].

mod card;
mod rows;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureState, FutureStatus},
  node::Element,
};
use orchester_core::time::Timestamp;
use orchester_workspace::messages::{FailureKind, Message, failure_kind};
use parking_lot::Mutex;

pub use self::rows::{field_hint, not_saved};
use super::message::message_text;

type Handler = Arc<dyn Fn() + Send + Sync>;

/// After this many failed tries the card says it still can't be read and leads with the log.
pub const STILL_AFTER: u32 = 3;

/// The error of a read, kept while the read runs again: a future's state forgets it while it is pending, and Try again
/// keeps the card in place, saying *Trying again…*.
#[derive(Default)]
pub struct LastFailure(Mutex<Option<Message>>);

impl LastFailure {
  /// What `state` failed with and whether it is being read again; `None` once a read succeeded.
  pub fn follow<T>(&self, state: &FutureState<T, Message>) -> Option<(Message, bool)> {
    let mut last = self.0.lock();
    match state.status {
      FutureStatus::Rejected => {
        last.clone_from(&state.error);
        last.clone().map(|error| (error, false))
      }
      FutureStatus::Pending => last.clone().map(|error| (error, true)),
      FutureStatus::Fulfilled => {
        *last = None;
        None
      }
      FutureStatus::Idle => None,
    }
  }
}

/// What failed and how to try again: the description each failure state is drawn from.
#[derive(Clone)]
pub struct Failure {
  id: Arc<str>,
  what: Arc<str>,
  error: Message,
  still_works: Option<Arc<str>>,
  /// The read is under way again.
  retrying: bool,
  retry: Option<Handler>,
  /// The project whose runner log Open log opens.
  log: Option<PathBuf>,
  /// When what still shows was read, for a transient wait.
  read_at: Option<Timestamp>,
}

impl PartialEq for Failure {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.id,
      &self.what,
      &self.error,
      &self.still_works,
      self.retrying,
      self.retry.is_some(),
      &self.log,
      self.read_at,
    ) == (
      &other.id,
      &other.what,
      &other.error,
      &other.still_works,
      other.retrying,
      other.retry.is_some(),
      &other.log,
      other.read_at,
    )
  }
}

impl Failure {
  /// `what` couldn't be read, in the app's words (*Agent profiles couldn't be read*), because of `error`. Its element
  /// id is `id`; its parts are `<id>-retry`, `<id>-log`, `<id>-details-toggle`, `<id>-details` and `<id>-copy`.
  pub fn new(id: impl Into<Arc<str>>, what: impl Into<Arc<str>>, error: Message) -> Self {
    Self {
      id: id.into(),
      what: what.into(),
      error,
      still_works: None,
      retrying: false,
      retry: None,
      log: None,
      read_at: None,
    }
  }

  /// Try again runs `retry`; while `retrying` the read is under way and the button says *Trying again…*.
  #[must_use]
  pub fn retry(mut self, retrying: bool, retry: impl Fn() + Send + Sync + 'static) -> Self {
    self.retrying = retrying;
    self.retry = Some(Arc::new(retry));
    self
  }

  /// Open log opens the runner log of the project at `root`.
  #[must_use]
  pub fn log(mut self, root: PathBuf) -> Self {
    self.log = Some(root);
    self
  }

  /// What still works while this can't be read.
  #[must_use]
  pub fn still_works(mut self, text: impl Into<Arc<str>>) -> Self {
    self.still_works = Some(text.into());
    self
  }

  /// When what still shows was read; a transient wait says it.
  #[must_use]
  pub fn read_at(mut self, at: Option<Timestamp>) -> Self {
    self.read_at = at;
    self
  }

  /// Whether this failure is a wait on something that comes back rather than an error.
  pub fn transient(&self) -> bool {
    is_transient(&self.error)
  }

  /// The page's card, in place of the tab's content.
  pub fn page(self, ctx: &mut Ctx) -> Element {
    card::page(ctx, self)
  }

  /// One row in place of a group's rows; the group keeps its heading.
  pub fn section(self, ctx: &Ctx) -> Element {
    rows::section(ctx, &self)
  }

  /// A list's row for an item that can't be read, with its `name` when it is known.
  pub fn row(self, ctx: &mut Ctx, name: Option<&str>) -> Element {
    rows::row(ctx, &self, name)
  }
}

/// Whether `error` is a wait on something that comes back on its own (the database busy) rather than an error.
pub fn is_transient(error: &Message) -> bool {
  failure_kind(error) == FailureKind::Busy
}

/// Why it couldn't be read, in one plain sentence from the typed error.
pub fn why(ctx: &Ctx, error: &Message) -> Arc<str> {
  match failure_kind(error) {
    FailureKind::Storage => ctx.t("ui.failure.why.storage"),
    FailureKind::Interrupted => ctx.t("ui.failure.why.interrupted"),
    FailureKind::Busy | FailureKind::Other => ctx.t_args("ui.failure.why.other", [("reason", message_text(ctx, error))]),
  }
}

/// The error's kind and its message, as Details shows and copies them.
pub fn detail(ctx: &Ctx, error: &Message) -> Arc<str> {
  ctx.t_args(
    "ui.failure.kind",
    [("kind", Arc::from(error.key)), ("message", message_text(ctx, error))],
  )
}
