use orchester_core::id::ProjectId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
  Task,
  Members,
  Tags,
  Project,
  /// The project's modules after one was made or changed ([`crate::ModulesChanged`]).
  Modules,
  Reset,
  Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownEventKind;

impl EventKind {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Task => "task",
      Self::Members => "members",
      Self::Tags => "tags",
      Self::Project => "project",
      Self::Modules => "modules",
      Self::Reset => "reset",
      Self::Revoked => "revoked",
    }
  }
}
impl TryFrom<&str> for EventKind {
  type Error = UnknownEventKind;
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "task" => Ok(Self::Task),
      "members" => Ok(Self::Members),
      "tags" => Ok(Self::Tags),
      "project" => Ok(Self::Project),
      "modules" => Ok(Self::Modules),
      "reset" => Ok(Self::Reset),
      "revoked" => Ok(Self::Revoked),
      _ => Err(UnknownEventKind),
    }
  }
}

/// Project events carry common metadata, never the acting member's personal role.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectUpdated {
  pub id: ProjectId,
  pub key: String,
  pub name: String,
}
