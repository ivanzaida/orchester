//! Run on a team task's page (`docs/design/team-authority.md` §2, ORC-725's `run_task` rule): its work starts through
//! the team server, never from this computer's queue. The task's claim is this computer's or taken now
//! (`claim.acquire`, through [`super::act`]); the module's branch tip here is the start's base; the pipeline's profiles
//! run on the person's local connections, chosen as `run_task` chooses them (`orchester_workspace::team_work::bind`);
//! then `work.start` goes through `team_work`, which keeps the start before each boundary and asks the project's
//! runner for the run of the accepted inputs. A start the server or the runner cannot take now waits and is carried on
//! by recovery; the page says so.

use std::path::PathBuf;

use orchester_api::v2::{AuthorityErrorCode, AuthorityRef, Subject};
use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, ResourceId, RunId, TaskId},
  run::inputs::LocalConsent,
  sharing::TaskRevision,
  store::ConnectionStore,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::{
  actions::{Action, ClaimAction, Dispatcher, WorkAction},
  background,
  messages::Message,
  runner::{RunRequestError, Runners},
  tasks::run::start_messages,
  team_work::{
    self, ChoiceError, McpHere, ProjectRunner, StartChoices, StartOutcome, TeamStartRequest, TeamWork, TeamWorkError,
  },
};

use super::{
  TeamSource,
  act::{Acted, Acting},
};

/// What the start of a team task's work needs.
#[derive(Clone, Debug)]
pub struct StartTask {
  /// The task's id on the server, and its revision as read.
  pub task: (TaskId, TaskRevision),
  /// The claim this computer holds on it; `None` takes one.
  pub claim: Option<ClaimId>,
  /// The task's module, its revision as read and its branch; `None` for a task in no module.
  pub module: Option<(ModuleId, TaskRevision, String)>,
  /// The task's pipeline on this computer.
  pub pipeline: Option<ResourceId>,
  /// The project's name, as its runner is named.
  pub project: String,
}

/// Where a start stands, as the page says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Started {
  /// The runner started the run.
  Run(RunId),
  /// The server or the runner cannot take it now: the start waits, and goes on from where it stands.
  Waiting,
  /// It did not start, and why.
  Stopped(Stop),
}

impl lurq::app::component::DevtoolsInspectable for Started {}

/// Why a team task's work did not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
  /// The task's claim could not be taken.
  Claim(Acted),
  /// The task is in no module, or names no pipeline this computer has.
  NotReady,
  /// The server refused the start for good.
  Refused(AuthorityErrorCode),
  /// The module's branch is not in this checkout.
  BranchNotFound(String),
  /// These profiles, by role, have no connection of their CLI here, or several.
  BindingsRequired(Vec<String>),
  /// These MCP servers, by name, that the pipeline's profiles use are not set up here.
  McpServersRequired(Vec<String>),
  /// The team server could not read the pipeline or a profile, or they have no published version.
  NotReadable,
  /// The runner refused the run.
  Runner(Vec<Message>),
  /// It could not be carried out here.
  Failed,
}

/// Starts `request`'s task on `source`'s project, with `runners` and `programs`.
pub async fn start(
  acting: &Acting,
  source: &TeamSource,
  (runners, programs): (Runners, Programs),
  request: StartTask,
) -> Started {
  let (Some(pipeline), Some((module, module_read, branch))) = (request.pipeline, request.module.clone()) else {
    return Started::Stopped(Stop::NotReady);
  };
  let (task, read) = request.task;
  let claim = match request.claim {
    Some(claim) => claim,
    None => match acting
      .act(
        source,
        Action::Claim(ClaimAction::Acquire {
          subject: Subject::Task { id: task },
        }),
      )
      .await
    {
      (Acted::Done, Some(claim)) => claim,
      (acted, _) => return Started::Stopped(Stop::Claim(acted)),
    },
  };
  let root = source.root().to_owned();
  let choices = match choices(source, (root.clone(), programs), (pipeline, &branch)).await {
    Ok(choices) => choices,
    Err(stop) => return Started::Stopped(stop),
  };
  let (base_commit, bindings) = choices;
  let work = WorkAction {
    authority: AuthorityRef {
      subject: Subject::Task { id: task },
      claim_id: claim,
      work_id: None,
    },
    base_commit,
    read,
    module,
    module_read,
  };
  let start = TeamStartRequest {
    operation: OperationId::generate(),
    work,
    choices: StartChoices {
      bindings,
      consent: LocalConsent::default(),
    },
  };
  let runner = ProjectRunner::new(runners, root.clone(), request.project);
  carry_out(acting, source, (runner, root), start).await
}

/// The start's base, the module branch's tip here, and the bindings of the pipeline's profiles on this computer's
/// connections.
async fn choices(
  source: &TeamSource,
  (root, programs): (PathBuf, Programs),
  (pipeline, branch): (ResourceId, &str),
) -> Result<
  (
    orchester_core::run::CommitId,
    orchester_core::run::inputs::LocalBindings,
  ),
  Stop,
> {
  let branch = branch.to_owned();
  let read = background::run(move || {
    let commit = team_work::branch_tip(programs.runner(), &root, &branch).map_err(|error| chosen(&error))?;
    let here = ProjectDb::open(&root)
      .and_then(|db| Ok((db.connections()?, McpHere::read(&db)?)))
      .map_err(|error| {
        tracing::warn!(%error, "the project's connections cannot be read for a team start");
        Stop::Failed
      })?;
    Ok((commit, here))
  })
  .await
  .map_err(|_| Stop::Failed)?;
  let (commit, (connections, mcp)) = read?;
  let bindings = team_work::bind(source.project(), pipeline, (&connections, &mcp), &[])
    .await
    .map_err(|error| chosen(&error))?;
  Ok((commit, bindings))
}

/// The start through the team work, on a worker: its futures hold the project database across their awaits.
async fn carry_out(
  acting: &Acting,
  source: &TeamSource,
  (runner, root): (ProjectRunner, PathBuf),
  start: TeamStartRequest,
) -> Started {
  let (servers, key, runtime) = (acting.servers.clone(), source.key(), acting.runtime.clone());
  let outcome = background::run(move || {
    runtime.block_on(async move {
      let session = servers
        .team_session(key.server, key.project)
        .await
        .map_err(|_| Stop::Claim(Acted::SignedOut))?;
      let dispatcher = Dispatcher::new(root, servers, Some(session));
      TeamWork::new(dispatcher, runner)
        .start(start)
        .await
        .map_err(|error| work_failed(&error))
    })
  })
  .await;
  match outcome {
    Ok(Ok(outcome)) => started(outcome),
    Ok(Err(stop)) => Started::Stopped(stop),
    Err(_) => Started::Stopped(Stop::Failed),
  }
}

fn started(outcome: StartOutcome) -> Started {
  match outcome {
    StartOutcome::Started(run) => Started::Run(run),
    StartOutcome::Waiting(_) => Started::Waiting,
    StartOutcome::Refused(error) => Started::Stopped(Stop::Refused(error.code())),
    StartOutcome::Failed(failure) => {
      tracing::warn!(?failure, "a team start failed for good");
      Started::Stopped(Stop::Failed)
    }
    StartOutcome::RunnerRefused(rejection) => {
      Started::Stopped(Stop::Runner(start_messages(&RunRequestError::Rejected(rejection))))
    }
    StartOutcome::RunnerUnavailable(_) => Started::Waiting,
  }
}

fn chosen(error: &ChoiceError) -> Stop {
  match error {
    ChoiceError::BranchNotFound(branch) => Stop::BranchNotFound(branch.clone()),
    ChoiceError::BindingsRequired(profiles) => Stop::BindingsRequired(profiles.clone()),
    ChoiceError::McpServersRequired(servers) => Stop::McpServersRequired(servers.clone()),
    ChoiceError::Read(_)
    | ChoiceError::NotPublished
    | ChoiceError::Unreadable
    | ChoiceError::LegacyProfileVersion
    | ChoiceError::ConnectionNotFound(_) => {
      tracing::warn!(%error, "a team start's choices cannot be made");
      Stop::NotReadable
    }
  }
}

fn work_failed(error: &TeamWorkError) -> Stop {
  tracing::warn!(%error, "a team start could not go on");
  Stop::Failed
}
