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
  id::{ClaimId, OperationId},
  run::{CommitId, inputs::LocalConsent},
  store::ConnectionStore,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{
  Team,
  bindings::{Binding, bind, choice_refused},
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
  tasks::run::start_messages,
  team_reads::{TeamReadError, listed::TeamPipelines},
  team_work::{
    self, McpHere, ProjectRunner, StartChoices, StartFailure, StartOutcome, TeamStartRequest, TeamWork, TeamWorkError,
  },
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
  let Some(named) = task.task.content.pipeline.clone() else {
    return Err(ToolError::refused(
      "task_incomplete",
      vec![Message::new("appMcp.errors.teamNoPipeline")],
    ));
  };
  let module_read = match read(&project, project.module(module)).await {
    Ok(read) => read.into_value().module,
    Err(TeamReadError::NotFound) => return Err(ToolError::not_found(Missing::Module, module.to_string())),
    Err(error) => return Err(read_refused(&error)),
  };
  let (root, programs) = (team.open.repo_root.clone(), services.programs().clone());
  let branch = module_read.branch.clone();
  let (pipeline, base_commit, (connections, mcp)) = blocking(move || {
    let db = ProjectDb::open(&root)?;
    // Only the team's pipelines run a team task: a person's own local pipeline never does, even with the same id.
    let pipeline = TeamPipelines::of(&db)?
      .id(&named)
      .ok_or_else(|| pipeline_missing(&named))?;
    let commit = branch_tip(programs.runner(), &root, &branch)?;
    Ok((pipeline, commit, (db.connections()?, McpHere::read(&db)?)))
  })
  .await?;
  let choices = StartChoices {
    bindings: bind(&project, pipeline, (&connections, &mcp), given).await?,
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

/// The commit `branch` is at in the checkout at `root`: a module's branch is a start's base and a delivery's source.
pub(super) fn branch_tip(
  runner: &(dyn CommandRunner + Send + Sync),
  root: &Path,
  branch: &str,
) -> Result<CommitId, ToolError> {
  team_work::branch_tip(runner, root, branch).map_err(|error| choice_refused(&error))
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
fn started(outcome: StartOutcome) -> Result<Value, ToolError> {
  match outcome {
    StartOutcome::Started(run) => Ok(json!({ "run": { "id": run.to_string() }, "team": true })),
    StartOutcome::Waiting(_) => Err(ToolError::refused(
      "team_start_waiting",
      vec![Message::new("appMcp.errors.teamStartWaiting")],
    )),
    StartOutcome::Refused(error) => Err(authority_refused(&error)),
    StartOutcome::Failed(StartFailure::BundleSchema { read, built }) => Err(bundle_schema(Schemas { read, built })),
    StartOutcome::Failed(StartFailure::BundleUnreadable) => Err(ToolError::refused(
      "team_bundle_unreadable",
      vec![Message::new("appMcp.errors.teamBundleUnreadable")],
    )),
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

/// The bundle schema this computer reads and the one the team server built the run's bundle in.
struct Schemas {
  read: u32,
  built: u32,
}

/// The bundle is built in another schema than this computer reads: the older of the two is updated.
fn bundle_schema(Schemas { read, built }: Schemas) -> ToolError {
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
