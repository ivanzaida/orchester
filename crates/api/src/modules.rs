//! A shared project's modules (`docs/architecture/server.md`, "Modules"): what a person sets of a module, the module as
//! the server keeps it under a revision, and every move of a task into or out of one. Which module a task is in is a
//! field of the task ([`crate::TaskContent::module`]), changed under the task's revision.

use chrono::NaiveDate;
use orchester_core::{
  id::{IdempotencyKey, ModuleId, TaskId},
  module::{ModuleDetails, ModuleError, ModuleState},
  name::Name,
  sharing::TaskRevision,
  task::organization::ChangeActor,
};
use serde::{Deserialize, Serialize};

use crate::{Attribution, UserRef, wire};

/// What a person sets of a module; its days are `YYYY-MM-DD`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleFields {
  pub name: String,
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub start: Option<String>,
  #[serde(default)]
  pub target: Option<String>,
  #[serde(default, with = "wire::ModuleStateWire")]
  pub state: ModuleState,
}

/// Why fields are not a module's details.
#[derive(Debug, PartialEq, Eq)]
pub enum FieldsError {
  /// A name that is not a name, or a day that is not `YYYY-MM-DD`.
  Invalid,
  /// Core's rules refused them.
  Module(ModuleError),
}

impl ModuleFields {
  /// The details these fields are, by core's rules.
  pub fn details(&self) -> Result<ModuleDetails, FieldsError> {
    let name = Name::new(&self.name).map_err(|_| FieldsError::Invalid)?;
    let days = (day(self.start.as_deref())?, day(self.target.as_deref())?);
    ModuleDetails::new(name, &self.description, days, self.state).map_err(FieldsError::Module)
  }

  pub fn from_core(details: &ModuleDetails) -> Self {
    Self {
      name: details.name().as_str().to_owned(),
      description: details.description().to_owned(),
      start: details.start().map(|day| day.to_string()),
      target: details.target().map(|day| day.to_string()),
      state: details.state(),
    }
  }
}

fn day(value: Option<&str>) -> Result<Option<NaiveDate>, FieldsError> {
  value
    .map(|day| day.parse::<NaiveDate>().map_err(|_| FieldsError::Invalid))
    .transpose()
}

/// A module as the server keeps it. Its revision counts its changes as a task's does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedModule {
  pub id: ModuleId,
  #[serde(flatten)]
  pub fields: ModuleFields,
  pub revision: TaskRevision,
  pub created: Attribution,
  pub updated: Attribution,
}

/// A task moved into a module, out of one, or from one to another; `sequence` orders every move of the server's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedModuleChange {
  pub sequence: u64,
  pub task: TaskId,
  pub from: Option<ModuleId>,
  pub to: Option<ModuleId>,
  pub by: UserRef,
  #[serde(with = "wire::Actor")]
  pub actor: ChangeActor,
  pub at: i64,
}

/// `POST …/modules`: a module with the id the desktop made for it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleCreate {
  pub idempotency_key: IdempotencyKey,
  pub id: ModuleId,
  pub module: ModuleFields,
}

/// `PATCH …/modules/{module}`: the module's fields as a whole, made on `revision`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePatch {
  pub idempotency_key: IdempotencyKey,
  pub revision: TaskRevision,
  pub module: ModuleFields,
}

/// A local project's module, uploaded when the project is shared or joins: it keeps its id, or becomes the project's
/// module of its name.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadModule {
  pub id: ModuleId,
  pub module: ModuleFields,
  pub created_at: i64,
}

/// An uploaded module that became the project's module of its name: the upload's tasks of `from` are in `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappedModule {
  pub from: ModuleId,
  pub to: ModuleId,
}

#[derive(Serialize, Deserialize)]
pub struct ModuleResponse {
  pub module: SharedModule,
}

/// `GET …/modules?after=`: every module of the project, and the moves after the sequence `after`.
#[derive(Serialize, Deserialize)]
pub struct Modules {
  pub modules: Vec<SharedModule>,
  pub changes: Vec<SharedModuleChange>,
}

/// A `modules` event: the project's modules after a create, a change or an upload.
#[derive(Serialize, Deserialize)]
pub struct ModulesChanged {
  pub modules: Vec<SharedModule>,
}
