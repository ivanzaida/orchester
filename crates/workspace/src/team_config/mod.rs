//! The team configuration of each team project open this session, held in memory only
//! (`docs/design/team-configuration-live.md`): read from the team server when the project's event stream opens, and
//! again each time it opens after a drop, and kept current by the stream's events (`sharing::sync::live`). Nothing of
//! it is written to the project's database; what this computer fills in for it (connections, program paths, secret
//! values, MCP switches) stays there, keyed by the team resource's id. A local project has none.

mod config;

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::time::Timestamp;
use parking_lot::Mutex;

pub use self::config::{NotTeamForm, TeamConfig, TeamResource, TeamVersion};
use crate::messages::Message;

/// How a team project's configuration stands this session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamRead {
  /// Nothing has been read yet, and no read has failed: the stream is connecting.
  Reading,
  /// The configuration was read this session; `unreached` says why the server cannot be reached now, while it cannot.
  Read {
    config: Arc<TeamConfig>,
    unreached: Option<Unreached>,
  },
  /// Nothing was read this session, and the server could not be reached for it.
  Unreachable(Unreached),
}

impl TeamRead {
  /// The configuration read this session, reachable or not.
  pub fn config(&self) -> Option<&Arc<TeamConfig>> {
    match self {
      Self::Read { config, .. } => Some(config),
      Self::Reading | Self::Unreachable(_) => None,
    }
  }

  /// Why the team server cannot be reached now; `None` while it can, or while it is being read.
  pub fn unreached(&self) -> Option<&Unreached> {
    match self {
      Self::Read { unreached, .. } => unreached.as_ref(),
      Self::Unreachable(unreached) => Some(unreached),
      Self::Reading => None,
    }
  }

  /// The configuration read this session, or why there is none to read from: a reader never falls back to an older
  /// copy.
  pub fn loaded(self) -> Result<Arc<TeamConfig>, TeamNotRead> {
    match self {
      Self::Read { config, .. } => Ok(config),
      Self::Reading => Err(TeamNotRead::Reading),
      Self::Unreachable(unreached) => Err(TeamNotRead::Unreachable(unreached)),
    }
  }
}

/// Why a team project's configuration cannot be read from memory now.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TeamNotRead {
  /// The stream is connecting and has read nothing yet.
  #[error("the team's configuration is being read")]
  Reading,
  /// Nothing was read this session, and the team server could not be reached for it.
  #[error("the team server cannot be reached")]
  Unreachable(Unreached),
}

impl TeamNotRead {
  /// What a person is told: the reading, or why the server cannot be reached.
  pub fn message(&self) -> Message {
    match self {
      Self::Reading => Message::new("teamConfig.reading"),
      Self::Unreachable(unreached) => unreached.reason.clone(),
    }
  }
}

/// Why the team server could not be reached, and when it was last tried.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unreached {
  pub reason: Message,
  pub at: Timestamp,
}

/// The team configurations of this session's team projects, by folder. Cloning shares them.
#[derive(Clone, Default)]
pub struct TeamConfigs {
  reads: Arc<Mutex<HashMap<PathBuf, TeamRead>>>,
}

impl TeamConfigs {
  /// How the configuration of the team project at `root` stands; [`TeamRead::Reading`] until its stream says more.
  pub fn read(&self, root: &Path) -> TeamRead {
    self.reads.lock().get(root).cloned().unwrap_or(TeamRead::Reading)
  }

  /// The configuration of `root` as the server answered it now.
  pub(crate) fn fill(&self, root: &Path, config: TeamConfig) {
    let read = TeamRead::Read {
      config: Arc::new(config),
      unreached: None,
    };
    self.reads.lock().insert(root.to_owned(), read);
  }

  /// The server of `root` could not be reached: what was read stays, said unreachable, and with nothing read the
  /// project reads unreachable.
  pub(crate) fn unreached(&self, root: &Path, unreached: Unreached) {
    let mut reads = self.reads.lock();
    let next = match reads.remove(root) {
      Some(TeamRead::Read { config, .. }) => TeamRead::Read {
        config,
        unreached: Some(unreached),
      },
      _ => TeamRead::Unreachable(unreached),
    };
    reads.insert(root.to_owned(), next);
  }

  /// Changes the configuration read for `root` with `change`, which says whether it changed anything; nothing is done
  /// while none was read. Whether it changed.
  pub(crate) fn update(&self, root: &Path, change: impl FnOnce(&mut TeamConfig) -> bool) -> bool {
    let mut reads = self.reads.lock();
    let Some(TeamRead::Read { config, .. }) = reads.get_mut(root) else {
      return false;
    };
    change(Arc::make_mut(config))
  }

  /// Keeps only the projects at `roots`: a project no longer shared, or closed, has no team configuration.
  pub(crate) fn keep(&self, roots: &[&Path]) {
    self.reads.lock().retain(|root, _| roots.contains(&root.as_path()));
  }
}
