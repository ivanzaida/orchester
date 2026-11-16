//! One team project's reads in one session, as a query's argument: its [`SourceKey`] (the server, the session's
//! generation and the project) is what a cached read is kept by, and what makes two sources the same; the handle to the
//! project's reads, its checkout and since when its server has not answered ride along without being part of it. No
//! display name and no token is in the key, and the handle never prints.

use std::{
  hash::{Hash, Hasher},
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_api::releases::ReleaseVersion;
use orchester_core::{
  id::{ProjectId, ServerId},
  time::Timestamp,
};
use orchester_workspace::{
  sharing::{Servers, SharingError},
  team_reads::{TeamProject, TeamReadError},
};
use parking_lot::Mutex;

use super::reached::{Reached, ReadsReached};

/// What a team read is kept by: the server, the session's generation and the project on the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceKey {
  pub server: ServerId,
  pub generation: u64,
  pub project: ProjectId,
}

/// A team project's reads in one session.
#[derive(Clone)]
pub struct TeamSource {
  key: SourceKey,
  project: TeamProject,
  root: Arc<PathBuf>,
  servers: Servers,
  /// When a read last found the server unreachable, until one answers.
  unanswered: Arc<Mutex<Option<Timestamp>>>,
  /// Where the session keeps how each project's reads last ended.
  reached: ReadsReached,
}

impl TeamSource {
  pub(super) fn new(
    key: SourceKey,
    project: TeamProject,
    (root, servers, reached): (PathBuf, Servers, ReadsReached),
  ) -> Self {
    Self {
      key,
      project,
      root: Arc::new(root),
      servers,
      unanswered: Arc::default(),
      reached,
    }
  }

  pub const fn key(&self) -> SourceKey {
    self.key
  }

  pub(super) const fn project(&self) -> &TeamProject {
    &self.project
  }

  /// The project's checkout on this computer.
  pub fn root(&self) -> &Path {
    &self.root
  }

  pub(crate) const fn servers(&self) -> &Servers {
    &self.servers
  }

  /// What a read's `answer` says: the read, or why it failed, with since when the server has not answered. Whether
  /// the server answered is kept for the session ([`ReadsReached`]).
  pub(super) fn answered<T>(&self, answer: Result<T, TeamReadError>) -> Result<T, TeamFailure> {
    let mut unanswered = self.unanswered.lock();
    match answer {
      Ok(read) => {
        *unanswered = None;
        self.reached.record(&self.root, Reached::Answered);
        Ok(read)
      }
      Err(TeamReadError::Unavailable { .. }) => {
        self.reached.record(&self.root, Reached::Unanswered);
        Err(TeamFailure::Unavailable {
          since: *unanswered.get_or_insert_with(Timestamp::now),
        })
      }
      Err(error) => Err(TeamFailure::from(&error)),
    }
  }
}

impl PartialEq for TeamSource {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key
  }
}

impl Eq for TeamSource {}

impl Hash for TeamSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.key.hash(state);
  }
}

impl std::fmt::Debug for TeamSource {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("TeamSource")
      .field("key", &self.key)
      .finish_non_exhaustive()
  }
}

impl lurq::app::component::DevtoolsInspectable for TeamSource {}

/// Why a team project's data cannot be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamFailure {
  /// The server has not answered since `since`; nothing read before is shown as current.
  Unavailable { since: Timestamp },
  /// This computer is signed out of the server.
  SignedOut,
  /// The person is no longer a member of the workspace.
  NotMember,
  /// The server refuses this version of Orchester (`426 unsupported_contract`): it accepts desktops of `required` and
  /// later only.
  Unsupported { required: ReleaseVersion },
  /// The server refused the read otherwise, or answered with what the contract does not read.
  Refused,
}

impl From<&TeamReadError> for TeamFailure {
  fn from(error: &TeamReadError) -> Self {
    match error {
      TeamReadError::Unavailable { .. } => Self::Unavailable {
        since: Timestamp::now(),
      },
      TeamReadError::SignedOut | TeamReadError::SessionChanged => Self::SignedOut,
      TeamReadError::MembershipRevoked | TeamReadError::NotFound => Self::NotMember,
      TeamReadError::NeedsVersion { required } => Self::Unsupported {
        required: required.clone(),
      },
      TeamReadError::CursorExpired
      | TeamReadError::KeptChanging
      | TeamReadError::Refused(_)
      | TeamReadError::Server(_)
      | TeamReadError::Unreadable => Self::Refused,
    }
  }
}

impl From<&SharingError> for TeamFailure {
  fn from(error: &SharingError) -> Self {
    match error {
      SharingError::Offline => Self::Unavailable {
        since: Timestamp::now(),
      },
      SharingError::SignedOut | SharingError::ServerMissing => Self::SignedOut,
      SharingError::NotMember => Self::NotMember,
      SharingError::Client(error) if error.is_connection() => Self::Unavailable {
        since: Timestamp::now(),
      },
      _ => Self::Refused,
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for TeamFailure {}
