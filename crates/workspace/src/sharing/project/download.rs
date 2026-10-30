//! A shared project's first download (ORC-282, ORC-291): a join of a team server, with a code or a cloud identity,
//! marks every checkout linked to a project there as downloading, and a link of a checkout without tasks marks that
//! one; both read the caches at once, without waiting for the minute's round. The round's sync of a marked checkout
//! reports here as it goes ([`DownloadProgress`]) and ends the mark as done, or as a failure that says why; a round
//! that could not sync a marked checkout at all (its server is no longer in the list, its folder is gone) fails it too,
//! so no checkout stays downloading for work that is not happening. *Try again* marks a failed one downloading again
//! and reads the caches again; the next round continues at the page that failed, and a later round after the end reads
//! from the start and changes nothing here. The Tasks list shows the mark while the project has no task yet.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  time::Instant,
};

use orchester_api::TasksPage;
use orchester_core::id::ServerId;

use super::{
  super::{ServerEntry, Servers, SharingError},
  download_progress::{DownloadFailure, DownloadProgress, DownloadState},
  sync::Synced,
};
use crate::{background, messages::Message};

/// How a checkout's first download from its team server stands, while it runs or after it failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FirstDownload {
  /// The project is being read from the server named `server`.
  Downloading { server: String },
  /// Reading it failed, and why.
  Failed { server: String, reason: Message },
}

impl FirstDownload {
  /// `progress` as the Tasks list follows it: `None` once the download is done.
  fn of(progress: &DownloadProgress) -> Option<Self> {
    let server = progress.server.clone();
    match &progress.state {
      DownloadState::Downloading => Some(Self::Downloading { server }),
      DownloadState::Done => None,
      DownloadState::Failed { reason, .. } => Some(Self::Failed {
        server,
        reason: reason.clone(),
      }),
    }
  }
}

/// A checkout's first download as kept, where its next page starts, and when it was asked for: a round that began
/// before that settles nothing of it.
pub(crate) struct Mark {
  progress: DownloadProgress,
  after: u32,
  asked: Instant,
}

impl Servers {
  /// How the first download of the checkout at `root` stands; `None` when none was asked for, or it is done.
  pub fn download_state(&self, root: &Path) -> Option<FirstDownload> {
    let downloads = self.inner.downloads.lock();
    downloads.get(root).and_then(|mark| FirstDownload::of(&mark.progress))
  }

  /// What the first download of the checkout at `root` reported last, also once it is done; `None` when none was asked
  /// for this session. [`Self::subscribe_shared`] wakes when it changed.
  pub fn download_progress(&self, root: &Path) -> Option<DownloadProgress> {
    self.inner.downloads.lock().get(root).map(|mark| mark.progress.clone())
  }

  /// What a join of `server`, named `name`, starts: every checkout linked to a project there is downloading, and the
  /// caches are read at once.
  pub(crate) async fn first_download(&self, server: ServerId, name: &str) {
    let servers = self.clone();
    let linked = match background::run(move || servers.linked_projects(server)).await {
      Ok(Ok(linked)) => linked,
      Ok(Err(error)) => {
        // The sync below reads them all the same; only the Tasks list's downloading state is missing then.
        tracing::warn!(%error, "cannot read which projects are linked to a server just joined");
        Vec::new()
      }
      // The blocking pool is shutting down with the window: there is no round left to sync in.
      Err(_) => return,
    };
    self.mark_downloading(linked.into_iter().map(|project| project.repo_root), name);
  }

  /// What a link of the checkout at `root` to a project on the server named `name` starts when it took no tasks: they
  /// are read at once.
  pub(super) fn download_linked(&self, root: &Path, name: &str) {
    self.mark_downloading(std::iter::once(root.to_owned()), name);
  }

  /// The checkouts at `roots` begin their first download from the server named `name`, and the caches are read at once.
  fn mark_downloading(&self, roots: impl Iterator<Item = PathBuf>, name: &str) {
    let asked = Instant::now();
    {
      let mut downloads = self.inner.downloads.lock();
      for root in roots {
        let progress = DownloadProgress::started(name.to_owned());
        downloads.insert(
          root,
          Mark {
            progress,
            after: 0,
            asked,
          },
        );
      }
    }
    self.shared_changed();
    self.sync_now();
  }

  /// *Try again* after the first download of the checkout at `root` failed: it downloads again, from the page that
  /// failed. Nothing happens for a checkout whose download did not fail.
  pub fn retry_download(&self, root: &Path) {
    {
      let mut downloads = self.inner.downloads.lock();
      let Some(mark) = downloads.get_mut(root) else {
        return;
      };
      if !matches!(mark.progress.state, DownloadState::Failed { .. }) {
        return;
      }
      mark.progress.state = DownloadState::Downloading;
      mark.asked = Instant::now();
    }
    self.shared_changed();
    self.sync_now();
  }

  /// Where a round that began at `round` starts reading the tasks of the checkout at `root`: where its first download
  /// stopped, or at the start.
  pub(super) fn download_resume(&self, root: &Path, round: Instant) -> u32 {
    let mut downloads = self.inner.downloads.lock();
    unsettled(&mut downloads, root, round).map_or(0, |mark| mark.after)
  }

  /// A round that began at `round` has the tags and the modules of the checkout at `root` in its cache.
  pub(super) fn download_parts_read(&self, root: &Path, round: Instant) {
    {
      let mut downloads = self.inner.downloads.lock();
      let Some(mark) = unsettled(&mut downloads, root, round) else {
        return;
      };
      mark.progress.parts_read();
    }
    self.shared_changed();
  }

  /// A round that began at `round` has `page` of the tasks of the checkout at `root` in its cache. A server that does
  /// not know where a page starts repeats the tasks before it, which are not new arrivals.
  pub(super) fn download_page_stored(&self, root: &Path, page: &TasksPage, round: Instant) {
    let mut downloads = self.inner.downloads.lock();
    let Some(mark) = unsettled(&mut downloads, root, round) else {
      return;
    };
    let arrived: Vec<_> = page.tasks.iter().filter(|task| task.number > mark.after).collect();
    mark.progress.stored(&arrived, page.total);
    mark.after = page
      .next
      .or(arrived.iter().map(|task| task.number).max())
      .unwrap_or(mark.after);
  }

  /// A round that began at `round` synced the checkout at `root`, on `entry`: reading it all ends its first download, a
  /// project gone from the server or a failure says why.
  pub(super) fn downloaded(
    &self,
    (root, entry): (&Path, &ServerEntry),
    synced: Result<&Synced, &SharingError>,
    round: Instant,
  ) {
    {
      let mut downloads = self.inner.downloads.lock();
      let Some(mark) = unsettled(&mut downloads, root, round) else {
        return;
      };
      match synced {
        Ok(Synced::Read) => mark.progress.done(),
        Ok(Synced::Removed) => mark.progress.failed(&SharingError::NotMember, &entry.address),
        Err(error) => mark.progress.failed(error, &entry.address),
      }
      mark.progress.server.clone_from(&entry.stored.name);
      mark.asked = round;
    }
    self.shared_changed();
  }

  /// A round that began at `round` synced the checkouts at `attempted` only: one still downloading that it could not
  /// sync at all fails, its server no longer being one this computer reaches it through.
  pub(super) fn unreached(&self, attempted: &[PathBuf], round: Instant) {
    let mut failed = false;
    {
      let mut downloads = self.inner.downloads.lock();
      for (root, mark) in downloads.iter_mut() {
        if mark.asked > round || attempted.contains(root) {
          continue;
        }
        if mark.progress.state == DownloadState::Downloading {
          // `SharingError::ServerMissing`'s words, which name no address.
          mark.progress.state = DownloadState::Failed {
            failure: DownloadFailure::ServerMissing,
            reason: Message::new("sharing.errors.serverMissing"),
          };
          failed = true;
        }
      }
    }
    if failed {
      self.shared_changed();
    }
  }
}

/// The first download of the checkout at `root` a round that began at `round` still reports to: one asked for by then
/// that did not end.
fn unsettled<'a>(downloads: &'a mut HashMap<PathBuf, Mark>, root: &Path, round: Instant) -> Option<&'a mut Mark> {
  downloads
    .get_mut(root)
    .filter(|mark| mark.asked <= round && mark.progress.state != DownloadState::Done)
}
