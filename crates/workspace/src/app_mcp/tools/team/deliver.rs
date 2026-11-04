//! `merge_run` of a team task's run (team authority P7b): a task's run never merges into the base branch from here
//! (D3). Its work went into its module's branch, so the tool requests that module's delivery through
//! [`ModuleDeliveries`]: the run's module, read from the team server for its branch; the module's claim (`claim.acquire`,
//! which answers the claim this session holds already and takes it when nobody holds it); the module branch's tip as
//! the source commit, and the tip of the branch the checkout has checked out, the base the module branch was made from,
//! as the base commit. A delivery of the module not settled yet is carried on rather than requested again. An Admin's
//! authorization or the workspace's automatic merge (D7) decides whether it is delivered: the tool cites no
//! authorization and never authorizes. Only a delivered module is the tool's result; every other state is a refusal
//! that names it.

use std::path::Path;

use orchester_api::v2::{AuthorityErrorCode, DeliveryState, Subject};
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
  team_work::{DeliveryProgress, ModuleDeliveries, TeamDeliveryRequest, TeamDeliveryState, TeamWorkError},
};

/// The branches a delivery takes the module's work between, each at its tip in the checkout.
struct Tips {
  branch: BranchName,
  source_commit: CommitId,
  base_branch: BranchName,
  base_commit: CommitId,
}

/// Requests the delivery of the module run `run` of `team` worked in, or carries on the module's open one.
pub(in crate::app_mcp::tools) async fn merge_run(
  services: &Services,
  team: &Team,
  run: &str,
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
    let claim = acquire(&dispatcher, Subject::Module { id: module }, delivery_failed).await?;
    let request = TeamDeliveryRequest {
      operation: OperationId::generate(),
      delivery: DeliveryAction {
        module,
        claim,
        branch: tips.branch,
        source_commit: tips.source_commit,
        base_commit: tips.base_commit,
        step: DeliveryStep::Request { authorization: None },
      },
      base_branch: tips.base_branch,
    };
    deliver(&ModuleDeliveries::new(dispatcher, programs), request)
      .await
      .map_err(work_refused)
  })
  .await?;
  answer(progress, state.as_ref(), translate)
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

/// The module's `branch` and the branch checked out at `root`, each with its tip.
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
  let base_commit = branch_tip(runner, root, base_branch.as_str())?;
  Ok(Tips {
    // `branch_tip` read the branch by this name, so it is one.
    branch: BranchName::new(branch).expect("a branch whose tip was read"),
    source_commit,
    base_branch,
    base_commit,
  })
}

/// Carries on the module's open delivery, with the project's other open ones in their order, else requests `request`;
/// where it stands, and its record.
async fn deliver(
  deliveries: &ModuleDeliveries,
  request: TeamDeliveryRequest,
) -> Result<(DeliveryProgress, Option<TeamDeliveryState>), TeamWorkError> {
  let (operation, progress) = match deliveries.open_delivery(request.delivery.module)? {
    Some(open) => {
      let carried = deliveries
        .recover()
        .await?
        .into_iter()
        .find(|(operation, _)| *operation == open);
      // Another caller settled it between the read and the recovery: its record says how.
      let progress = carried.map_or(Err(TeamWorkError::UnreadableDelivery(open)), |(_, progress)| progress)?;
      (open, progress)
    }
    None => (request.operation, deliveries.request(request).await?),
  };
  Ok((progress, deliveries.delivery_state(operation)?))
}

/// The tool's answer for where the delivery stands.
fn answer(
  progress: DeliveryProgress,
  state: Option<&TeamDeliveryState>,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let refused = |code: &str, message: Message| ToolError::refused(code, vec![message]);
  match progress {
    DeliveryProgress::WorkRunning(runs) => Err(refused(
      "team_work_running",
      Message::new("appMcp.errors.teamDeliveryWorkRunning").with("runs", runs.len().to_string()),
    )),
    DeliveryProgress::Refused(error) if error.code() == AuthorityErrorCode::DeliveryNotAuthorized => Err(refused(
      "delivery_not_authorized",
      Message::new("appMcp.errors.teamDeliveryNotAuthorized"),
    )),
    DeliveryProgress::Refused(error) => Err(authority_refused(&error)),
    DeliveryProgress::Waiting(TeamDeliveryStage::Reporting) => Err(refused(
      "team_delivery_waiting",
      Message::new("appMcp.errors.teamDeliveryReportWaiting"),
    )),
    DeliveryProgress::Waiting(_) => Err(refused(
      "team_delivery_waiting",
      Message::new("appMcp.errors.teamDeliveryWaiting"),
    )),
    DeliveryProgress::MergeWaiting => Err(refused(
      "team_delivery_merge_waiting",
      Message::new("appMcp.errors.teamDeliveryMergeWaiting"),
    )),
    DeliveryProgress::Reported(delivery) => match delivery.state {
      DeliveryState::Delivered => Ok(delivered(state, translate)?),
      DeliveryState::Failed => Err(refused(
        "delivery_failed",
        Message::new("appMcp.errors.teamDeliveryFailed"),
      )),
      DeliveryState::Authorized | DeliveryState::Requested => Err(refused(
        "team_delivery_waiting",
        Message::new("appMcp.errors.teamDeliveryWaiting"),
      )),
    },
    // A report refused as not authorized is of a delivery the automatic merge's withdrawal failed.
    DeliveryProgress::ReportRefused(error) if error.code() == AuthorityErrorCode::DeliveryNotAuthorized => Err(
      refused("delivery_failed", Message::new("appMcp.errors.teamDeliveryWithdrawn")),
    ),
    DeliveryProgress::ReportRefused(error) => Err(refused(
      "delivery_report_refused",
      Message::new("appMcp.errors.teamDeliveryReportRefused").with("code", code_name(error.code())),
    )),
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

fn delivery_failed() -> ToolError {
  ToolError::refused(
    "team_delivery_failed",
    vec![Message::new("appMcp.errors.teamDeliveryCouldNotGoOn")],
  )
}
