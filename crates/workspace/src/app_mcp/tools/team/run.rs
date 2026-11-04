//! `run_task` of a team task: its work starts through the team server, never from this computer's queue. The task is
//! read from the server, claimed for this session (`claim.acquire`, which answers the claim already held), and its
//! module read for the branch its work builds on; the branch's commit here is the start's base. `work.start` goes
//! through [`crate::team_work`], which keeps the start before each boundary and asks the project's runner for the run
//! of the accepted inputs. A start the server or the runner cannot take now is recorded and carried on by recovery; the
//! answer says where it waits.

use std::path::Path;

use orchester_api::v2::{AuthorityRef, Subject, TeamCommandValue};
use orchester_core::{
  command::CommandRunner,
  id::{ClaimId, OperationId, ResourceId, TaskId},
  integration::base::set_branch_tip,
  run::{BranchName, CommitId, inputs::LocalConsent},
  store::ConnectionStore,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{
  Team,
  bindings::{Binding, bind},
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
  runner::RunRequestError,
  tasks::{changes::GIT, run::start_messages},
  team_reads::TeamReadError,
  team_work::{ProjectRunner, StartChoices, StartOutcome, TeamStartRequest, TeamWork, TeamWorkError},
};

pub(in crate::app_mcp::tools) async fn run_task(
  services: &Services,
  team: &Team,
  key: &str,
  given: &[Binding],
) -> Result<Value, ToolError> {
  let project = team.reads(services).await?;
  let task = find(&project, key).await?;
  let Some(module) = task.task.content.module else {
    return Err(ToolError::refused(
      "module_required",
      vec![Message::new("appMcp.errors.teamModuleRequired")],
    ));
  };
  let pipeline: ResourceId = task
    .task
    .content
    .pipeline
    .as_deref()
    .and_then(|pipeline| pipeline.parse().ok())
    .ok_or_else(|| ToolError::refused("task_incomplete", vec![Message::new("appMcp.errors.teamNoPipeline")]))?;
  let module_read = match read(&project, project.module(module)).await {
    Ok(read) => read.into_value().module,
    Err(TeamReadError::NotFound) => return Err(ToolError::not_found(Missing::Module, module.to_string())),
    Err(error) => return Err(read_refused(&error)),
  };
  let (root, programs) = (team.open.repo_root.clone(), services.programs().clone());
  let branch = module_read.branch.clone();
  let (base_commit, connections) = blocking(move || {
    let commit = base_commit(programs.runner(), &root, &branch)?;
    Ok((commit, ProjectDb::open(&root)?.connections()?))
  })
  .await?;
  let choices = StartChoices {
    bindings: bind(&project, pipeline, &connections, given).await?,
    consent: LocalConsent::default(),
  };
  let dispatcher = team.dispatcher(services).await?;
  let runner = ProjectRunner::new(
    services.runners.clone(),
    team.open.repo_root.clone(),
    team.open.project.name().as_str().to_owned(),
  );
  let (id, read) = (task.task.id, task.task.revision);
  let module_read = module_read.module.revision;
  let outcome = on_worker(move || async move {
    let claim_id = acquire(&dispatcher, id).await?;
    let work = WorkAction {
      authority: AuthorityRef {
        subject: Subject::Task { id },
        claim_id,
        work_id: None,
      },
      base_commit,
      read,
      module,
      module_read,
    };
    let request = TeamStartRequest {
      operation: OperationId::generate(),
      work,
      choices,
    };
    TeamWork::new(dispatcher, runner)
      .start(request)
      .await
      .map_err(work_refused)
  })
  .await?;
  started(outcome)
}

/// The commit the module's `branch` is at in the checkout at `root`, the start's base.
fn base_commit(runner: &(dyn CommandRunner + Send + Sync), root: &Path, branch: &str) -> Result<CommitId, ToolError> {
  let missing = || {
    ToolError::refused(
      "branch_not_found",
      vec![Message::new("appMcp.errors.branchNotFound").with("branch", branch.to_owned())],
    )
  };
  let name = BranchName::new(branch).map_err(|_| missing())?;
  let revision = set_branch_tip(&runner, Path::new(GIT), (root, &name)).map_err(|error| {
    tracing::warn!(%error, "cannot read a team module's branch");
    missing()
  })?;
  CommitId::new(revision.as_str()).map_err(|_| missing())
}

/// The claim of `task` this session holds, taken now unless it held it already.
async fn acquire(dispatcher: &Dispatcher, task: TaskId) -> Result<ClaimId, ToolError> {
  let action = Action::Claim(ClaimAction::Acquire {
    subject: Subject::Task { id: task },
  });
  let outcome = dispatcher
    .dispatch(&Intent::new(action))
    .await
    .map_err(action_refused)?;
  let ActionOutcome::Team(outcome) = outcome else {
    return Err(start_failed());
  };
  match accepted(outcome)? {
    Accepted::Command(receipt) => match receipt.result {
      TeamCommandValue::Claim { claim, .. } => Ok(claim.id),
      _ => Err(start_failed()),
    },
    Accepted::Definition(_) => Err(start_failed()),
  }
}

/// The tool's answer for where the start stands.
fn started(outcome: StartOutcome) -> Result<Value, ToolError> {
  match outcome {
    StartOutcome::Started(run) => Ok(json!({ "run": { "id": run.to_string() }, "team": true })),
    StartOutcome::Waiting(_) => Err(ToolError::refused(
      "team_start_waiting",
      vec![Message::new("appMcp.errors.teamStartWaiting")],
    )),
    StartOutcome::Refused(error) => Err(authority_refused(&error)),
    StartOutcome::Failed(failure) => {
      tracing::warn!(?failure, "a team start failed for good");
      Err(start_failed())
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

fn start_failed() -> ToolError {
  ToolError::refused("team_start_failed", vec![Message::new("appMcp.errors.teamStartFailed")])
}
