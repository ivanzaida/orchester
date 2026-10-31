//! What a shared project's first download reports while it runs (design frame `YN6MO`, `docs/design/startup.md`
//! decision 7): the tasks stored so far of how many the server says, the latest to arrive, each part's state, and how
//! it ended. The team settings are never read by the sync yet (S5), so they say that rather than pending or done.

use orchester_api::{
  ErrorCode, SharedTask,
  client::{ClientError, ServerAddress},
};
use orchester_core::task::organization::WorkflowStatus;

use crate::{messages::Message, sharing::SharingError};

/// How many of the latest tasks to arrive a download keeps.
const LATEST_ARRIVALS: usize = 3;

/// How a checkout's first download from its team server stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadProgress {
  /// The server's name.
  pub server: String,
  pub state: DownloadState,
  /// How many tasks the project has, once the server said, or once all of them arrived.
  pub total: Option<usize>,
  /// How many tasks this download stored so far.
  pub stored: usize,
  /// The latest tasks to arrive, newest first.
  pub latest: Vec<Arrival>,
  pub tags: PartState,
  pub modules: PartState,
  pub team_settings: PartState,
}

/// Whether a first download runs, or how it ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadState {
  Downloading,
  Done,
  /// It stopped, and why; what was stored stays, and a retry continues from there.
  Failed {
    failure: DownloadFailure,
    reason: Message,
  },
}

/// A task that arrived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arrival {
  pub key: String,
  pub title: String,
  pub workflow: WorkflowStatus,
}

/// How one part of a project stands in its first download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartState {
  Pending,
  Done,
  /// The sync does not read this part.
  NotAvailable,
}

/// Why a first download stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadFailure {
  /// The server cannot be reached now.
  Offline,
  /// This computer has no token the server accepts.
  SignedOut,
  /// The person is not a member of the project on the server.
  NotMember,
  /// The server is no longer in the list.
  ServerMissing,
  /// The server refused a request.
  Refused,
  /// What answered did not keep to the contract.
  InvalidAnswer,
  /// The server's certificate is not trusted.
  Certificate,
  /// This computer failed: its database, its keychain or its own work.
  Local,
}

impl DownloadProgress {
  /// A download that just began, from the server named `server`.
  pub(super) fn started(server: String) -> Self {
    Self {
      server,
      state: DownloadState::Downloading,
      total: None,
      stored: 0,
      latest: Vec::new(),
      tags: PartState::Pending,
      modules: PartState::Pending,
      team_settings: PartState::NotAvailable,
    }
  }

  /// The project's tags and modules are in the cache.
  pub(super) fn parts_read(&mut self) {
    self.tags = PartState::Done;
    self.modules = PartState::Done;
  }

  /// `arrived`, oldest first, are in the cache; the server says the project has `total` tasks when it knows.
  pub(super) fn stored(&mut self, arrived: &[&SharedTask], total: Option<usize>) {
    self.state = DownloadState::Downloading;
    self.stored += arrived.len();
    self.total = total.or(self.total);
    let older = std::mem::take(&mut self.latest);
    self.latest = arrived
      .iter()
      .rev()
      .map(|task| Arrival {
        key: task.key.clone(),
        title: task.content.title.clone(),
        workflow: task.content.workflow,
      })
      .chain(older)
      .take(LATEST_ARRIVALS)
      .collect();
  }

  /// Everything was read: a server that never said how many tasks there are had the ones stored.
  pub(super) fn done(&mut self) {
    self.state = DownloadState::Done;
    self.total = self.total.or(Some(self.stored));
  }

  /// `error` stopped the download, on the server at `address`.
  pub(super) fn failed(&mut self, error: &SharingError, address: &ServerAddress) {
    self.state = DownloadState::Failed {
      failure: error.into(),
      reason: error.message(address),
    };
  }
}

impl From<&SharingError> for DownloadFailure {
  fn from(error: &SharingError) -> Self {
    match error {
      SharingError::Client(error) => error.into(),
      SharingError::Offline => Self::Offline,
      SharingError::SignedOut => Self::SignedOut,
      SharingError::NotMember => Self::NotMember,
      SharingError::ServerMissing => Self::ServerMissing,
      SharingError::InvalidAnswer(_) => Self::InvalidAnswer,
      SharingError::CertificateChanged => Self::Certificate,
      SharingError::Store(_) | SharingError::Vault(_) | SharingError::Interrupted => Self::Local,
      SharingError::KeptChanging
      | SharingError::ShareInProgress
      | SharingError::NoPasswords
      | SharingError::TwoFactorOff
      | SharingError::TwoFactorSession
      | SharingError::UnprovenServer
      | SharingError::NoProject
      | SharingError::ProjectChanged
      | SharingError::AccountProject
      | SharingError::ForeignAddress { .. } => Self::Refused,
    }
  }
}

impl From<&ClientError> for DownloadFailure {
  fn from(error: &ClientError) -> Self {
    match error {
      ClientError::TimedOut | ClientError::Unreachable => Self::Offline,
      ClientError::Api {
        code: ErrorCode::Unauthorized,
        ..
      } => Self::SignedOut,
      ClientError::Api {
        code: ErrorCode::NotMember,
        ..
      } => Self::NotMember,
      ClientError::NotOrchester { .. } | ClientError::Redirect { .. } => Self::InvalidAnswer,
      ClientError::UntrustedCertificate => Self::Certificate,
      ClientError::Setup => Self::Local,
      ClientError::Address(_) | ClientError::PlainHttpRefused | ClientError::Api { .. } | ClientError::Cloud { .. } => {
        Self::Refused
      }
    }
  }
}
