//! What the caller may do in a workspace (S10): `{project_id, revision, role, actions, unresolved}`. Capabilities are
//! advisory: they are computed from the server's records for the reader, and every command checks again.

use orchester_core::{id::ProjectId, sharing::TaskRevision};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{DecisionId, LifecycleAction, Subject, page::nullable};

/// The caller's standing in the workspace: a member, a workspace admin, or a server admin (who need not be a member).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerRole {
  Member,
  Admin,
  ServerAdmin,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ActionError {
  #[error("a command kind is dotted lowercase words, such as claim.acquire, got {0:?}")]
  NotACommandKind(String),
  #[error("{0:?} is an action of its own, not a command kind")]
  Reserved(String),
}

const READ: &str = "read";
const EXPORT_SHARED: &str = "export.shared";
const SERVER_ADMIN: &str = "server.admin";

/// A command's `kind`, e.g. `claim.acquire`. Commands have no public payload types yet (S4): each later stage adds
/// its own, so a kind is any dotted list of lowercase words, `[a-z_]+(.[a-z_]+)*`, but the actions that are no command.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CommandKind(String);

impl CommandKind {
  pub fn new(kind: String) -> Result<Self, ActionError> {
    if [READ, EXPORT_SHARED, SERVER_ADMIN].contains(&kind.as_str()) || LifecycleAction::from_wire(&kind).is_some() {
      return Err(ActionError::Reserved(kind));
    }
    let word = |word: &str| !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'_');
    if !kind.split('.').all(word) {
      return Err(ActionError::NotACommandKind(kind));
    }
    Ok(Self(kind))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// What an action is: a command, or one of the reads, admin actions and task lifecycle changes that are no command. On
/// the wire a string: the command's kind, `read`, `export.shared`, `server.admin` or a [`LifecycleAction`]'s name
/// (`task.complete`, `task.cancel`, `task.reopen`, `task.triage`, `task.block`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Action {
  Read,
  ExportShared,
  ServerAdmin,
  /// A change of a task's lifecycle, which travels in the task patch; its subject is the task.
  Task(LifecycleAction),
  Command(CommandKind),
}

impl Action {
  pub fn as_str(&self) -> &str {
    match self {
      Self::Read => READ,
      Self::ExportShared => EXPORT_SHARED,
      Self::ServerAdmin => SERVER_ADMIN,
      Self::Task(action) => action.as_str(),
      Self::Command(kind) => kind.as_str(),
    }
  }
}

impl TryFrom<String> for Action {
  type Error = ActionError;

  fn try_from(action: String) -> Result<Self, Self::Error> {
    match action.as_str() {
      READ => Ok(Self::Read),
      EXPORT_SHARED => Ok(Self::ExportShared),
      SERVER_ADMIN => Ok(Self::ServerAdmin),
      _ => match LifecycleAction::from_wire(&action) {
        Some(task) => Ok(Self::Task(task)),
        None => CommandKind::new(action).map(Self::Command),
      },
    }
  }
}

impl From<Action> for String {
  fn from(action: Action) -> Self {
    match action {
      Action::Command(kind) => kind.0,
      other => other.as_str().to_owned(),
    }
  }
}

/// Why an action is not allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenialReason {
  RoleRequired,
  ClaimRequired,
  ClaimLost,
  ModuleRequired,
  DecisionRequired,
  ConfigurationInvalid,
  /// The subject is not in a state the action applies to: a Done task cannot be marked Done, an open one cannot be
  /// reopened. Told before who the caller is.
  TransitionDenied,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum CapabilityShapeError {
  #[error("an allowed action has no reason")]
  ReasonWhenAllowed,
  #[error("a denied action says why")]
  NoReasonWhenDenied,
}

/// `{action, subject:Subject?, allowed, reason:DenialReason?}`: whether the caller may take `action`, on `subject`
/// when it acts on one. `reason` is `null` exactly when the action is allowed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ActionCapabilityWire", into = "ActionCapabilityWire")]
pub struct ActionCapability {
  action: Action,
  subject: Option<Subject>,
  reason: Option<DenialReason>,
}

impl ActionCapability {
  pub const fn allowed(action: Action, subject: Option<Subject>) -> Self {
    Self {
      action,
      subject,
      reason: None,
    }
  }

  pub const fn denied(action: Action, subject: Option<Subject>, reason: DenialReason) -> Self {
    Self {
      action,
      subject,
      reason: Some(reason),
    }
  }

  pub const fn action(&self) -> &Action {
    &self.action
  }

  pub const fn subject(&self) -> Option<Subject> {
    self.subject
  }

  pub const fn is_allowed(&self) -> bool {
    self.reason.is_none()
  }

  /// Why it is denied; `None` when it is allowed.
  pub const fn reason(&self) -> Option<DenialReason> {
    self.reason
  }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionCapabilityWire {
  action: Action,
  #[serde(deserialize_with = "nullable")]
  subject: Option<Subject>,
  allowed: bool,
  #[serde(deserialize_with = "nullable")]
  reason: Option<DenialReason>,
}

impl TryFrom<ActionCapabilityWire> for ActionCapability {
  type Error = CapabilityShapeError;

  fn try_from(wire: ActionCapabilityWire) -> Result<Self, Self::Error> {
    match (wire.allowed, wire.reason) {
      (true, None) => Ok(Self::allowed(wire.action, wire.subject)),
      (false, Some(reason)) => Ok(Self::denied(wire.action, wire.subject, reason)),
      (true, Some(_)) => Err(CapabilityShapeError::ReasonWhenAllowed),
      (false, None) => Err(CapabilityShapeError::NoReasonWhenDenied),
    }
  }
}

impl From<ActionCapability> for ActionCapabilityWire {
  fn from(capability: ActionCapability) -> Self {
    Self {
      allowed: capability.is_allowed(),
      action: capability.action,
      subject: capability.subject,
      reason: capability.reason,
    }
  }
}

/// The caller's capabilities in project `project_id` at `revision` (a revision is 1..=`i64::MAX`, as a task's), and
/// the team-authority decisions still open that hold some actions back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
  pub project_id: ProjectId,
  pub revision: TaskRevision,
  pub role: CallerRole,
  pub actions: Vec<ActionCapability>,
  pub unresolved: Vec<DecisionId>,
}
