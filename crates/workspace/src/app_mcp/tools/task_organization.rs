//! The organization `create_task` and `update_task` give a task (ORC-58), as the task form gives it: its workflow
//! status and priority, the manual Blocked with its reason, the tasks that block it and its tags. A blocking task is
//! named by its key or id among the project's own tasks, with the gate the form offers; a tag by its name, matched
//! ignoring case as the form's tag picker matches it, and created when the project has none of that name, as the
//! picker creates it. The rules stay core's and the store's: a Blocked reason is not blank, and a link closes no
//! cycle.

use orchester_core::{
  name::Name,
  task::organization::{BlockingTask, DependencyGate, TaskOrganization},
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

use super::{
  task_write::write_error,
  tasks::find_task,
  wire::{Rank, Workflow},
};
use crate::{app_mcp::error::ToolError, messages, tasks::writer::TaskWriter};

/// A task that blocks the task: it cannot run until this one is finished.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct Blocker {
  /// The blocking task's key, e.g. `KON-12`, or its id; a task of the same project.
  pub task: String,
  /// When the blocking task is finished: `merge`, once its work is merged, as the task form links tasks; left
  /// out, `merge`.
  #[serde(default)]
  pub gate: Option<Gate>,
}

/// When a blocking task is finished, among the gates the task form offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Gate {
  Merge,
}

impl From<Gate> for DependencyGate {
  fn from(gate: Gate) -> Self {
    match gate {
      Gate::Merge => Self::Merge,
    }
  }
}

/// The organization a call changes; each part left out (`None`) stays as it is.
pub(super) struct Changes<'a> {
  pub status: Option<Workflow>,
  pub priority: Option<Rank>,
  /// A reason sets the manual Blocked, `Some(None)` clears it.
  pub blocked_reason: Option<Option<&'a str>>,
  /// The whole list of blocking tasks; empty clears it.
  pub blocked_by: Option<&'a [Blocker]>,
  /// The whole list of tag names; empty clears it.
  pub tags: Option<&'a [String]>,
  /// *Don't merge automatically* (ORC-69): the Orchestrator never merges the task's runs on its own.
  pub no_automatic_merge: Option<bool>,
}

impl Changes<'_> {
  /// `base` with these changes but the tags ([`Self::tagged`]), its blocking tasks named among those of `db`.
  pub(super) fn apply(&self, db: &ProjectDb, base: &TaskOrganization) -> Result<TaskOrganization, ToolError> {
    let reason = match self.blocked_reason {
      Some(given) => given.map(str::to_owned),
      None => base.blocked_reason().map(str::to_owned),
    };
    let dependencies = match self.blocked_by {
      Some(named) => named
        .iter()
        .map(|blocker| {
          Ok(BlockingTask {
            task: find_task(db, &blocker.task)?.id(),
            gate: blocker.gate.unwrap_or(Gate::Merge).into(),
          })
        })
        .collect::<Result<_, ToolError>>()?,
      None => base.dependencies().to_vec(),
    };
    let organization = TaskOrganization::new(
      self.status.map_or(base.status(), Into::into),
      self.priority.map_or(base.priority(), Into::into),
      reason,
      dependencies,
      base.tags().to_vec(),
    )
    .map_err(StoreError::from)?;
    Ok(organization.with_no_automatic_merge(self.no_automatic_merge.unwrap_or(base.no_automatic_merge())))
  }

  /// `organization` with the tags named, those the project lacks created through `writer`; as it was when no tags
  /// were given. Every name is checked before any tag is created.
  pub(super) fn tagged(
    &self,
    writer: &mut TaskWriter,
    organization: TaskOrganization,
  ) -> Result<TaskOrganization, ToolError> {
    let Some(given) = self.tags else {
      return Ok(organization);
    };
    let names = given
      .iter()
      .map(|named| Name::new(named))
      .collect::<Result<Vec<_>, _>>()
      .map_err(|error| ToolError::refused("invalid_tag", vec![messages::name_error(&error)]))?;
    let mut known = writer.db().project_tags()?;
    let mut tags = Vec::new();
    for name in names {
      let lowered = name.as_str().to_lowercase();
      let tag = match known.iter().find(|tag| tag.name().as_str().to_lowercase() == lowered) {
        Some(tag) => tag.clone(),
        None => {
          let created = writer.create_tag(name).map_err(write_error)?;
          known.push(created.clone());
          created
        }
      };
      tags.push(tag.id());
    }
    Ok(organization.with_tags(tags))
  }
}
