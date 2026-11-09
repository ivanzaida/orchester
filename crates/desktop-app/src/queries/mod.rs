//! What the settings tabs show, read through lurq's typed queries (the person's decision of 2026-11-02): one
//! [`QueryClient`] at the window's root keeps each stored setting in memory for the session, so a tab shows its last
//! read in its first frame and reads again behind it, and only a setting never read yet shows its skeleton.
//!
//! The workspace stays the one owner of reads and writes: each query only calls its read off the UI thread. A query's
//! arguments are its key: the [`Session`] it reads from and, for a project's data, the project's folder. Stored
//! settings stay fresh for a day (`stale_time = "24h"`), which is the session in practice, until a change invalidates
//! them; live state outside the database (the browser found, a source's place, the code index's builds, the servers)
//! goes stale at once (`"0s"`), so a tab that shows it shows its last value and reads it again behind it. An entry no
//! tab shows is kept for a day (`gc_time = "24h"`).
//!
//! [`follow`] keeps them: the window's root reads the application's settings as the session starts ([`AppReads`]),
//! each open project's loader reads its settings as it opens ([`ProjectReads`]), and both invalidate what a change
//! touched ([`changes`]): automation's recorded changes, the accounts', the update settings' and the runs'. A save
//! invalidates its own query once its write ended.
//!
//! The Agents hub reads the same way ([`hub`]), keyed by the project's folder: a team project's hub and a local one's
//! are live (`"0s"`: what this computer filled in is looked up in the keychain and on disk as they are read), so a
//! visit shows the last answer at once and reads it again behind it; what a local instruction's, skill's and new
//! profile's panel edit over is stored (`"24h"`). Nothing reads them ahead of a visit. [`ProjectReads`] invalidates
//! them with the project's other reads: the resources' and connections' changes (a panel's own save records one), the
//! runs' state for a local hub, and for a team's hub a change of the team's configuration, which is how the team
//! server's events reach it (the project's event stream applies each to the project's cache and wakes
//! `Servers::subscribe_shared`, which the project's feed follows by reading the configuration again). A screen
//! built on futures reads one through [`reading`].
//!
//! [`QueryClient`]: lurq::query::QueryClient

pub mod app;
pub mod changes;
pub mod follow;
pub mod hub;
mod kept;
pub mod project;

use std::hash::{Hash, Hasher};

use lurq::{
  app::ctx::{Ctx, FutureState},
  query::QueryHandle,
};
use orchester_workspace::messages::Message;
use parking_lot::Mutex;

pub use self::{
  follow::{AppReads, ProjectReads},
  kept::Kept,
};
use crate::services::Services;

/// The session a query reads from. A window has one, so every key of a client holds the same session: it adds nothing
/// to a key's hash, and two are equal when they share the workspace.
#[derive(Clone)]
pub struct Session(pub Services);

impl Session {
  /// The session of the window `ctx` renders in.
  pub fn of(ctx: &mut Ctx) -> Self {
    Self(Services::of(ctx))
  }
}

impl PartialEq for Session {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl Eq for Session {}

impl Hash for Session {
  fn hash<H: Hasher>(&self, _: &mut H) {}
}

/// `handle` as the state a tab built on futures reads: pending only while the entry has no data, so a cached entry
/// read again behind it is fulfilled with its last data; rejected while its last read failed.
pub fn state<T: Clone>(handle: &QueryHandle<T, Message>) -> FutureState<T, Message> {
  let data = handle.data().map(|data| T::clone(&data));
  match (handle.error(), data) {
    (Some(error), data) => FutureState::rejected(Message::clone(&error), data),
    (None, Some(data)) => FutureState::fulfilled(data),
    (None, None) => FutureState::pending(None),
  }
}

/// [`state`], pending with the entry's last data while a read is under way: a screen that shows the last answer while
/// it is read again, and keeps a failure while the read that follows it runs (`LastFailure`), reads its query so.
pub fn reading<T: Clone>(handle: &QueryHandle<T, Message>) -> FutureState<T, Message> {
  if handle.fetching() {
    return FutureState::pending(handle.data().map(|data| T::clone(&data)));
  }
  state(handle)
}

/// Forgets a control's own choice once the stored value is it again: its save's read landed, or a change from
/// elsewhere chose the same, and what is stored shows from then on, a later change from elsewhere too.
pub fn settle<T>(chosen: &lurq::core::Signal<Option<T>>, stored: Option<&T>, saving: bool)
where
  T: Clone + PartialEq + Send + Sync + lurq::core::SignalValue + 'static,
{
  let Some(choice) = chosen.get_untracked() else {
    return;
  };
  if !saving && stored == Some(&choice) {
    chosen.set(None);
  }
}

/// What a tab asks to read again, as one number for [`Again::follow`]: the hash of the counts it follows, so two
/// counts moving apart never cancel out as a sum of them would.
pub fn asked(counts: &impl Hash) -> u64 {
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  counts.hash(&mut hasher);
  hasher.finish()
}

/// A tab's own count of the times it asks to read again (*Try again*, *Check again*, a poll while something runs), each
/// of which invalidates its query once a render sees it move. For a stored setting the first render only takes it; for
/// live state ([`Again::live`]) it reads again too, so each visit shows the last value and reads it again behind it,
/// never only joining a read that started before the visit.
#[derive(Default)]
pub struct Again {
  seen: Mutex<Option<u64>>,
  live: bool,
}

impl Again {
  /// For live state: the first render reads again as well.
  pub fn live() -> Self {
    Self {
      seen: Mutex::default(),
      live: true,
    }
  }

  /// Invalidates `handle` when `asked` moved since the last render saw it (or on the first, for live state); returns
  /// whether it did.
  pub fn follow<T, E>(&self, asked: u64, handle: &QueryHandle<T, E>) -> bool {
    let mut seen = self.seen.lock();
    let again = seen.map_or(self.live, |seen| seen != asked);
    if again {
      handle.invalidate();
    }
    *seen = Some(asked);
    again
  }

  /// [`Self::follow`] for a query that follows counts which move faster than it reads, as `project::settled_read`
  /// does for a future: an invalidation drops the read under way and starts another, so while `handle` reads, a later
  /// ask waits, and the first render after that read invalidates it once for the asks made meanwhile. On the first
  /// render a read under way is the one the visit started (live state goes stale at once), which is the read asked for.
  pub fn follow_settled<T, E>(&self, asked: u64, handle: &QueryHandle<T, E>) -> bool {
    if !handle.fetching() {
      return self.follow(asked, handle);
    }
    let mut seen = self.seen.lock();
    if seen.is_some() {
      return false;
    }
    *seen = Some(asked);
    self.live
  }
}
