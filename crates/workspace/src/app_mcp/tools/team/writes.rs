//! `create_task` and `update_task` of a team project, through the dispatcher: a new task is the team server's
//! `task.create`, an edit its Admin-checked task route on the revision just read, so a member's edit is refused
//! `not_admin` and nothing is saved here. Only what the team server holds of a task is sent. A pipeline, limits or a
//! workflow status change wait for team-authority decisions (D6, D4) and are refused before anything is sent; what
//! the server does not hold of a team task (story points, tags and blocking tasks by name, Don't merge automatically,
//! delivers, the base branch) is refused rather than kept on this computer.
//!
//! The server takes a task's details, its organization and its module on separate routes, so one call may send up to
//! three changes, each on the revision the one before it left. Once one is saved, a later refusal does not undo it:
//! the tool answers the task as saved, with the changes `not_saved` and why, so a client never sends the saved part
//! again (a second task, for a create).

use orchester_api::{
  SharedTask,
  v2::{DecisionId, TeamCommandValue},
};
use orchester_core::{
  id::ModuleId,
  name::Name,
  task::{
    TaskDetails, TaskLimits,
    organization::{OrganizationError, TaskOrganization, WorkflowStatus},
  },
};
use orchester_store::StoreError;
use serde_json::{Value, json};

use super::{
  Team, on_worker,
  outcome::{accepted, action_refused},
  tasks::find,
};
use crate::{
  actions::{Accepted, Action, ActionError, ActionOutcome, Definition, Dispatcher, Intent, TaskAction},
  app_mcp::{
    Services,
    error::{Missing, ToolError},
    tools::{
      task_write::{CreateTask, UpdateTask},
      wire::{Rank, Workflow},
    },
  },
  messages::Message,
  tasks::form::title_error,
};

/// One change of a team task, sent on its own route.
enum Change {
  Details(TaskDetails, TaskDetails),
  Organization(TaskOrganization, TaskOrganization),
  Module(Option<ModuleId>),
}

impl Change {
  /// The action that makes this change of `task` as it was last saved.
  fn action(self, task: &SharedTask) -> TaskAction {
    let (task, read) = (task.id, task.revision);
    match self {
      Self::Details(base, edited) => TaskAction::Edit {
        task,
        read,
        base,
        edited,
      },
      Self::Organization(base, edited) => TaskAction::Organize {
        task,
        read,
        base,
        edited,
      },
      Self::Module(module) => TaskAction::Move { task, read, module },
    }
  }

  /// What the result names this change, when it was not saved.
  const fn name(&self) -> &'static str {
    match self {
      Self::Details(..) => "details",
      Self::Organization(..) => "organization",
      Self::Module(_) => "module",
    }
  }
}

/// The changes of a call the server did not save after an earlier one was, and why.
struct NotSaved {
  changes: Vec<&'static str>,
  error: ToolError,
}

pub(in crate::app_mcp::tools) async fn create_task(
  services: &Services,
  team: &Team,
  args: &CreateTask,
) -> Result<Value, ToolError> {
  let unsupported = [
    ("story_points", args.story_points.is_some()),
    ("tags", !args.tags.is_empty()),
    ("blocked_by", !args.blocked_by.is_empty()),
    ("no_automatic_merge", args.no_automatic_merge),
    ("delivers", args.delivers.is_some()),
    ("base_branch", args.base_branch.is_some()),
  ];
  refuse_unsupported(&unsupported)?;
  let limited = matches!(args.budget_tokens, Some(Some(_))) || matches!(args.time_limit_minutes, Some(Some(_)));
  if args.pipeline.is_some() || limited {
    return Err(action_refused(ActionError::DecisionRequired(DecisionId::D6)));
  }
  let status = args.workflow_status.map_or_else(WorkflowStatus::default, Into::into);
  if status != WorkflowStatus::default() {
    return Err(action_refused(ActionError::DecisionRequired(DecisionId::D4)));
  }
  let details = TaskDetails {
    title: title(&args.title)?,
    brief: args.brief.clone().unwrap_or_default(),
    acceptance_criteria: criteria(&args.criteria),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let organization = TaskOrganization::new(
    status,
    args.priority.map(Into::into).unwrap_or_default(),
    args.blocked_reason.clone(),
    Vec::new(),
    Vec::new(),
  )
  .map_err(organization_refused)?;
  let module = args.module.as_deref().map(module_id).transpose()?;
  let dispatcher = team.dispatcher(services).await?;
  let (task, not_saved) = on_worker(move || async move {
    let created = match carry_out(&dispatcher, TaskAction::Create { details, organization }).await? {
      Accepted::Command(receipt) => match receipt.result {
        TeamCommandValue::Task { task, .. } => *task,
        _ => return Err(not_a_task()),
      },
      Accepted::Definition(_) => return Err(not_a_task()),
    };
    let moves = module.map(|module| Change::Module(Some(module)));
    apply(&dispatcher, (created, true), moves.into_iter().collect()).await
  })
  .await?;
  Ok(answer(&task, not_saved))
}

pub(in crate::app_mcp::tools) async fn update_task(
  services: &Services,
  team: &Team,
  args: &UpdateTask,
) -> Result<Value, ToolError> {
  let unsupported = [
    ("story_points", args.story_points.is_some()),
    ("tags", args.tags.is_some()),
    ("blocked_by", args.blocked_by.is_some()),
    ("no_automatic_merge", args.no_automatic_merge.is_some()),
    ("delivers", args.delivers.is_some()),
    ("base_branch", args.base_branch.is_some()),
  ];
  refuse_unsupported(&unsupported)?;
  if args.pipeline.is_some() || args.budget_tokens.is_some() || args.time_limit_minutes.is_some() {
    return Err(action_refused(ActionError::DecisionRequired(DecisionId::D6)));
  }
  let module = match &args.module {
    Some(Some(named)) => Some(Some(module_id(named)?)),
    Some(None) => Some(None),
    None => None,
  };
  let task = find(&team.reads(services).await?, &args.key).await?.task;
  // Everything refused in advance is refused here, before the first change is sent.
  let mut changes = Vec::new();
  if let Some((base, edited)) = edited_details(&task, args)? {
    changes.push(Change::Details(base, edited));
  }
  if let Some((base, edited)) = organized(&task, args)? {
    changes.push(Change::Organization(base, edited));
  }
  if let Some(module) = module.filter(|module| *module != task.content.module) {
    changes.push(Change::Module(module));
  }
  let dispatcher = team.dispatcher(services).await?;
  let (task, not_saved) = on_worker(move || async move { apply(&dispatcher, (task, false), changes).await }).await?;
  Ok(answer(&task, not_saved))
}

/// `changes` of `task`, in order, each on the revision the one before it left; `saved` when `task` itself was just
/// saved (created). A refusal before anything was saved is the call's error; after, the task as saved and what was not.
async fn apply(
  dispatcher: &Dispatcher,
  (mut task, mut saved): (SharedTask, bool),
  changes: Vec<Change>,
) -> Result<(SharedTask, Option<NotSaved>), ToolError> {
  let mut changes = changes.into_iter();
  while let Some(change) = changes.next() {
    let name = change.name();
    match edit(dispatcher, change.action(&task)).await {
      Ok(next) => (task, saved) = (next, true),
      Err(error) if saved => {
        let changes = std::iter::once(name)
          .chain(changes.map(|change| change.name()))
          .collect();
        return Ok((task, Some(NotSaved { changes, error })));
      }
      Err(error) => return Err(error),
    }
  }
  Ok((task, None))
}

/// The result: the task as the server saved it, and the changes it did not save after it, with why.
fn answer(task: &SharedTask, not_saved: Option<NotSaved>) -> Value {
  let mut result = json!({ "task": saved(task) });
  if let Some(NotSaved { changes, error }) = not_saved {
    tracing::warn!(code = error.code(), "a team task was saved in part");
    result["not_saved"] = json!({ "changes": changes, "code": error.code() });
  }
  result
}

/// The details read of `task` and as `args` change them; `None` when they change none.
fn edited_details(task: &SharedTask, args: &UpdateTask) -> Result<Option<(TaskDetails, TaskDetails)>, ToolError> {
  if args.title.is_none() && args.brief.is_none() && args.criteria.is_none() {
    return Ok(None);
  }
  let content = &task.content;
  // Pipeline and limits are left out of both: they wait for D6, and an edit sends only what changed.
  let base = TaskDetails {
    title: title(&content.title)?,
    brief: content.brief.clone(),
    acceptance_criteria: content.criteria.clone(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let edited = TaskDetails {
    title: args.title.as_deref().map_or_else(|| Ok(base.title.clone()), title)?,
    brief: args.brief.clone().unwrap_or_else(|| base.brief.clone()),
    acceptance_criteria: args
      .criteria
      .as_deref()
      .map_or_else(|| base.acceptance_criteria.clone(), criteria),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  Ok((edited != base).then_some((base, edited)))
}

/// The organization read of `task` and as `args` change it; `None` when they change nothing of it. A workflow status
/// change is refused (D4) before any change of the call is sent.
fn organized(task: &SharedTask, args: &UpdateTask) -> Result<Option<(TaskOrganization, TaskOrganization)>, ToolError> {
  if args.workflow_status.is_none() && args.priority.is_none() && args.blocked_reason.is_none() {
    return Ok(None);
  }
  let base = task.content.organization().core().map_err(organization_refused)?;
  let status = args.workflow_status.map_or(base.status(), Into::into);
  if status != base.status() {
    return Err(action_refused(ActionError::DecisionRequired(DecisionId::D4)));
  }
  let edited = TaskOrganization::new(
    status,
    args.priority.map_or(base.priority(), Into::into),
    match &args.blocked_reason {
      Some(reason) => reason.clone(),
      None => base.blocked_reason().map(str::to_owned),
    },
    base.dependencies().to_vec(),
    base.tags().to_vec(),
  )
  .map_err(organization_refused)?;
  Ok((edited != base).then_some((base, edited)))
}

/// `action` through `dispatcher`: the task as the server holds it after it.
async fn edit(dispatcher: &Dispatcher, action: TaskAction) -> Result<SharedTask, ToolError> {
  match carry_out(dispatcher, action).await? {
    Accepted::Definition(Definition::Task(task)) => Ok(*task),
    _ => Err(not_a_task()),
  }
}

async fn carry_out(dispatcher: &Dispatcher, action: TaskAction) -> Result<Accepted, ToolError> {
  let outcome = dispatcher
    .dispatch(&Intent::new(Action::Task(action)))
    .await
    .map_err(action_refused)?;
  match outcome {
    ActionOutcome::Team(outcome) => accepted(outcome),
    // The dispatcher found the project local after all, e.g. unlinked meanwhile: nothing of a team task was saved.
    ActionOutcome::Local(_) => Err(not_a_task()),
  }
}

/// What the tools say about a team task saved on the server.
fn saved(task: &SharedTask) -> Value {
  json!({
    "key": task.key,
    "id": task.id.to_string(),
    "revision": task.revision.get(),
    "workflow_status": Workflow::from(task.content.workflow),
    "priority": Rank::from(task.content.priority),
    "blocked_reason": task.content.blocked_reason,
    "module": task.content.module.map(|module| module.to_string()),
    "team": true,
  })
}

/// The fields `unsupported` names as set, refused before anything is sent.
fn refuse_unsupported(unsupported: &[(&'static str, bool)]) -> Result<(), ToolError> {
  let named: Vec<&str> = unsupported
    .iter()
    .filter(|(_, set)| *set)
    .map(|(name, _)| *name)
    .collect();
  if named.is_empty() {
    return Ok(());
  }
  Err(ToolError::refused(
    "team_field_unsupported",
    vec![Message::new("appMcp.errors.teamFieldUnsupported").with("fields", named.join(", "))],
  ))
}

fn title(text: &str) -> Result<Name, ToolError> {
  Name::new(text.trim()).map_err(|error| ToolError::refused("invalid_task", vec![title_error(&error)]))
}

/// The criteria given, the blank ones left out, as the form keeps them.
fn criteria(given: &[String]) -> Vec<String> {
  given
    .iter()
    .map(|criterion| criterion.trim().to_owned())
    .filter(|criterion| !criterion.is_empty())
    .collect()
}

/// A team module named by its id: a team project's modules are the server's, never looked up by name here.
fn module_id(named: &str) -> Result<ModuleId, ToolError> {
  named.parse().map_err(|_| ToolError::not_found(Missing::Module, named))
}

fn organization_refused(error: OrganizationError) -> ToolError {
  StoreError::Organization(error).into()
}

/// The server answered a task action with something other than a task.
fn not_a_task() -> ToolError {
  ToolError::refused(
    "team_action_failed",
    vec![Message::new("appMcp.errors.teamActionFailed")],
  )
}
