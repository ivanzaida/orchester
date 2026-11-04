//! A local project's actions, carried out by its task writer exactly as its screens and tools carry them out today.
//! A new task's batch key is the intent's operation id, so sending the same intent again saves it once.

use orchester_core::{
  id::{IdempotencyKey, OperationId},
  store::TaskStore,
  task::organization::{ChangeActor, TaskOrganization},
  time::Timestamp,
};

use super::{
  ActionError, LocalResult,
  action::{Action, ModuleAction, TagAction, TaskAction},
};
use crate::tasks::{data::SaveTarget, writer::TaskWriter};

/// Carries out `action` on the local project of `writer`; a team-only action never reaches here.
pub(super) fn run(
  writer: &mut TaskWriter,
  operation: OperationId,
  action: &Action,
) -> Result<LocalResult, ActionError> {
  match action {
    Action::Task(action) => task(writer, operation, action),
    Action::Module(action) => module(writer, action),
    Action::Tag(action) => tag(writer, action),
    Action::Claim(_) | Action::Assignment(_) | Action::Policy(_) | Action::Delivery(_) => Err(ActionError::TeamOnly),
  }
}

fn task(writer: &mut TaskWriter, operation: OperationId, action: &TaskAction) -> Result<LocalResult, ActionError> {
  let result = match action {
    TaskAction::Create { details, organization } => {
      let batch = IdempotencyKey::from_uuid(operation.as_uuid());
      let base = TaskOrganization::default();
      let saved = writer.save_organized(
        SaveTarget::New(batch),
        details.clone(),
        (&base, organization),
        Timestamp::now(),
      )?;
      LocalResult::Task(Box::new(saved))
    }
    TaskAction::Edit { task, read, edited, .. } => {
      LocalResult::Task(Box::new(writer.update_task(*task, *read, edited.clone())?))
    }
    TaskAction::Organize { task, edited, .. } => {
      let edited = edited.clone();
      LocalResult::Changed(writer.update_organization_by(*task, ChangeActor::Person, |_| edited)?)
    }
    TaskAction::Move { task, module, .. } => {
      LocalResult::Changed(writer.set_task_module(*task, *module, ChangeActor::Person)?)
    }
  };
  Ok(result)
}

fn module(writer: &mut TaskWriter, action: &ModuleAction) -> Result<LocalResult, ActionError> {
  match action {
    ModuleAction::Create { details } => Ok(LocalResult::Module(writer.create_module(details.clone())?)),
    ModuleAction::Edit {
      module, base, edited, ..
    } => Ok(LocalResult::Module(writer.update_module(
      *module,
      base,
      edited.clone(),
    )?)),
    ModuleAction::AssignBranch { .. } => Err(ActionError::TeamOnly),
  }
}

fn tag(writer: &mut TaskWriter, action: &TagAction) -> Result<LocalResult, ActionError> {
  match action {
    TagAction::Create { name } => return Ok(LocalResult::Tag(writer.create_tag(name.clone())?)),
    TagAction::Rename { tag, name } => writer.rename_tag(*tag, name)?,
    TagAction::Merge { tag, into } => writer.merge_tags(*tag, *into)?,
    TagAction::Delete { tag } => writer.delete_tag(*tag)?,
  }
  Ok(LocalResult::Tags)
}
