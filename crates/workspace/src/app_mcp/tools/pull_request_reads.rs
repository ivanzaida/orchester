//! What `get_pull_request` read from GitHub of a pull request the synced list does not hold (an older closed
//! one, or a number GitHub does not have), kept 30 s ([`crate::pull_requests::list::REFRESH_INTERVAL`]): a
//! refresh within it answers from what was read instead of asking GitHub again, as a refresh of the list
//! answers from what its sync stored.

use std::{collections::HashMap, sync::Arc};

use orchester_core::{id::ProjectId, pull_request::SyncFailure, time::Timestamp};
use parking_lot::Mutex;

use crate::pull_requests::list::{ListedPullRequest, is_recent};

/// What a read of one pull request outside the list came to.
#[derive(Clone, Debug)]
pub(crate) enum ReadOne {
  Read(Box<ListedPullRequest>),
  /// GitHub has no pull request of that number in the repository.
  NotFound,
  /// GitHub was asked and the pull request was not read.
  Failed(SyncFailure),
}

/// Pull request `number` of a project.
type Key = (ProjectId, u64);

/// The reads of the last 30 s, each with when it asked GitHub; shared by every call of the server.
#[derive(Clone, Default)]
pub(crate) struct RecentReads(Arc<Mutex<HashMap<Key, (Timestamp, ReadOne)>>>);

impl RecentReads {
  /// What reading the pull request `key` names came to less than 30 s before `now`.
  /// Older reads are forgotten.
  pub(crate) fn recent(&self, key: Key, now: Timestamp) -> Option<ReadOne> {
    let mut reads = self.0.lock();
    reads.retain(|_, (at, _)| is_recent(*at, now));
    reads.get(&key).map(|(_, read)| read.clone())
  }

  /// Keeps what reading the pull request `key` names at `at` came to.
  pub(crate) fn keep(&self, key: Key, at: Timestamp, read: ReadOne) {
    self.0.lock().insert(key, (at, read));
  }
}
