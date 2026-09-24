//! User-facing messages for errors from core and store: an i18n key plus its placeholder
//! values. Errors are mapped here, at the UI boundary; their `Display` text is for logs only.

use orchester_core::{
  name::{Name, NameError},
  resource::PublishError,
};
use orchester_store::StoreError;

use crate::runner::state::RunnerFailure;

/// A translatable message: `key` in the locale files and the values of its placeholders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
  pub key: &'static str,
  pub args: Vec<(&'static str, String)>,
}

impl Message {
  pub const fn new(key: &'static str) -> Self {
    Self { key, args: Vec::new() }
  }

  pub fn with(mut self, name: &'static str, value: String) -> Self {
    self.args.push((name, value));
    self
  }
}

impl lurq::app::component::DevtoolsInspectable for Message {}

pub fn name_error(error: &NameError) -> Message {
  match error {
    NameError::Blank => Message::new("errors.nameRequired"),
    NameError::TooLong { .. } => Message::new("errors.nameTooLong").with("max", Name::MAX_CHARS.to_string()),
  }
}

pub fn store_error(error: &StoreError) -> Message {
  match error {
    StoreError::NotAGitRepository(_) => Message::new("errors.notAGitRepository"),
    StoreError::AlreadyInitialized(_) => Message::new("errors.alreadyInitialized"),
    StoreError::NotInitialized(_) => Message::new("errors.projectMissing"),
    StoreError::SchemaTooNew { .. } => Message::new("errors.newerSchema"),
    StoreError::Unrepresentable(_) => Message::new("errors.unsupportedPath"),
    StoreError::DuplicateConnectionName(_) => Message::new("errors.connectionNameTaken"),
    StoreError::Sqlite(_)
    | StoreError::Io { .. }
    | StoreError::Corrupt { .. }
    | StoreError::TaskNotFound(_)
    | StoreError::EmptyBatch
    | StoreError::ResourceNotFound(_)
    | StoreError::ResourceKindMismatch { .. }
    | StoreError::DuplicateSlug { .. }
    | StoreError::NoDraft(_)
    | StoreError::ConnectionNotFound(_) => Message::new("errors.storage"),
    StoreError::Publish(PublishError::StaleDraft { latest, .. }) => Message::new("errors.staleDraft").with(
      "version",
      latest.map_or_else(String::new, |version| version.to_string()),
    ),
    StoreError::Publish(PublishError::VersionLimit) => Message::new("errors.versionLimit"),
    StoreError::Publish(PublishError::NoSteps | PublishError::UnassignedSteps(_)) => {
      Message::new("pipelines.problems.notPublishable")
    }
  }
}

/// Work that ran on a background thread panicked or was cancelled; the cause is logged.
pub fn interrupted() -> Message {
  Message::new("errors.interrupted")
}

pub fn runner_failure(failure: &RunnerFailure) -> Message {
  match failure {
    RunnerFailure::MissingExecutable { path } => {
      Message::new("errors.runnerMissing").with("path", path.display().to_string())
    }
    RunnerFailure::AlreadyServed => Message::new("errors.runnerAlreadyServed"),
    RunnerFailure::StartFailed => Message::new("errors.runnerStartFailed"),
    RunnerFailure::Lost => Message::new("errors.runnerLost"),
  }
}
