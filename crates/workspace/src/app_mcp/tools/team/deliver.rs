//! `merge_run` of a team task's run (team authority P7b): a task's run never merges into the base branch from here
//! (D3). Its work went into its module's branch, so the tool requests that module's delivery through
//! [`ModuleDeliveries`]: the run's module, read from the team server for its branch; the module's claim (`claim.acquire`,
//! which answers the claim this session holds already and takes it when nobody holds it); the module branch's tip as
//! the source commit, and the tip of the branch the checkout has checked out, the base the module branch was made from,
//! as the base commit. A delivery of the module not settled yet is carried on rather than requested again. An Admin's
//! authorization or the workspace's automatic merge (D7) decides whether it is delivered: the request cites the Admin's
//! `delivery.authorize` operation its caller names, if any, and the tool never authorizes. Only a delivered module is
//! the tool's result; every other state is a refusal that names it.

use std::path::Path;

use orchester_api::v2::{AuthorityError, AuthorityErrorCode, DeliveryState, Subject};
use orchester_core::{
  id::{ModuleId, OperationId},
  integration::{base::RunBaseSource, checked_out_branch},
  run::{BranchName, CommitId},
};
use orchester_exec::programs::Programs;
use orchester_store::{DeliveryMerge, ProjectDb, TeamDeliveryStage};
use serde_json::{Value, json};

use super::{
  Team, on_worker,
  outcome::{action_refused, authority_refused, code_name},
  read, read_refused,
  run::{acquire, branch_tip},
};
use crate::{
  actions::{DeliveryAction, DeliveryStep},
  app_mcp::{
    Services, Translate,
    error::{Missing, ToolError},
    services::blocking,
    tools::find_run,
  },
  messages::Message,
  tasks::changes::GIT,
  team_reads::TeamReadError,
  team_work::{
    DeliveryProgress, DeliveryStatus, ModuleDeliveries, TeamDeliveryRequest, TeamDeliveryState, TeamWorkError,
  },
};

/// The branches a delivery takes the module's work between, each at its tip in the checkout.
struct Tips {
  branch: BranchName,
  source_commit: CommitId,
  base_branch: BranchName,
  base_commit: CommitId,
}

/// Where the module's delivery stands: what carrying it on answered, `None` when another caller settled it meanwhile;
/// and its record.
type Delivered = (Option<DeliveryProgress>, Option<TeamDeliveryState>);

/// Requests the delivery of the module run `run` of `team` worked in, citing the Admin's `authorization` when one is
/// named, or carries on the module's open one.
pub(in crate::app_mcp::tools) async fn merge_run(
  services: &Services,
  team: &Team,
  (run, authorization): (&str, Option<OperationId>),
  translate: &Translate,
) -> Result<Value, ToolError> {
  let (root, named) = (team.open.repo_root.clone(), run.to_owned());
  let module = blocking(move || run_module(&ProjectDb::open(&root)?, &named)).await?;
  let project = team.reads(services).await?;
  let branch = match read(&project, project.module(module)).await {
    Ok(read) => read.into_value().module.branch,
    Err(TeamReadError::NotFound) => return Err(ToolError::not_found(Missing::Module, module.to_string())),
    Err(error) => return Err(read_refused(&error)),
  };
  let (root, programs) = (team.open.repo_root.clone(), services.programs().clone());
  let tips = {
    let programs = programs.clone();
    blocking(move || tips(&programs, &root, &branch)).await?
  };
  let dispatcher = team.dispatcher(services).await?;
  let (progress, state) = on_worker(move || async move {
    let deliveries = ModuleDeliveries::new(dispatcher, programs);
    deliver(&deliveries, (module, tips), authorization).await
  })
  .await?;
  match progress {
    Some(progress) => answer(progress, state.as_ref(), translate),
    None => recorded(state.as_ref(), translate),
  }
}

/// The module run `named` worked in: a run that did not start on a module's branch has no delivery.
fn run_module(db: &ProjectDb, named: &str) -> Result<ModuleId, ToolError> {
  match find_run(db, named)?.base_source() {
    RunBaseSource::Module { module } => Ok(module),
    RunBaseSource::Task | RunBaseSource::CheckedOut | RunBaseSource::Project => Err(ToolError::refused(
      "module_required",
      vec![Message::new("appMcp.errors.teamRunNotInModule")],
    )),
  }
}

/// The module's `branch` and the branch checked out at `root`, each with its tip; refused when the checkout has no
/// branch checked out, or the module's own.
fn tips(programs: &Programs, root: &Path, branch: &str) -> Result<Tips, ToolError> {
  let runner = programs.runner();
  let no_base = || ToolError::refused("no_base_branch", vec![Message::new("appMcp.errors.teamDeliveryNoBase")]);
  let source_commit = branch_tip(runner, root, branch)?;
  let base_branch = checked_out_branch(&runner, Path::new(GIT), root)
    .map_err(|error| {
      tracing::warn!(%error, "cannot read the branch a team delivery lands on");
      no_base()
    })?
    .ok_or_else(no_base)?;
  if base_branch.as_str() == branch {
    return Err(ToolError::refused(
      "module_branch_checked_out",
      vec![Message::new("appMcp.errors.teamDeliveryOnModuleBranch").with("branch", branch.to_owned())],
    ));
  }
  let base_commit = branch_tip(runner, root, base_branch.as_str())?;
  Ok(Tips {
    // `branch_tip` read the branch by this name, so it is one.
    branch: BranchName::new(branch).expect("a branch whose tip was read"),
    source_commit,
    base_branch,
    base_commit,
  })
}

/// Carries on the module's open delivery, with the project's other open ones in their order; else, unless runs of the
/// module still work here, takes the module's claim and requests its delivery between `tips`.
async fn deliver(
  deliveries: &ModuleDeliveries,
  (module, tips): (ModuleId, Tips),
  authorization: Option<OperationId>,
) -> Result<Delivered, ToolError> {
  if let Some(open) = deliveries.open_delivery(module).map_err(work_refused)? {
    // Another caller may have settled it between the read and the recovery: then its record says how.
    let carried = deliveries
      .recover()
      .await
      .map_err(work_refused)?
      .into_iter()
      .find(|(operation, _)| *operation == open)
      .map(|(_, progress)| progress)
      .transpose()
      .map_err(work_refused)?;
    return Ok((carried, deliveries.delivery_state(open).map_err(work_refused)?));
  }
  let running = deliveries.runs_at_work(module).map_err(work_refused)?;
  if !running.is_empty() {
    return Ok((Some(DeliveryProgress::WorkRunning(running)), None));
  }
  let claim = acquire(deliveries.dispatcher(), Subject::Module { id: module }, delivery_failed).await?;
  let request = TeamDeliveryRequest {
    operation: OperationId::generate(),
    delivery: DeliveryAction {
      module,
      claim,
      branch: tips.branch,
      source_commit: tips.source_commit,
      base_commit: tips.base_commit,
      step: DeliveryStep::Request { authorization },
    },
    base_branch: tips.base_branch,
  };
  let operation = request.operation;
  let progress = deliveries.request(request).await.map_err(work_refused)?;
  Ok((
    Some(progress),
    deliveries.delivery_state(operation).map_err(work_refused)?,
  ))
}

/// The tool's answer for where the delivery stands.
fn answer(
  progress: DeliveryProgress,
  state: Option<&TeamDeliveryState>,
  translate: &Translate,
) -> Result<Value, ToolError> {
  match progress {
    DeliveryProgress::WorkRunning(runs) => Err(ToolError::refused(
      "team_work_running",
      vec![Message::new("appMcp.errors.teamDeliveryWorkRunning").with("runs", runs.len().to_string())],
    )),
    DeliveryProgress::Refused(error) if error.code() == AuthorityErrorCode::DeliveryNotAuthorized => {
      Err(not_authorized())
    }
    DeliveryProgress::Refused(error) => Err(authority_refused(&error)),
    DeliveryProgress::Waiting(TeamDeliveryStage::Reporting) => Err(refused(
      "team_delivery_waiting",
      "appMcp.errors.teamDeliveryReportWaiting",
    )),
    DeliveryProgress::Waiting(_) => Err(waiting()),
    DeliveryProgress::MergeWaiting => Err(refused(
      "team_delivery_merge_waiting",
      "appMcp.errors.teamDeliveryMergeWaiting",
    )),
    DeliveryProgress::Reported(delivery) => match delivery.state {
      DeliveryState::Delivered => delivered(state, translate),
      DeliveryState::Failed => Err(failed()),
      DeliveryState::Authorized | DeliveryState::Requested => Err(waiting()),
    },
    // A report refused as not authorized is of a delivery the automatic merge's withdrawal failed.
    DeliveryProgress::ReportRefused(error) if error.code() == AuthorityErrorCode::DeliveryNotAuthorized => {
      Err(refused("delivery_failed", "appMcp.errors.teamDeliveryWithdrawn"))
    }
    DeliveryProgress::ReportRefused(error) => Err(report_refused(&error)),
  }
}

/// The tool's answer for a delivery another caller settled while this one carried it on, by its record.
fn recorded(state: Option<&TeamDeliveryState>, translate: &Translate) -> Result<Value, ToolError> {
  let Some(recorded) = state else {
    return Err(delivery_failed());
  };
  match &recorded.status {
    DeliveryStatus::Delivered => delivered(state, translate),
    DeliveryStatus::Failed => Err(failed()),
    DeliveryStatus::AwaitingAuthorization => Err(not_authorized()),
    // Only a report is refused once the merge was made here.
    DeliveryStatus::Refused(error) if recorded.merge.is_some() => Err(report_refused(error)),
    DeliveryStatus::Refused(error) => Err(authority_refused(error)),
    DeliveryStatus::Requested | DeliveryStatus::Authorized { .. } | DeliveryStatus::AuthorizedAutomatically => {
      Err(waiting())
    }
  }
}

/// The delivered module as the tool answers it: its branch, both commits and the merge commit on the base branch.
fn delivered(state: Option<&TeamDeliveryState>, translate: &Translate) -> Result<Value, ToolError> {
  let Some(state) = state else {
    return Err(delivery_failed());
  };
  let merge_commit = match &state.merge {
    Some(DeliveryMerge::Merged { commit }) => commit.as_str(),
    Some(DeliveryMerge::Failed { .. }) | None => return Err(delivery_failed()),
  };
  Ok(json!({
    "delivery": {
      "module": state.module.to_string(),
      "branch": state.branch.as_str(),
      "into": state.base_branch.as_str(),
      "source_commit": state.source_commit.as_str(),
      "base_commit": state.base_commit.as_str(),
      "merge_commit": merge_commit,
      "authorization": state.server.as_ref().and_then(|server| server.authorization_operation_id).map(|by| by.to_string()),
      "state": "delivered",
    },
    "message": translate.text(&Message::new("appMcp.teamDelivered").with("into", state.base_branch.to_string())),
    "team": true,
  }))
}

fn work_refused(error: TeamWorkError) -> ToolError {
  match error {
    TeamWorkError::Action(error) => action_refused(error),
    TeamWorkError::Store(error) => error.into(),
    error => {
      tracing::warn!(%error, "a team delivery could not go on");
      delivery_failed()
    }
  }
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}

fn not_authorized() -> ToolError {
  refused("delivery_not_authorized", "appMcp.errors.teamDeliveryNotAuthorized")
}

fn waiting() -> ToolError {
  refused("team_delivery_waiting", "appMcp.errors.teamDeliveryWaiting")
}

fn failed() -> ToolError {
  refused("delivery_failed", "appMcp.errors.teamDeliveryFailed")
}

fn report_refused(error: &AuthorityError) -> ToolError {
  ToolError::refused(
    "delivery_report_refused",
    vec![Message::new("appMcp.errors.teamDeliveryReportRefused").with("code", code_name(error.code()))],
  )
}

fn delivery_failed() -> ToolError {
  refused("team_delivery_failed", "appMcp.errors.teamDeliveryCouldNotGoOn")
}
