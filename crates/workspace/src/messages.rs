//! User-facing messages for errors from core and store: an i18n key plus its placeholder
//! values. Errors are mapped here, at the UI boundary; their `Display` text is for logs only.

use orchester_core::{
  name::{Name, NameError},
  resource::PublishError,
  secret::{SecretError, VaultError},
  service::{NeedsError, ServiceName},
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

pub fn module_error(error: &orchester_core::module::ModuleError) -> Message {
  use orchester_core::module::ModuleError;
  Message::new(match error {
    ModuleError::TargetBeforeStart => "modules.errors.targetBeforeStart",
    ModuleError::DuplicateName => "modules.errors.duplicateName",
    ModuleError::NotFound(_) => "modules.errors.notFound",
    ModuleError::ChangedMeanwhile => "modules.errors.changedMeanwhile",
  })
}

pub fn store_error(error: &StoreError) -> Message {
  match error {
    StoreError::Organization(error) => organization_error(error),
    StoreError::Module(error) => module_error(error),
    StoreError::NotAGitRepository(_) => Message::new("errors.notAGitRepository"),
    StoreError::AlreadyInitialized(_) => Message::new("errors.alreadyInitialized"),
    StoreError::NotInitialized(_) => Message::new("errors.projectMissing"),
    StoreError::ProjectOfAnotherAccount(_) => Message::new("errors.anotherAccountsProject"),
    StoreError::SchemaTooNew { .. } => Message::new("errors.newerSchema"),
    StoreError::SchemaTooOld { .. } => Message::new("errors.olderSchema"),
    StoreError::UnrepresentablePath(_) => Message::new("errors.unsupportedPath"),
    StoreError::Unrepresentable(_) => Message::new("errors.unstorableValue"),
    StoreError::DuplicateConnectionName(_) => Message::new("errors.connectionNameTaken"),
    StoreError::DuplicateAccountName(name) => Message::new("accounts.errors.nameTaken").with("name", name.clone()),
    StoreError::AccountFolderTaken => Message::new("accounts.errors.folderTaken"),
    StoreError::DuplicateSecretName { .. } => Message::new("errors.secretNameTaken"),
    StoreError::AlreadyMerged(_) => Message::new("runs.merge.failure.alreadyMerged"),
    StoreError::NotMergeable(_) => Message::new("runs.merge.failure.notMergeable"),
    StoreError::AlreadyCompleted(_) => Message::new("runs.complete.failure.alreadyCompleted"),
    StoreError::NotCompletable(_) => Message::new("runs.complete.failure.notCompletable"),
    StoreError::TaskNotEditable { source, .. } => edit_error(source),
    StoreError::TaskGroupReviewChanged(_) => Message::new("tasks.bulk.reviewChanged"),
    StoreError::TaskNotSelectable { problem, .. } => crate::tasks::group::problem_message(problem),
    StoreError::PipelineNotPublished(_) => Message::new("runs.preflight.pipelineNotPublished"),
    StoreError::TaskNotHeld(_) => Message::new("tasks.bulk.notHeld"),
    StoreError::TaskRunActive(_) => Message::new("tasks.delete.stopRunFirst"),
    StoreError::SharedTask(_) | StoreError::SharedProject => Message::new("sharing.errors.serverOnly"),
    StoreError::NotAdmin => Message::new("sharing.errors.notAdmin"),
    StoreError::NotSharedTask(_) => Message::new("sharing.notShared.readOnly"),
    StoreError::NotShared => Message::new("sharing.errors.notShared"),
    StoreError::AlreadyShared => Message::new("sharing.errors.alreadyShared"),
    StoreError::DraftChanged(_) => Message::new("errors.draftChanged"),
    StoreError::VersionInUse(_) => Message::new("sharing.errors.versionInUse"),
    StoreError::NotPublished(_) => Message::new("errors.neverPublished"),
    StoreError::Sqlite(_) if error.is_locked() => Message::new("errors.databaseLocked"),
    StoreError::Unavailable { .. } => Message::new("errors.databaseLocked"),
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
    | StoreError::AgentAccountNotFound(_)
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
    StoreError::Publish(PublishError::Permissions(problem)) => crate::agents::permissions::not_enforceable(problem),
    StoreError::Publish(PublishError::Needs(error)) => needs_error(error),
    StoreError::AlwaysAllow(_) => Message::new("errors.alwaysAllow"),
  }
}

/// Why a service or stack cannot be published: what it needs is not published, or the needs form a cycle.
pub fn needs_error(error: &NeedsError) -> Message {
  match error {
    NeedsError::DuplicateService(service) => {
      Message::new("services.errors.duplicateService").with("service", service.to_string())
    }
    NeedsError::Missing {
      name,
      needed_by: Some(service),
    } => Message::new("services.errors.needNotPublished")
      .with("service", service.to_string())
      .with("need", name.to_string()),
    NeedsError::Missing { name, needed_by: None } => {
      Message::new("services.errors.serviceNotPublished").with("service", name.to_string())
    }
    NeedsError::Cycle(cycle) => {
      let cycle: Vec<&str> = cycle.iter().map(ServiceName::as_str).collect();
      Message::new("services.errors.needsCycle").with("cycle", cycle.join(" → "))
    }
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

/// Why the secret store did not give or keep the app's MCP token, in its own short words: the store is named beside
/// them (*The token can't be read from Windows Credential Manager*, `eTrCO`).
pub fn secret_store_error(error: &VaultError) -> Message {
  match error {
    VaultError::AccessDenied(_) => Message::new(secret_store::DENIED),
    VaultError::Unsupported => Message::new("errors.keychainUnsupported"),
    VaultError::TooLong { max } => Message::new("errors.secretTooLong").with("max", max.to_string()),
    VaultError::Failed(_) | VaultError::NotFound => Message::new("errors.secretStoreFailed"),
  }
}

/// How each platform's secret store refuses: Windows Credential Manager says access is denied; the macOS keychain is
/// locked, or the person did not allow the app.
mod secret_store {
  #[cfg(target_os = "macos")]
  pub const DENIED: &str = "errors.secretStoreDeniedMacos";
  #[cfg(not(target_os = "macos"))]
  pub const DENIED: &str = "errors.secretStoreDenied";
}

/// A change only the installed app's MCP server takes (a fixed address, a stored token) was asked of a copy that
/// takes a new port and token on each start.
pub fn app_mcp_not_installation() -> Message {
  Message::new("errors.appMcpNotInstallation")
}

/// Work that ran on a background thread panicked or was cancelled; the cause is logged.
pub fn interrupted() -> Message {
  Message::new("errors.interrupted")
}

/// The repository's agent files could not be read; the cause is logged.
pub fn repository_unreadable() -> Message {
  Message::new("errors.repositoryUnreadable")
}

/// What kind of failure a message names, so a screen can say why in plain words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
  /// The project's data could not be read or written ([`store_error`]'s storage failures).
  Storage,
  /// The database is busy: a wait that ends on its own.
  Busy,
  /// The work stopped before it finished ([`interrupted`]).
  Interrupted,
  /// Anything else; its own words say why.
  Other,
}

pub fn failure_kind(message: &Message) -> FailureKind {
  match message.key {
    "errors.storage" => FailureKind::Storage,
    "errors.databaseLocked" => FailureKind::Busy,
    "errors.interrupted" => FailureKind::Interrupted,
    _ => FailureKind::Other,
  }
}

/// A task was changed since it was read, so a change made on what was read was not saved.
pub fn task_changed_meanwhile() -> Message {
  Message::new("tasks.conflict.changedMeanwhile")
}

/// A save of a new draft found its task queued meanwhile, so it was not saved as a draft.
pub fn task_no_longer_draft() -> Message {
  Message::new("tasks.new.noLongerDraft")
}

/// A save stored details that differ from the ones it was given.
pub fn task_saved_differently() -> Message {
  Message::new("tasks.new.savedDetailsDiffer")
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
