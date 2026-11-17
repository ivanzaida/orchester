//! `deliver_module` and `start_acceptance`: a module with a branch of its own on its way to its base branch
//! (`docs/plan/orchestrator-autonomy/OA-17-…`, `OA-20-…`, `OA-32-…`). Delivery is a call of its own in both kinds of
//! project, apart from a run's merge: a local module's own branch is merged into its base branch on this computer
//! ([`crate::modules::delivery`]); a team module's is requested from its team server, which an Admin's authorization
//! or the team's automatic merge decides ([`super::team`]). An acceptance is one task Orchester makes in the module
//! and starts, whose run reviews the module's whole diff; its brief is Orchester's. A team module's acceptance task is
//! made on its team server, in the module, on the team pipeline this computer chose for it.

use std::path::Path;

use orchester_core::{
  id::{ModuleId, OperationId, ResourceId, TaskId},
  integration::base::BaseChoice,
  module::{
    Module,
    delivery::{Acceptance, DeliveryStep, ModuleDelivered},
  },
  orchestrator::{
    AcceptedUnverified,
    instructions::{ACCEPTANCE_CRITERIA, AcceptedTask, acceptance_brief, acceptance_title},
  },
  store::{ActivityStore, RunStore, TaskStore},
  task::{Task, organization::WorkflowStatus},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  modules::find_module,
  run_task::{self, RunTask},
  task_write, team, wire,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::Link,
  messages::Message,
  modules::delivery::{deliver_in, facts},
  tasks::{detail::run_store_error, outside::Outside},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct DeliverModule {
  /// The project's id, or its name.
  pub project: String,
  /// The module's id, or its name.
  pub module: String,
  /// A team project only: the operation id of the Admin's delivery authorization the delivery cites. Leave it out for
  /// the team's automatic merge, and for a local project.
  #[serde(default)]
  pub authorization: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct StartAcceptance {
  /// The project's id, or its name.
  pub project: String,
  /// The module's id, or its name.
  pub module: String,
}

pub async fn deliver_module(
  services: &Services,
  args: DeliverModule,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let authorization = args
    .authorization
    .as_deref()
    .map(|named| {
      named.parse::<OperationId>().map_err(|_| {
        ToolError::InvalidArguments(format!(
          "`authorization` is an operation id such as a UUID, not `{named}`"
        ))
      })
    })
    .transpose()?;
  if let Some(team) = team::of(services, &args.project).await? {
    return team::deliver_module(services, &team, (&args.module, authorization), translate).await;
  }
  if authorization.is_some() {
    return Err(ToolError::InvalidArguments(
      "`authorization` cites a team module delivery's authorization; a local module's delivery takes none".to_owned(),
    ));
  }
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let module = find_module(&ProjectDb::open(&open.repo_root)?, &args.module)?;
    let delivered = deliver_in(&open.repo_root, &services.programs, module.id())
      .map_err(|refusal| ToolError::refused(refusal.code(), vec![refusal.message()]))?;
    services.changes.record(open.project.id(), Outside::Tasks);
    Ok(json!({
      "delivered": delivered_json(&delivered),
      "module": module.id().to_string(),
      "link": Link::Module(open.project.id(), module.id()).path(),
    }))
  })
  .await
}

/// A delivery that merged, as the tools tell it.
pub(super) fn delivered_json(delivered: &ModuleDelivered) -> Value {
  json!({
    "into": delivered.into.as_str(),
    "head": delivered.head.as_str(),
    "revision": delivered.merge.as_str(),
    "at": wire::time(delivered.at),
  })
}

/// What `get_module` tells of a module's own branch: `null` for a module without one.
pub(super) fn own_branch_json(db: &ProjectDb, module: &Module) -> Result<Value, ToolError> {
  let Some(own) = module.own_branch() else {
    return Ok(Value::Null);
  };
  let read = facts(db, module)?;
  let delivered = db.module_delivery(module.id())?;
  // A branch with nothing merged into it reads as delivered (nothing to deliver); it waits for work first.
  let waits_for = match read.step() {
    DeliveryStep::Delivered if delivered.is_none() => "work",
    step => step.name(),
  };
  Ok(json!({
    "branch": own.branch.as_str(),
    "acceptance_pipeline": own.acceptance.map(|pipeline| pipeline.to_string()),
    "deliver_when_accepted": own.deliver_when_accepted,
    "acceptances": read.acceptances,
    "acceptance": acceptance_name(read.acceptance),
    "delivered": delivered.as_ref().map(delivered_json),
    // What it waits for once its every task is done.
    "waits_for": waits_for,
  }))
}

pub(super) const fn acceptance_name(acceptance: Acceptance) -> &'static str {
  match acceptance {
    Acceptance::None => "none",
    Acceptance::UnderWay => "under_way",
    Acceptance::Passed => "passed",
    Acceptance::NotPassed => "not_passed",
  }
}

pub async fn start_acceptance(services: &Services, args: StartAcceptance) -> Result<Value, ToolError> {
  if let Some(team) = team::of(services, &args.project).await? {
    return team::start_acceptance(services, &team, &args).await;
  }
  let reading = services.clone();
  let (project, named) = (args.project.clone(), args.module);
  let (module, create) = blocking(move || {
    let open = reading.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let module = find_module(&db, &named)?;
    let create = prepared(&db, (&open.repo_root, &reading), &module, &args.project)?;
    Ok((module.id(), create))
  })
  .await?;
  made_and_started(services, (project, module), create).await
}

/// Makes the acceptance task `create` describes, keeps it as the acceptance of `module` of `project` and starts it: one
/// way for a local module and a team module, whose task is made on its team server.
pub(super) async fn made_and_started(
  services: &Services,
  (project, module): (String, ModuleId),
  create: task_write::CreateTask,
) -> Result<Value, ToolError> {
  let created = task_write::create_task(services, create).await?;
  let named = |pointer: &str| {
    created
      .pointer(pointer)
      .and_then(Value::as_str)
      .map(str::to_owned)
      .ok_or_else(ToolError::interrupted)
  };
  let key = named("/task/key")?;
  let task: TaskId = named("/task/id")?.parse().map_err(|_| ToolError::interrupted())?;
  let (recording, at, named_project) = (services.clone(), Timestamp::now(), project.clone());
  blocking(move || {
    let open = recording.project(&named_project)?;
    ProjectDb::open(&open.repo_root)?.keep_module_acceptance(module, task, at)?;
    Ok(())
  })
  .await?;
  // A team task the server saved without its module is no task of the module yet: its run would not start from the
  // module's branch, so it is not started, and what was not saved is told.
  if let Some(not_saved) = created.get("not_saved") {
    return Ok(json!({ "acceptance": { "task": key, "started": null, "not_saved": not_saved } }));
  }
  // The task is the module's acceptance from here on, whether or not its run starts now: a start that is refused is
  // told with it, and the task starts as any task does.
  let started = run_task::run_task(
    services,
    RunTask {
      project,
      key: key.clone(),
    },
  )
  .await;
  Ok(match started {
    Ok(run) => json!({ "acceptance": { "task": key, "started": run } }),
    Err(error) => json!({ "acceptance": { "task": key, "started": null, "not_started": error.code() } }),
  })
}

/// A refusal of a module's acceptance or delivery, worded by `key`.
pub(super) fn refused(code: &'static str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}

/// The acceptance task of `module` as `create_task` takes it; refused when the module is not ready for one: it has no
/// branch of its own or names no acceptance pipeline, a task of it is still open, an acceptance is under way, or the
/// branch as it is now passed one already.
fn prepared(
  db: &ProjectDb,
  (root, services): (&Path, &Services),
  module: &Module,
  project: &str,
) -> Result<task_write::CreateTask, ToolError> {
  let own = module
    .own_branch()
    .ok_or_else(|| refused("no_own_branch", "modules.delivery.noOwnBranch"))?;
  let pipeline = own
    .acceptance
    .ok_or_else(|| refused("no_acceptance_pipeline", "modules.acceptance.noPipeline"))?;
  let acceptances: Vec<TaskId> = db
    .module_acceptances(module.id())?
    .into_iter()
    .map(|(task, _)| task)
    .collect();
  let tasks: Vec<Task> = db
    .tasks()?
    .into_iter()
    .filter(|task| task.module() == Some(module.id()) && !acceptances.contains(&task.id()))
    .collect();
  let open: Vec<String> = tasks
    .iter()
    .filter(|task| !task.organization().status().is_closed())
    .map(|task| task.key().to_string())
    .collect();
  refuse_open(&open)?;
  refuse_accepting(facts(db, module)?.acceptance)?;
  let into = match module.base() {
    BaseChoice::Branch(branch) => branch.as_str().to_owned(),
    BaseChoice::CheckedOut => checked_out_into(services, root),
  };
  let listed = tasks
    .iter()
    .filter(|task| task.organization().status() != WorkflowStatus::Canceled)
    .map(|task| {
      Ok(AcceptedTask {
        key: task.key().to_string(),
        title: task.details().title.as_str().to_owned(),
        criteria: task.details().acceptance_criteria.clone(),
        accepted_unverified: accepted_unverified(db, task.id())?,
      })
    })
    .collect::<Result<Vec<_>, ToolError>>()?;
  acceptance_create(
    project,
    (module.id(), module.name().as_str()),
    (own.branch.as_str(), &into),
    (pipeline, &listed),
  )
}

/// Refused while the tasks `open`, by their keys, are neither done nor canceled.
pub(super) fn refuse_open(open: &[String]) -> Result<(), ToolError> {
  if open.is_empty() {
    return Ok(());
  }
  Err(ToolError::refused(
    "tasks_open",
    vec![Message::new("modules.acceptance.tasksOpen").with("tasks", open.join(", "))],
  ))
}

/// Refused while the module's latest acceptance, which stands as `acceptance`, is under way or passed for its branch
/// as it is now.
pub(super) fn refuse_accepting(acceptance: Acceptance) -> Result<(), ToolError> {
  match acceptance {
    Acceptance::UnderWay => Err(refused("acceptance_under_way", "modules.acceptance.underWay")),
    Acceptance::Passed => Err(refused("already_accepted", "modules.acceptance.alreadyAccepted")),
    Acceptance::None | Acceptance::NotPassed => Ok(()),
  }
}

/// The branch the checkout at `root` has checked out, which a module that names no base branch is delivered into; as
/// the brief words it when none can be read.
pub(super) fn checked_out_into(services: &Services, root: &Path) -> String {
  let runner = services.programs.runner();
  orchester_core::integration::checked_out_branch(&runner, Path::new(crate::tasks::changes::GIT), root)
    .ok()
    .flatten()
    .map_or_else(|| "the base branch".to_owned(), |branch| branch.as_str().to_owned())
}

/// The acceptance task of module `(id, name)` of `project` as `create_task` takes it: its branch going `into` its base
/// branch, on `pipeline`, with Orchester's brief listing `tasks`.
pub(super) fn acceptance_create(
  project: &str,
  (module, name): (ModuleId, &str),
  (branch, into): (&str, &str),
  (pipeline, tasks): (ResourceId, &[AcceptedTask]),
) -> Result<task_write::CreateTask, ToolError> {
  serde_json::from_value(json!({
    "project": project,
    "title": acceptance_title(name),
    "brief": acceptance_brief(name, (branch, into), tasks),
    "criteria": ACCEPTANCE_CRITERIA,
    "pipeline": pipeline.to_string(),
    "module": module.to_string(),
  }))
  .map_err(|error| ToolError::InvalidArguments(error.to_string()))
}

/// The steps of the latest run of task `task` here that automation accepted unverified; none while it never ran here.
pub(super) fn accepted_unverified(db: &ProjectDb, task: TaskId) -> Result<Vec<String>, ToolError> {
  let runs = db.runs_of_task(task).map_err(run_store_error)?;
  let Some(run) = runs.iter().rev().find_map(|listed| listed.as_ref().ok()) else {
    return Ok(Vec::new());
  };
  let events = db.events(run.id()).map_err(run_store_error)?;
  let outputs = db.step_outputs(run.id())?;
  Ok(
    AcceptedUnverified::of_run(run, &events, &outputs)
      .into_iter()
      .map(|accepted| accepted.step)
      .collect(),
  )
}
