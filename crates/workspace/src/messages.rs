//! User-facing messages for errors from core and store: an i18n key plus its placeholder
//! values. Errors are mapped here, at the UI boundary; their `Display` text is for logs only.

use orchester_core::{
  name::{Name, NameError},
  resource::PublishError,
  secret::{SecretError, VaultError},
  task::EditError,
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

pub fn name_error(error: &NameError) -> Message {
  match error {
    NameError::Blank => Message::new("errors.nameRequired"),
    NameError::TooLong { .. } => Message::new("errors.nameTooLong").with("max", Name::MAX_CHARS.to_string()),
  }
}

/// The key of [`OrganizationError::StopRunFirst`](orchester_core::task::organization::OrganizationError):
/// a form that saves Canceled asks for confirmation when it sees it.
pub const CANCEL_CONFIRM_REQUIRED: &str = "tasks.cancel.confirmRequired";

pub fn organization_error(error: &orchester_core::task::organization::OrganizationError) -> Message {
  use orchester_core::task::organization::OrganizationError;
  Message::new(match error {
    OrganizationError::MissingReason => "tasks.blocked.reasonRequired",
    OrganizationError::DependencyCycle => "tasks.blocked.cycle",
    OrganizationError::StopRunFirst(_) => CANCEL_CONFIRM_REQUIRED,
    OrganizationError::DuplicateTag => "tasks.tags.duplicate",
    OrganizationError::TagNotFound(_) => "tasks.tags.stale",
    OrganizationError::SameTag => "tasks.tags.sameTarget",
    OrganizationError::Blocked(_) => "tasks.blocked.runUnavailable",
  })
}

pub fn store_error(error: &StoreError) -> Message {
  match error {
    StoreError::Organization(error) => organization_error(error),
    StoreError::NotAGitRepository(_) => Message::new("errors.notAGitRepository"),
    StoreError::AlreadyInitialized(_) => Message::new("errors.alreadyInitialized"),
    StoreError::NotInitialized(_) => Message::new("errors.projectMissing"),
    StoreError::SchemaTooNew { .. } => Message::new("errors.newerSchema"),
    StoreError::SchemaTooOld { .. } => Message::new("errors.olderSchema"),
    StoreError::Unrepresentable(_) => Message::new("errors.unsupportedPath"),
    StoreError::DuplicateConnectionName(_) => Message::new("errors.connectionNameTaken"),
    StoreError::DuplicateSecretName { .. } => Message::new("errors.secretNameTaken"),
    StoreError::AlreadyMerged(_) => Message::new("runs.merge.failure.alreadyMerged"),
    StoreError::TaskNotEditable { source, .. } => edit_error(source),
    StoreError::TaskGroupReviewChanged(_) => Message::new("tasks.bulk.reviewChanged"),
    StoreError::TaskNotSelectable { problem, .. } => crate::tasks::group::problem_message(problem),
    StoreError::PipelineNotPublished(_) => Message::new("runs.preflight.pipelineNotPublished"),
    StoreError::TaskNotHeld(_) => Message::new("tasks.bulk.notHeld"),
    StoreError::SharedTask(_) | StoreError::SharedProject => Message::new("sharing.errors.serverOnly"),
    StoreError::NotSharedTask(_) => Message::new("sharing.notShared.readOnly"),
    StoreError::NotShared => Message::new("sharing.errors.notShared"),
    StoreError::AlreadyShared => Message::new("sharing.errors.alreadyShared"),
    StoreError::Sqlite(_)
    | StoreError::Io { .. }
    | StoreError::Corrupt { .. }
    | StoreError::TaskNotFound(_)
    | StoreError::RunNotFound(_)
    | StoreError::EmptyBatch
    | StoreError::ResourceNotFound(_)
    | StoreError::ResourceKindMismatch { .. }
    | StoreError::DuplicateSlug { .. }
    | StoreError::NoDraft(_)
    | StoreError::ConnectionNotFound(_)
    | StoreError::NotAGitHubConnection(_)
    | StoreError::NotAnAgentCliConnection(_)
    | StoreError::InvocationNotFound(_)
    | StoreError::InvocationPermissionsNotRecordable(_)
    | StoreError::SecretNotFound(_)
    | StoreError::ServerNotFound(_)
    | StoreError::MemoryNotFound(_)
    | StoreError::MemoryExists(_)
    | StoreError::MemoryConflict(_)
    | StoreError::MemoryNotActive(_)
    | StoreError::DuplicateMemoryKey(_)
    | StoreError::MigrationBrokeReferences { .. } => Message::new("errors.storage"),
    StoreError::Publish(PublishError::StaleDraft { latest, .. }) => Message::new("errors.staleDraft").with(
      "version",
      latest.map_or_else(String::new, |version| version.to_string()),
    ),
    StoreError::Publish(PublishError::VersionLimit) => Message::new("errors.versionLimit"),
    StoreError::Publish(PublishError::NoSteps | PublishError::UnassignedSteps(_)) => {
      Message::new("pipelines.problems.notPublishable")
    }
    StoreError::Publish(PublishError::BlankInstruction) => Message::new("instructions.errors.blankText"),
    StoreError::Publish(PublishError::BlankSkillDescription) => Message::new("skills.errors.blankDescription"),
    StoreError::Publish(PublishError::BlankSkillBody) => Message::new("skills.errors.blankBody"),
    StoreError::Publish(PublishError::ConnectionMissing) => {
      Message::new("agents.permissions.publish.connectionMissing")
    }
    StoreError::Publish(PublishError::Permissions(_)) => Message::new("agents.permissions.publish.notEnforceable"),
    StoreError::AlwaysAllow(_) => Message::new("errors.alwaysAllow"),
  }
}

/// Why a task can no longer be edited.
pub const fn edit_error(error: &EditError) -> Message {
  match error {
    EditError::QueuedWithoutRun => Message::new("tasks.edit.notStarted"),
    EditError::Incomplete(_) => Message::new("tasks.edit.incomplete"),
  }
}

/// A secret that could not be added, replaced or archived. Neither the error nor the message holds
/// the value: keychain details go to the log only.
pub fn secret_error(error: &SecretError<StoreError>) -> Message {
  match error {
    SecretError::Taken { .. } => Message::new("errors.secretNameTaken"),
    SecretError::NotFound(_) | SecretError::Archived(_) => Message::new("errors.secretGone"),
    SecretError::Vault(error) => vault_error(error),
    SecretError::Store(error) | SecretError::ValueLeftBehind { source: error, .. } => store_error(error),
    SecretError::RotationNotRecorded { .. } => Message::new("errors.secretRotationNotRecorded"),
  }
}

pub fn vault_error(error: &VaultError) -> Message {
  match error {
    VaultError::Unsupported => Message::new("errors.keychainUnsupported"),
    VaultError::AccessDenied(_) => Message::new("errors.keychainDenied"),
    VaultError::TooLong { max } => Message::new("errors.secretTooLong").with("max", max.to_string()),
    VaultError::Failed(_) | VaultError::NotFound => Message::new("errors.keychainFailed"),
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

/// The detail of 02.H (`Rw6xK` in `EFY7I`): "Tried 5 times since 14:02. Interrupted runs keep
/// their last checkpoint. Nothing runs until the runner starts." `since` is the local time the
/// runner dropped, when known.
pub fn runner_not_restarted(attempts: u32, since: Option<&str>) -> Message {
  let key = match (attempts, since) {
    (1, Some(_)) => "runner.connection.notRestartedSince.one",
    (_, Some(_)) => "runner.connection.notRestartedSince.other",
    (1, None) => "runner.connection.notRestarted.one",
    (_, None) => "runner.connection.notRestarted.other",
  };
  let message = Message::new(key).with("attempts", attempts.to_string());
  match since {
    Some(time) => message.with("time", time.to_owned()),
    None => message,
  }
}
