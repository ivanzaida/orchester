//! `create_task` and `update_task` of a team project, through the dispatcher: a new task is the team server's
//! `task.create`, an edit its Admin-checked task route on the revision just read, so a member's edit is refused
//! `not_admin` and nothing is saved here. Only what the team server holds of a task is sent. Its pipeline is one of the
//! team's, sent by its resource id (`docs/architecture/server.md` row 80, decided 2026-11-05); a person's own local
//! pipeline, or anything else that is not a team pipeline here, is refused `team_pipeline_required`, and moving a task
//! from one pipeline to another is a person's decision, as on a local project. Limits wait for a team-authority
//! decision (D6) and are refused before anything is sent. A workflow status or blocked reason change is the task
//! lifecycle (D4): the same patch, which the server allows the claim's holder (Done, Blocked) or an Admin, and a create
//! carries no status. Story points, the tasks that block it, its tags and *Don't merge automatically* are the server's
//! as its other details (OA-35): a blocking task is named by its key among the team's tasks, a tag by its name among
//! the team's tags (made there when the team has none of it), and a member's change of them is `not_admin`. What the
//! server does not hold of a team task (delivers, the base branch) is refused rather than kept on this computer.
//!
//! A new task is made in its module at once, which the holder of the module's claim may do as an Admin may (OA-32). The
//! server takes a task's details, its organization, its story points and its module as separate changes, so one
//! call may send up to four, each on the revision the one before it left. Once one is saved, a later refusal does not
//! undo it: the tool answers the task as saved, with the changes `not_saved` and why, so a client never sends the saved
//! part again (a second task, for a create).

use orchester_api::{
  SharedTask,
  v2::{DecisionId, LifecycleAction, TeamCommandValue},
};
use orchester_core::{
  id::{ModuleId, ResourceId, TagId},
  name::Name,
  task::{
    TaskDetails, TaskLimits,
    estimate::StoryPoints,
    organization::{BlockingTask, OrganizationError, TaskOrganization, WorkflowStatus},
  },
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::{Value, json};

use super::{
  Team,
  modules::module_named,
  on_worker, organized,
  outcome::{accepted, action_refused, team_pipeline_required},
  tasks::find,
};
use crate::{
  actions::{Accepted, Action, ActionError, ActionOutcome, Definition, Dispatcher, Intent, TaskAction},
  app_mcp::{
    Services,
    error::ToolError,
    services::blocking,
    tools::{
      setup::widening,
      task_write::{CreateTask, UpdateTask, pipeline_named, story_points},
      wire::{Rank, Workflow},
    },
  },
  messages::Message,
  tasks::{form::title_error, team_pipelines::team_published_pipelines},
  team_reads::listed::TeamPipelines,
};

/// One change of a team task, sent on its own route.
enum Change {
  Details(TaskDetails, TaskDetails),
  Organization(TaskOrganization, TaskOrganization),
  Points(StoryPoints),
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
      Self::Points(points) => TaskAction::Points {
        task,
        read,
        points: Some(points),
      },
      Self::Module(module) => TaskAction::Move { task, read, module },
    }
  }

  /// What the result names this change, when it was not saved.
  const fn name(&self) -> &'static str {
    match self {
      Self::Details(..) => "details",
      Self::Organization(..) => "organization",
      Self::Points(_) => "story_points",
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
    ("delivers", args.delivers.is_some()),
    ("base_branch", args.base_branch.is_some()),
  ];
  refuse_unsupported(&unsupported)?;
  let limited = matches!(args.budget_tokens, Some(Some(_))) || matches!(args.time_limit_minutes, Some(Some(_)));
  if limited {
    return Err(action_refused(ActionError::DecisionRequired(DecisionId::D6)));
  }
  let status = args.workflow_status.map_or_else(WorkflowStatus::default, Into::into);
  if status != WorkflowStatus::default() {
    return Err(action_refused(ActionError::StatusOnCreate(status)));
  }
  let points = story_points(args.story_points)?;
  let (pipeline, _) = pipelines((services, team), args.pipeline.clone(), None).await?;
  let details = TaskDetails {
    title: title(&args.title)?,
    brief: args.brief.clone().unwrap_or_default(),
    acceptance_criteria: criteria(&args.criteria),
    pipeline,
    limits: TaskLimits::NONE,
  };
  let project = team.reads(services).await?;
  let blocking = organized::blockers(&project, &args.blocked_by).await?;
  let module = match args.module.as_deref() {
    Some(named) => Some(module_named(&project, named).await?),
    None => None,
  };
  // Checked with no tags first, so a refused organization makes no tag on the server.
  let organization = new_organization(args, status, (blocking, Vec::new()))?;
  let organization = organization.with_tags(organized::tags(services, team, &args.tags).await?);
  let dispatcher = team.dispatcher(services).await?;
  let (task, not_saved) = on_worker(move || async move {
    let create = TaskAction::Create {
      details,
      organization,
      module,
    };
    let created = match carry_out(&dispatcher, create).await? {
      Accepted::Command(receipt) => match receipt.result {
        TeamCommandValue::Task { task, .. } => *task,
        _ => return Err(not_a_task()),
      },
      Accepted::Definition(_) => return Err(not_a_task()),
    };
    apply(
      &dispatcher,
      (created, true),
      points.map(Change::Points).into_iter().collect(),
    )
    .await
  })
  .await?;
  Ok(answer(&task, not_saved))
}

/// A new team task's organization as `args` give it, with its `blocking` tasks and `tags` resolved on the server.
fn new_organization(
  args: &CreateTask,
  status: WorkflowStatus,
  (blocking, tags): (Vec<BlockingTask>, Vec<TagId>),
) -> Result<TaskOrganization, ToolError> {
  TaskOrganization::new(
    status,
    args.priority.map(Into::into).unwrap_or_default(),
    args.blocked_reason.clone(),
    blocking,
    tags,
  )
  .map(|organization| organization.with_no_automatic_merge(args.no_automatic_merge))
  .map_err(organization_refused)
}

pub(in crate::app_mcp::tools) async fn update_task(
  services: &Services,
  team: &Team,
  args: &UpdateTask,
) -> Result<Value, ToolError> {
  let unsupported = [
    ("delivers", args.delivers.is_some()),
    ("base_branch", args.base_branch.is_some()),
  ];
  refuse_unsupported(&unsupported)?;
  if args.budget_tokens.is_some() || args.time_limit_minutes.is_some() {
    return Err(action_refused(ActionError::DecisionRequired(DecisionId::D6)));
  }
  let points = story_points(args.story_points)?;
  let project = team.reads(services).await?;
  let module = match &args.module {
    Some(Some(named)) => Some(Some(module_named(&project, named).await?)),
    Some(None) => Some(None),
    None => None,
  };
  let task = find(&project, &args.key).await?.task;
  let blocking = match &args.blocked_by {
    Some(named) => Some(organized::blockers(&project, named).await?),
    None => None,
  };
  let (chosen, current) = pipelines((services, team), args.pipeline.clone(), task.content.pipeline.clone()).await?;
  // Another pipeline runs the task with other steps, profiles and approvals: a person decides that, as on a local
  // project. A task without one may get any team pipeline.
  if let Some(chosen) = chosen
    && task.content.pipeline.is_some()
    && current != Some(chosen)
  {
    widening::allowed(services, vec![Message::new("appMcp.widens.taskPipeline")])?;
  }
  // Everything refused in advance is refused here, before the first change is sent: the organization is checked with
  // the task's tags before any tag is made on the server.
  let mut changes = Vec::new();
  if let Some((base, edited)) = edited_details(&task, args, (current, chosen.or(current)))? {
    changes.push(Change::Details(base, edited));
  }
  organized(&task, args, (blocking.clone(), None))?;
  let tags = match &args.tags {
    Some(names) => Some(organized::tags(services, team, names).await?),
    None => None,
  };
  if let Some((base, edited)) = organized(&task, args, (blocking, tags))? {
    changes.push(Change::Organization(base, edited));
  }
  if let Some(points) = points.filter(|points| task.content.story_points != Some(*points)) {
    changes.push(Change::Points(points));
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

/// The details read of `task`, the team pipeline `current` it names here, and as `args` change them, with the team
/// pipeline `edited`; `None` when they change none.
fn edited_details(
  task: &SharedTask,
  args: &UpdateTask,
  (current, edited): (Option<ResourceId>, Option<ResourceId>),
) -> Result<Option<(TaskDetails, TaskDetails)>, ToolError> {
  if args.title.is_none() && args.brief.is_none() && args.criteria.is_none() && args.pipeline.is_none() {
    return Ok(None);
  }
  let content = &task.content;
  // Limits are left out of both: they wait for D6, and an edit sends only what changed.
  let base = TaskDetails {
    title: title(&content.title)?,
    brief: content.brief.clone(),
    acceptance_criteria: content.criteria.clone(),
    pipeline: current,
    limits: TaskLimits::NONE,
  };
  let edited = TaskDetails {
    title: args.title.as_deref().map_or_else(|| Ok(base.title.clone()), title)?,
    brief: args.brief.clone().unwrap_or_else(|| base.brief.clone()),
    acceptance_criteria: args
      .criteria
      .as_deref()
      .map_or_else(|| base.acceptance_criteria.clone(), criteria),
    pipeline: edited,
    limits: TaskLimits::NONE,
  };
  Ok((edited != base).then_some((base, edited)))
}

/// The team pipeline `named` among the team's pipelines published here, by its id or name, and the team pipeline a
/// task's `current` pipeline is here, read together. A `named` pipeline that is not the team's, a person's own local
/// one included, is refused: only the team's pipelines run a team task, and its Admins make them.
async fn pipelines(
  (services, team): (&Services, &Team),
  named: Option<String>,
  current: Option<String>,
) -> Result<(Option<ResourceId>, Option<ResourceId>), ToolError> {
  if named.is_none() && current.is_none() {
    return Ok((None, None));
  }
  let root = team.open.repo_root.clone();
  // The team's pipelines as this session read them from the team server; refused while it cannot be read.
  let config = services.workspace().team_read(&root).loaded()?;
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    let pipelines = TeamPipelines::of(&config);
    let chosen = match named {
      Some(named) => {
        // A team project offers only the team's pipelines: a person's own local one is none of these.
        let published = team_published_pipelines(&db, &config)?;
        let chosen = pipeline_named(&published, &named).map_err(|_| team_pipeline_required(&named))?;
        Some(chosen.id)
      }
      None => None,
    };
    Ok((chosen, current.and_then(|current| pipelines.id(&current))))
  })
  .await
}

/// The organization read of `task` and as `args` change it, with the `blocking` tasks and the `tags` they name as
/// the server resolved them (the task's own where they name none); `None` when they change nothing of it. A workflow
/// status change the lifecycle does not have (D4: In progress, In review, Done to Canceled) is refused before any
/// change of the call is sent; who may make the others the team server decides, by the caller's claim and role.
fn organized(
  task: &SharedTask,
  args: &UpdateTask,
  (blocking, tags): (Option<Vec<BlockingTask>>, Option<Vec<TagId>>),
) -> Result<Option<(TaskOrganization, TaskOrganization)>, ToolError> {
  let unchanged = args.workflow_status.is_none()
    && args.priority.is_none()
    && args.blocked_reason.is_none()
    && args.blocked_by.is_none()
    && args.tags.is_none()
    && args.no_automatic_merge.is_none();
  if unchanged {
    return Ok(None);
  }
  let base = task.content.organization().core().map_err(organization_refused)?;
  let status = args.workflow_status.map_or(base.status(), Into::into);
  if status != base.status() && LifecycleAction::of_workflow(base.status(), status).is_none() {
    return Err(action_refused(ActionError::WorkflowNotOffered {
      from: base.status(),
      to: status,
    }));
  }
  let edited = TaskOrganization::new(
    status,
    args.priority.map_or(base.priority(), Into::into),
    match &args.blocked_reason {
      Some(reason) => reason.clone(),
      None => base.blocked_reason().map(str::to_owned),
    },
    blocking.unwrap_or_else(|| base.dependencies().to_vec()),
    tags.unwrap_or_else(|| base.tags().to_vec()),
  )
  .map_err(organization_refused)?
  .with_no_automatic_merge(args.no_automatic_merge.unwrap_or(base.no_automatic_merge()));
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
    "blocked_by": task.content.blocking,
    "tags": task.content.tags,
    "story_points": task.content.story_points,
    "no_automatic_merge": task.content.no_automatic_merge,
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
