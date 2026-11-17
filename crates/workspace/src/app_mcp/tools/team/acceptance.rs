//! A team module's acceptance before its delivery (`docs/plan/orchestrator-autonomy/OA-20-…`, `OA-32-…`), as a local
//! module's: `start_acceptance` makes one task in the module on its team server, through `task.create` as
//! `create_task` makes one, on the team pipeline this computer chose for the module's acceptance, keeps it as the
//! module's acceptance and starts it through the team's work start. The module's tasks and their statuses are the
//! server's, read now; the steps automation accepted unverified are this computer's runs'. The acceptance pipeline and
//! *Deliver when accepted* are kept on this computer (`AcceptanceChoice`): the server keeps neither, and changing
//! them sends nothing to it, so a member sets them as an Admin does. Where the module stands is the rule of a local
//! module ([`crate::modules::team_delivery`]).

use orchester_api::v2::{ModuleSummary, TaskFilters, TaskRead};
use orchester_core::{
  id::{ModuleId, ResourceId, TaskId},
  module::delivery::{AcceptanceChoice, DeliveryStep},
  orchestrator::instructions::AcceptedTask,
  task::organization::WorkflowStatus,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Team, modules, outcome::team_pipeline_required, read, read_refused, tasks};
use crate::{
  app_mcp::{
    Services,
    error::ToolError,
    services::blocking,
    tools::{
      module_delivery::{
        StartAcceptance, acceptance_create, acceptance_name, accepted_unverified, checked_out_into, made_and_started,
        refuse_accepting, refuse_open, refused,
      },
      task_write::pipeline_named,
    },
  },
  modules::team_delivery::{self, TeamDelivery},
  tasks::team_pipelines::team_published_pipelines,
  team_reads::{TeamProject, TeamReadError},
};

pub(in crate::app_mcp::tools) async fn start_acceptance(
  services: &Services,
  team: &Team,
  args: &StartAcceptance,
) -> Result<Value, ToolError> {
  let project = team.reads(services).await?;
  let summary = modules::find(&project, &args.module).await?;
  let module = summary.module.module.id;
  let tasks = of_module(&project, module).await?;
  let (root, reading, named) = (team.open.repo_root.clone(), services.clone(), args.project.clone());
  let create = blocking(move || {
    let db = ProjectDb::open(&root)?;
    let pipeline = db
      .team_module_acceptance(module)?
      .acceptance
      .ok_or_else(|| refused("no_acceptance_pipeline", "modules.acceptance.noPipeline"))?;
    let acceptances: Vec<TaskId> = db
      .module_acceptances(module)?
      .into_iter()
      .map(|(task, _)| task)
      .collect();
    let work: Vec<&TaskRead> = tasks
      .iter()
      .filter(|read| !acceptances.contains(&read.task.id))
      .collect();
    let open: Vec<String> = work
      .iter()
      .filter(|read| !read.task.content.workflow.is_closed())
      .map(|read| read.task.key.clone())
      .collect();
    refuse_open(&open)?;
    let status = |task: TaskId| status_among(&tasks, task);
    refuse_accepting(team_delivery::read(&db, module, &status)?.facts.acceptance)?;
    let listed = work
      .iter()
      .filter(|read| read.task.content.workflow != WorkflowStatus::Canceled)
      .map(|read| {
        Ok(AcceptedTask {
          key: read.task.key.clone(),
          title: read.task.content.title.clone(),
          criteria: read.task.content.criteria.clone(),
          accepted_unverified: accepted_unverified(&db, read.task.id)?,
        })
      })
      .collect::<Result<Vec<_>, ToolError>>()?;
    let into = checked_out_into(&reading, &root);
    acceptance_create(
      &named,
      (module, &summary.module.module.fields.name),
      (&summary.module.branch, &into),
      (pipeline, &listed),
    )
  })
  .await?;
  made_and_started(services, (args.project.clone(), module), create).await
}

/// What `get_module` tells of a team module's branch on its way to its base branch, as a local module's own branch:
/// the branch the team assigned, this computer's acceptance pipeline and *Deliver when accepted*, its acceptances and
/// where the latest stands, read from the server now, and what it waits for once its every task is done.
pub(in crate::app_mcp::tools) async fn own_branch_json(
  services: &Services,
  team: &Team,
  summary: &ModuleSummary,
) -> Result<Value, ToolError> {
  let module = summary.module.module.id;
  let root = team.open.repo_root.clone();
  let latest = blocking(move || Ok(ProjectDb::open_read_only(&root)?.module_acceptances(module)?.pop())).await?;
  let status = match latest {
    Some((task, _)) => latest_status(services, team, task).await?,
    None => None,
  };
  let root = team.open.repo_root.clone();
  let (choice, read) = blocking(move || {
    let db = ProjectDb::open_read_only(&root)?;
    let choice = db.team_module_acceptance(module)?;
    let read = team_delivery::read(&db, module, &|task| {
      status.filter(|_| Some(task) == latest.map(|(id, _)| id))
    })?;
    Ok((choice, read))
  })
  .await?;
  let TeamDelivery { facts, step, merged } = read;
  // A branch with nothing merged into it here reads as delivered (nothing to deliver); it waits for work first.
  let waits_for = match step {
    DeliveryStep::Delivered if !merged => "work",
    step => step.name(),
  };
  Ok(json!({
    "branch": summary.module.branch,
    "acceptance_pipeline": choice.acceptance.map(|pipeline| pipeline.to_string()),
    "deliver_when_accepted": choice.deliver_when_accepted,
    "acceptances": facts.acceptances,
    "acceptance": acceptance_name(facts.acceptance),
    "waits_for": waits_for,
  }))
}

/// Keeps for the team module `module` the acceptance pipeline `pipeline` names among the team's published pipelines
/// (`Some(None)`: none) and *Deliver when accepted*; what is `None` stays as it is. Nothing is sent to the server.
pub(in crate::app_mcp::tools) async fn choose(
  services: &Services,
  team: &Team,
  module: ModuleId,
  (pipeline, deliver_when_accepted): (Option<Option<String>>, Option<bool>),
) -> Result<(), ToolError> {
  if pipeline.is_none() && deliver_when_accepted.is_none() {
    return Ok(());
  }
  let root = team.open.repo_root.clone();
  let config = match pipeline {
    Some(Some(_)) => Some(services.workspace().team_read(&root).loaded()?),
    Some(None) | None => None,
  };
  blocking(move || {
    let mut db = ProjectDb::open(&root)?;
    let was = db.team_module_acceptance(module)?;
    let acceptance = match (pipeline, &config) {
      (Some(Some(named)), Some(config)) => Some(team_pipeline(&db, config, &named)?),
      (Some(_), _) => None,
      (None, _) => was.acceptance,
    };
    let choice = AcceptanceChoice {
      acceptance,
      deliver_when_accepted: deliver_when_accepted.unwrap_or(was.deliver_when_accepted),
    };
    db.set_team_module_acceptance(module, choice.kept())?;
    Ok(())
  })
  .await
}

/// The team pipeline `named` by its id or name among the team's pipelines published here; another is refused.
fn team_pipeline(
  db: &ProjectDb,
  config: &crate::team_config::TeamConfig,
  named: &str,
) -> Result<ResourceId, ToolError> {
  let published = team_published_pipelines(db, config)?;
  Ok(
    pipeline_named(&published, named)
      .map_err(|_| team_pipeline_required(named))?
      .id,
  )
}

/// The workflow status the team server holds the task `task` in now; `None` when it has no such task any more.
async fn latest_status(services: &Services, team: &Team, task: TaskId) -> Result<Option<WorkflowStatus>, ToolError> {
  let project = team.reads(services).await?;
  match read(&project, project.task(task)).await {
    Ok(found) => Ok(Some(found.into_value().task.content.workflow)),
    Err(TeamReadError::NotFound) => Ok(None),
    Err(error) => Err(read_refused(&error)),
  }
}

/// Every task the team server holds in `module`, acceptances included.
async fn of_module(project: &TeamProject, module: ModuleId) -> Result<Vec<TaskRead>, ToolError> {
  let filters = TaskFilters {
    modules: vec![Some(module)],
    ..TaskFilters::default()
  };
  tasks::all_tasks(project, filters)
    .await
    .map_err(|error| read_refused(&error))
}

/// The workflow status of `task` among `tasks`; `None` when it is not among them (moved out of the module, or gone).
fn status_among(tasks: &[TaskRead], task: TaskId) -> Option<WorkflowStatus> {
  tasks
    .iter()
    .find(|read| read.task.id == task)
    .map(|read| read.task.content.workflow)
}
