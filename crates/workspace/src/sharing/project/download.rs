//! A shared project's first download after a join (ORC-282): a join of a team server, with a code or a cloud identity,
//! marks every checkout linked to a project there as downloading and reads the caches at once, without waiting for
//! the minute's round. The round's sync of each marked checkout takes its mark away, or turns it into a failure that
//! says why; a round that could not sync a marked checkout at all (its server is no longer in the list, its folder is
//! gone) fails it too, so no checkout stays downloading for work that is not happening. *Try again* marks a failed one
//! downloading again and reads the caches again. The Tasks list shows the mark while the project has no task yet.

use std::{
  path::{Path, PathBuf},
  time::Instant,
};

use orchester_core::id::ServerId;

use super::super::{ServerEntry, Servers, SharingError};
use crate::{background, messages::Message};

/// How a checkout's first download from its team server stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FirstDownload {
  /// The project is being read from the server named `server`.
  Downloading { server: String },
  /// Reading it failed, and why.
  Failed { server: String, reason: Message },
}

/// A checkout's first download as kept, and when it was asked for: a round that began before that settles nothing of
/// it.
pub(crate) struct Mark {
  download: FirstDownload,
  asked: Instant,
}

impl Servers {
  /// How the first download of the checkout at `root` stands; `None` when none was asked for, or it is done.
  pub fn download_state(&self, root: &Path) -> Option<FirstDownload> {
    self.inner.downloads.lock().get(root).map(|mark| mark.download.clone())
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
    let asked = Instant::now();
    {
      let mut downloads = self.inner.downloads.lock();
      for project in linked {
        let download = FirstDownload::Downloading {
          server: name.to_owned(),
        };
        downloads.insert(project.repo_root, Mark { download, asked });
      }
    }
    self.shared_changed();
    self.sync_now();
  }

  /// *Try again* after the first download of the checkout at `root` failed: it downloads again. Nothing happens for a
  /// checkout whose download did not fail.
  pub fn retry_download(&self, root: &Path) {
    {
      let mut downloads = self.inner.downloads.lock();
      let Some(mark) = downloads.get_mut(root) else {
        return;
      };
      let FirstDownload::Failed { server, .. } = &mark.download else {
        return;
      };
      mark.download = FirstDownload::Downloading { server: server.clone() };
      mark.asked = Instant::now();
    }
    self.shared_changed();
    self.sync_now();
  }

  /// A round that began at `round` synced the checkout at `root`, on `entry`: a success ends its first download, a
  /// failure says why.
  pub(super) fn downloaded(
    &self,
    (root, entry): (&Path, &ServerEntry),
    synced: Result<(), &SharingError>,
    round: Instant,
  ) {
    {
      let mut downloads = self.inner.downloads.lock();
      if downloads.get(root).is_none_or(|mark| mark.asked > round) {
        return;
      }
      match synced {
        Ok(()) => {
          downloads.remove(root);
        }
        Err(error) => {
          let download = FirstDownload::Failed {
            server: entry.stored.name.clone(),
            reason: error.message(&entry.address),
          };
          downloads.insert(root.to_owned(), Mark { download, asked: round });
        }
      }
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
        if let FirstDownload::Downloading { server } = &mark.download {
          // `SharingError::ServerMissing`'s words, which name no address.
          mark.download = FirstDownload::Failed {
            server: server.clone(),
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
