//! A team project's definitions (task edits, modules, tags and the workspace policy) are no commands: the team server
//! changes them on their own Admin-checked routes (Authority S3, `docs/cloud-api.md`, "Team authority P2 commands"),
//! each decided on the revision the person read. This module builds the edit for its route from the action, checked
//! here without any effect; [`send`] carries it out. A task's pipeline is one of the team's, named by its resource id
//! (`docs/architecture/server.md` row 80, decided 2026-11-05). A change of something whose owner is an open decision
//! (a task's limits, D6) is refused before it is sent, never left out silently. A workflow status change is the
//! lifecycle (D4): the same patch, which the server decides by the caller's claim and role.

mod send;

use orchester_api::{
  Blocking, ModuleCreate, ModuleFields, ModulePatch, TagMerge, TagName, TaskChanges, TaskPatch,
  v2::{DecisionId, LifecycleAction, WorkspacePolicyChanges},
};
use orchester_core::{
  id::{IdempotencyKey, ModuleId, OperationId, TagId, TaskId},
  module::ModuleDetails,
  name::Name,
  sharing::TaskRevision,
  task::{TaskDetails, organization::TaskOrganization},
};
use orchester_store::ProjectDb;

pub(super) use self::send::carry_out;
use super::{
  ActionError,
  action::{PolicyAction, TagAction},
  command::TeamPipelineCheck,
  ownership::{TeamLink, team_task},
};

/// One definition edit and the route it goes to, under the team project's `/api/v1` (the policy's under `/api/v2`).
pub(super) enum DefinitionEdit {
  /// `PATCH …/tasks/{task}`, on the task's revision.
  Task {
    task: TaskId,
    patch: TaskPatch,
  },
  /// `POST …/modules`, keyed by the intent's operation, which also makes the module's id.
  CreateModule(ModuleCreate),
  /// `PATCH …/modules/{module}`, the module's fields as a whole on its revision, keyed by the intent's operation.
  Module {
    module: ModuleId,
    patch: ModulePatch,
  },
  Tag(TagEdit),
  /// `PATCH /api/v2/projects/{project}/policy`, on the policy's revision.
  Policy(WorkspacePolicyChanges),
}

/// A tag edit; tags have no revision.
pub(super) enum TagEdit {
  /// `POST …/tags`.
  Create(TagName),
  /// `PATCH …/tags/{tag}`.
  Rename { tag: TagId, name: TagName },
  /// `POST …/tags/{tag}/merge`.
  Merge { tag: TagId, into: TagMerge },
  /// `DELETE …/tags/{tag}`.
  Delete(TagId),
}

/// The edit of `task` read at `read`: only the fields that changed.
pub(super) fn task(
  db: &ProjectDb,
  link: TeamLink,
  (task, read): (TaskId, TaskRevision),
  changes: TaskChanges,
) -> Result<DefinitionEdit, ActionError> {
  Ok(DefinitionEdit::Task {
    task: team_task(db, link, task)?,
    patch: TaskPatch {
      revision: read,
      changes,
    },
  })
}

/// A task moved into `module`, or out of its module.
pub(super) fn moved(module: Option<ModuleId>) -> TaskChanges {
  TaskChanges {
    module: Some(module),
    ..TaskChanges::default()
  }
}

/// The details that changed; the limits' owner is open (D6), and a pipeline is a team pipeline, by its id.
pub(super) fn detail_changes(
  pipelines: &TeamPipelineCheck<'_>,
  base: &TaskDetails,
  edited: &TaskDetails,
) -> Result<TaskChanges, ActionError> {
  if base.limits != edited.limits {
    return Err(ActionError::DecisionRequired(DecisionId::D6));
  }
  let pipeline = changed(&base.pipeline, &edited.pipeline)
    .map(|pipeline| pipeline.map(|pipeline| pipelines.named(pipeline)).transpose())
    .transpose()?;
  some(TaskChanges {
    title: changed(&base.title, &edited.title).map(|title| title.as_str().to_owned()),
    brief: changed(&base.brief, &edited.brief).cloned(),
    criteria: changed(&base.acceptance_criteria, &edited.acceptance_criteria).cloned(),
    pipeline,
    ..TaskChanges::default()
  })
}

/// The organization fields that changed. A workflow status change is sent when the lifecycle has it (D4:
/// `LifecycleAction::of_workflow`); the others (In progress and In review, Done to Canceled) are not a person's here.
pub(super) fn organization_changes(
  base: &TaskOrganization,
  edited: &TaskOrganization,
) -> Result<TaskChanges, ActionError> {
  let workflow = changed(&base.status(), &edited.status())
    .map(|&to| match LifecycleAction::of_workflow(base.status(), to) {
      Some(_) => Ok(to),
      None => Err(ActionError::WorkflowNotOffered {
        from: base.status(),
        to,
      }),
    })
    .transpose()?;
  let blocking = changed(base.dependencies(), edited.dependencies()).map(|links| {
    links
      .iter()
      .map(|link| Blocking {
        task: link.task,
        gate: link.gate,
      })
      .collect()
  });
  some(TaskChanges {
    workflow,
    priority: changed(&base.priority(), &edited.priority()).copied(),
    tags: changed(base.tags(), edited.tags()).map(<[_]>::to_vec),
    blocking,
    blocked_reason: changed(&base.blocked_reason(), &edited.blocked_reason()).map(|reason| reason.map(str::to_owned)),
    ..TaskChanges::default()
  })
}

/// `edited` when it differs from `base`.
fn changed<'a, T: PartialEq + ?Sized>(base: &T, edited: &'a T) -> Option<&'a T> {
  (base != edited).then_some(edited)
}

/// `changes`, unless they set nothing.
fn some(changes: TaskChanges) -> Result<TaskChanges, ActionError> {
  let nothing = changes.workflow.is_none()
    && changes.title.is_none()
    && changes.brief.is_none()
    && changes.criteria.is_none()
    && changes.pipeline.is_none()
    && changes.priority.is_none()
    && changes.tags.is_none()
    && changes.blocking.is_none()
    && changes.blocked_reason.is_none()
    && changes.module.is_none();
  if nothing {
    return Err(ActionError::NothingToChange);
  }
  Ok(changes)
}

/// A new module, keyed by the intent's `operation`, which also makes its id: the same intent sent again answers the
/// first module rather than making a second.
pub(super) fn create_module(operation: OperationId, details: &ModuleDetails) -> DefinitionEdit {
  DefinitionEdit::CreateModule(ModuleCreate {
    idempotency_key: IdempotencyKey::from_uuid(operation.as_uuid()),
    id: ModuleId::from_uuid(operation.as_uuid()),
    module: ModuleFields::from_core(details),
  })
}

/// `module` read as `base` at `read`, changed to `edited`, keyed by the intent's `operation`.
pub(super) fn edit_module(
  operation: OperationId,
  (module, read): (ModuleId, TaskRevision),
  base: &ModuleDetails,
  edited: &ModuleDetails,
) -> Result<DefinitionEdit, ActionError> {
  let fields = ModuleFields::from_core(edited);
  if ModuleFields::from_core(base) == fields {
    return Err(ActionError::NothingToChange);
  }
  Ok(DefinitionEdit::Module {
    module,
    patch: ModulePatch {
      idempotency_key: IdempotencyKey::from_uuid(operation.as_uuid()),
      revision: read,
      module: fields,
    },
  })
}

pub(super) fn tag(action: &TagAction) -> DefinitionEdit {
  let named = |name: &Name| TagName {
    name: name.as_str().to_owned(),
  };
  DefinitionEdit::Tag(match action {
    TagAction::Create { name } => TagEdit::Create(named(name)),
    TagAction::Rename { tag, name } => TagEdit::Rename {
      tag: *tag,
      name: named(name),
    },
    TagAction::Merge { tag, into } => TagEdit::Merge {
      tag: *tag,
      into: TagMerge { into: *into },
    },
    TagAction::Delete { tag } => TagEdit::Delete(*tag),
  })
}

pub(super) const fn policy(action: PolicyAction) -> DefinitionEdit {
  DefinitionEdit::Policy(WorkspacePolicyChanges {
    revision: action.read,
    automatic_merge: action.automatic_merge,
  })
}
