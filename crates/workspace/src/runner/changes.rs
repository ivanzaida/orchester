//! What the projects' runners report as changed, counted per project and per run, for the screens that read runs
//! again when they change.

use std::{collections::HashMap, hash::Hash, path::Path, sync::Arc};

use orchester_core::id::{ChatId, RunId};
use parking_lot::Mutex;
use tokio::sync::watch;

use super::Runners;

/// The change counts of one project's runs or conversations that something follows ([`Runners::run_changes_of`],
/// [`Runners::chat_changes_of`]).
///
/// A count lives only while a receiver of it does: it is made by the first subscription, and once all its receivers
/// are gone it is dropped the next time the project's counts of the kind are touched (a change, a subscription). So
/// the map never holds more keys than are followed, plus those dropped since that last touch, however many runs or
/// conversations a long session reports. A key subscribed to again starts a new count, which is harmless: readers
/// follow when a count moves, never its value.
#[derive(Clone)]
pub(super) struct Followed<K> {
  counts: Arc<Mutex<HashMap<K, watch::Sender<u64>>>>,
}

/// The change counts of the project's runs.
pub(super) type RunChanges = Followed<RunId>;

/// The change counts of the project's conversations.
pub(super) type ChatChanges = Followed<ChatId>;

impl<K> Default for Followed<K> {
  fn default() -> Self {
    Self { counts: Arc::default() }
  }
}

impl<K: Eq + Hash> Followed<K> {
  pub(super) fn subscribe(&self, key: K) -> watch::Receiver<u64> {
    let mut counts = self.counts.lock();
    counts.retain(|_, count| count.receiver_count() > 0);
    counts.entry(key).or_insert_with(|| watch::Sender::new(0)).subscribe()
  }

  /// `key` changed; `None` for a change that does not say which, which counts for every key.
  pub(super) fn changed(&self, key: Option<K>) {
    let mut counts = self.counts.lock();
    counts.retain(|_, count| count.receiver_count() > 0);
    let bump = |count: &watch::Sender<u64>| count.send_modify(|revision| *revision += 1);
    match key {
      Some(key) => counts.get(&key).into_iter().for_each(bump),
      None => counts.values().for_each(bump),
    }
  }
}

impl Runners {
  /// Changes whenever project `root`'s runner reports that a run changed state.
  pub fn states_of(&self, root: &Path) -> watch::Receiver<u64> {
    self.inner.shared.counters(root).states.subscribe()
  }

  /// Changes whenever project `root`'s runner reports that a run or its transcript changed.
  pub fn changes_of(&self, root: &Path) -> watch::Receiver<u64> {
    self.inner.shared.counters(root).changes.subscribe()
  }

  /// [`Self::changes_of`] for run `run` of project `root` alone: changes whenever the runner reports that this run
  /// or its transcript changed, or a change that names no run (such as [`Self::base_notices_queued`]), and not
  /// for the project's other runs. See [`RunChanges`] for how long a run's count is kept.
  pub fn run_changes_of(&self, root: &Path, run: RunId) -> watch::Receiver<u64> {
    self.inner.shared.counters(root).runs.subscribe(run)
  }

  /// Tells the readers of [`Self::changes_of`] and of every run's [`Self::run_changes_of`] that the desktop queued
  /// notices that project `root`'s base branch moved (`crate::base_moved`): the runs' timelines read them again. A
  /// run's state did not change.
  pub fn base_notices_queued(&self, root: &Path) {
    self.inner.shared.counters(root).count(None, false);
  }

  /// Counts a change as project `root`'s runner reports it: a run's state (`state`), which moves its transcript on
  /// too, or its transcript alone; of run `run`, or of no run in particular (`None`), which counts for every run.
  /// For tests of what follows the counts, which run no runner.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn count_change_for_test(&self, root: &Path, run: Option<RunId>, state: bool) {
    if state {
      self.inner.shared.runs.send_modify(|revision| *revision += 1);
    }
    self.inner.shared.counters(root).count(run, state);
  }
}
