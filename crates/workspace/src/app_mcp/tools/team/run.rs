//! `run_task` of a team task: its work starts through the team server, never from this computer's queue. The task is
//! read from the server, claimed for this session (`claim.acquire`, which answers the claim already held), and its
//! module read for the branch its work builds on; the branch's commit here is the start's base. Before anything is
//! claimed, the pipeline must be ready on this computer ([`crate::readiness`]): one that is not is refused as
//! `not_ready_here` with the missing parts, and no claim is taken, no work is asked for and no attempt is counted.
//! `work.start` goes through [`crate::team_work`], which keeps the start before each boundary, checks the versions the
//! bundle pins once more and asks the project's runner for the run of the accepted inputs. A start the server or the
//! runner cannot take now is recorded and carried on by recovery; the answer says where it waits.

use std::path::Path;

use orchester_api::v2::{AuthorityErrorCode, AuthorityRef, Subject, TeamCommandValue};
use orchester_core::{
  command::CommandRunner,
  id::{ClaimId, OperationId},
  run::{BranchName, CommitId},
};
use serde_json::{Value, json};

use super::{
  Team,
  bundle_missing::{self, claim_message},
  on_worker,
  outcome::{accepted, action_refused, authority_refused},
  read, read_refused,
  tasks::find,
};
use crate::{
  actions::{Accepted, Action, ActionOutcome, ClaimAction, Dispatcher, Intent, WorkAction},
  app_mcp::{
    Services,
    error::{Missing, ToolError},
    services::blocking,
  },
  messages::Message,
  readiness::{NotReadyHere, StartReadiness},
  runner::RunRequestError,
  tasks::run::start_messages,
  team_reads::{TeamReadError, listed::TeamPipelines},
  team_work::{
    self, BranchNotFound, ClaimEnd, ProjectRunner, StartFailure, StartOutcome, StartTurn, TeamStartRequest, TeamWork,
    TeamWorkError,
  },
};

pub(in crate::app_mcp::tools) async fn run_task(
  services: &Services,
  team: &Team,
  key: &str,
) -> Result<Value, ToolError> {
  let project = team.reads(services).await?;
  let task = find(&project, key).await?;
  let Some(named) = task.task.content.pipeline.clone() else {
    return Err(ToolError::refused(
      "task_incomplete",
      vec![Message::new("appMcp.errors.teamNoPipeline")],
    ));
  };
  // The task's module as the server reads it now, with its revision and its branch; none for a task in no module,
  // whose run starts on the branch this computer has checked out.
  let module = match task.task.content.module {
    Some(module) => match read(&project, project.module(module)).await {
      Ok(read) => {
        let read = read.into_value().module;
        Some((module, read.module.revision, read.branch))
      }
      Err(TeamReadError::NotFound) => return Err(ToolError::not_found(Missing::Module, module.to_string())),
      Err(error) => return Err(read_refused(&error)),
    },
    None => None,
  };
  let (root, programs) = (team.open.repo_root.clone(), services.programs().clone());
  let (workspace, vault) = (services.workspace().clone(), services.vault.clone());
  let start_vault = vault.clone();
  let branch = module.as_ref().map(|(_, _, branch)| branch.clone());
  // Only the team's pipelines, as this session read them, run a team task: a person's own local pipeline never does,
  // even with the same id.
  let config = workspace.team_read(&root).loaded()?;
  let ((base_commit, base_branch), programs) = blocking(move || {
    let pipeline = TeamPipelines::of(&config)
      .id(&named)
      .ok_or_else(|| pipeline_missing(&named))?;
    let base = start_base(programs.runner(), &root, branch.as_deref())?;
    match workspace
      .team_start_readiness(&root, vault.get(), pipeline)
      .map_err(ToolError::from)?
    {
      StartReadiness::Ready(programs) => Ok((base, programs)),
      StartReadiness::NotReady(refusal) => Err(ToolError::NotReadyHere(refusal)),
      StartReadiness::PipelineUnknown => Err(pipeline_missing(&named)),
    }
  })
  .await?;
  let dispatcher = team.dispatcher(services).await?;
  let runner = ProjectRunner::new(
    services.runners.clone(),
    team.open.repo_root.clone(),
    team.open.project.name().as_str().to_owned(),
  );
  let (id, read) = (task.task.id, task.task.revision);
  let module = module.map(|(module, read, _)| (module, read));
  let outcome = on_worker(move || async move {
    // The project's turn comes before the claim: the recovery of its other starts would otherwise release the claim
    // this one takes while its own start is not recorded yet.
    let turn = StartTurn::wait(dispatcher.servers(), dispatcher.root()).await;
    let claim_id = acquire(&dispatcher, Subject::Task { id }, start_failed).await?;
    let work = WorkAction {
      authority: AuthorityRef {
        subject: Subject::Task { id },
        claim_id,
        work_id: None,
      },
      base_commit,
      read,
      module,
    };
    let request = TeamStartRequest {
      operation: OperationId::generate(),
      work,
      base_branch,
      programs,
    };
    TeamWork::new(dispatcher, runner)
      .start_in(&turn, request, start_vault.get())
      .await
      .map_err(work_refused)
  })
  .await?;
  started(team, outcome).await
}

/// The base a start takes at `root`: the tip of its module's `branch`, or, for a task in no module, the branch the
/// checkout has checked out with its tip, which the run then merges into.
fn start_base(
  runner: &(dyn CommandRunner + Send + Sync),
  root: &Path,
  branch: Option<&str>,
) -> Result<(CommitId, Option<BranchName>), ToolError> {
  let Some(branch) = branch else {
    let not_found = |BranchNotFound(branch)| {
      ToolError::refused(
        "branch_not_found",
        vec![Message::new("appMcp.errors.branchNotFound").with("branch", branch)],
      )
    };
    return match team_work::checked_out_tip(runner, root).map_err(not_found)? {
      Some((branch, commit)) => Ok((commit, Some(branch))),
      None => Err(ToolError::refused(
        "no_base_branch",
        vec![Message::new("appMcp.errors.teamStartNoBranch")],
      )),
    };
  };
  Ok((branch_tip(runner, root, branch)?, None))
}

/// The commit `branch` is at in the checkout at `root`: a module's branch is a start's base and a delivery's source.
pub(super) fn branch_tip(
  runner: &(dyn CommandRunner + Send + Sync),
  root: &Path,
  branch: &str,
) -> Result<CommitId, ToolError> {
  team_work::branch_tip(runner, root, branch).map_err(|BranchNotFound(branch)| {
    ToolError::refused(
      "branch_not_found",
      vec![Message::new("appMcp.errors.branchNotFound").with("branch", branch)],
    )
  })
}

/// The claim of `subject` this session holds, taken now unless it held it already; `failed` when the server answered
/// with something other than a claim.
pub(super) async fn acquire(
  dispatcher: &Dispatcher,
  subject: Subject,
  failed: fn() -> ToolError,
) -> Result<ClaimId, ToolError> {
  let action = Action::Claim(ClaimAction::Acquire { subject });
  let outcome = dispatcher
    .dispatch(&Intent::new(action))
    .await
    .map_err(action_refused)?;
  let ActionOutcome::Team(outcome) = outcome else {
    return Err(failed());
  };
  match accepted(outcome)? {
    Accepted::Command(receipt) => match receipt.result {
      TeamCommandValue::Claim { claim, .. } => Ok(claim.id),
      _ => Err(failed()),
    },
    Accepted::Definition(_) => Err(failed()),
  }
}

/// The tool's answer for where the start stands.
async fn started(team: &Team, outcome: StartOutcome) -> Result<Value, ToolError> {
  match outcome {
    StartOutcome::Started(run) => Ok(json!({ "run": { "id": run.to_string() }, "team": true })),
    StartOutcome::Waiting(_) => Err(ToolError::refused(
      "team_start_waiting",
      vec![Message::new("appMcp.errors.teamStartWaiting")],
    )),
    StartOutcome::Refused(error, claim) if error.code() == AuthorityErrorCode::BundleMissing => {
      Err(bundle_missing::refused(team, error.details(), claim).await)
    }
    StartOutcome::Refused(error, _) => Err(authority_refused(&error)),
    StartOutcome::Failed(StartFailure::BundleSchema { read, built }, claim) => {
      Err(bundle_schema(Schemas { read, built }, claim))
    }
    StartOutcome::Failed(StartFailure::BundleUnreadable, claim) => Err(ToolError::refused(
      "team_bundle_unreadable",
      vec![Message::new("appMcp.errors.teamBundleUnreadable"), claim_message(claim)],
    )),
    StartOutcome::Failed(StartFailure::NotReadyHere { pipeline, missing }, claim) => Err(ToolError::NotReadyHere(
      NotReadyHere::after_start(pipeline, missing, claim),
    )),
    StartOutcome::Failed(failure, claim) => {
      tracing::warn!(?failure, "a team start failed for good");
      Err(ToolError::refused(
        "team_start_failed",
        vec![Message::new("appMcp.errors.teamStartFailed"), claim_message(claim)],
      ))
    }
    StartOutcome::RunnerRefused(rejection) => {
      let error = RunRequestError::Rejected(rejection);
      Err(ToolError::runner(&error, start_messages(&error)))
    }
    StartOutcome::RunnerUnavailable(request) => {
      let error = RunRequestError::Request(request);
      Err(ToolError::runner(&error, start_messages(&error)))
    }
  }
}

/// The bundle schema this computer reads and the one the team server built the run's bundle in.
struct Schemas {
  read: u32,
  built: u32,
}

/// The bundle is built in another schema than this computer reads: the older of the two is updated.
fn bundle_schema(Schemas { read, built }: Schemas, claim: ClaimEnd) -> ToolError {
  let key = if built > read {
    "appMcp.errors.teamBundleSchemaNewer"
  } else {
    "appMcp.errors.teamBundleSchemaOlder"
  };
  ToolError::refused(
    "team_bundle_schema_mismatch",
    vec![
      Message::new(key)
        .with("read", read.to_string())
        .with("built", built.to_string()),
      claim_message(claim),
    ],
  )
}

fn work_refused(error: TeamWorkError) -> ToolError {
  match error {
    TeamWorkError::Action(error) => action_refused(error),
    TeamWorkError::Store(error) => error.into(),
    error => {
      tracing::warn!(%error, "a team start could not go on");
      start_failed()
    }
  }
}

/// The task names `named`, which is none of the team's pipelines this computer has: refused before anything is sent.
fn pipeline_missing(named: &str) -> ToolError {
  ToolError::refused(
    "team_pipeline_missing",
    vec![Message::new("appMcp.errors.teamPipelineMissing").with("pipeline", named.to_owned())],
  )
}

fn start_failed() -> ToolError {
  ToolError::refused("team_start_failed", vec![Message::new("appMcp.errors.teamStartFailed")])
}
