//! Run on a team task's page (`docs/design/team-authority.md` §2, ORC-725's `run_task` rule): its work starts through
//! the team server, never from this computer's queue. The pipeline must be ready on this computer
//! (`orchester_workspace::readiness`), checked first: one that is not starts nothing, claims nothing and says what is
//! missing (`not_ready_here`, as `run_task` refuses it). The task's claim is this computer's or taken now
//! (`claim.acquire`, through [`super::act`]); the module's branch tip here is the start's base; then `work.start` goes
//! through `team_work`, which keeps the start before each boundary, binds the run to what this computer filled in and
//! asks the project's runner for the run of the accepted inputs. A start the server or the runner cannot take now waits
//! and is carried on by recovery; the page says so.

use std::path::{Path, PathBuf};

use orchester_api::v2::{AuthorityError, AuthorityErrorCode, AuthorityRef, ErrorDetails, Subject};
use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, ResourceId, RunId, TaskId},
  run::{BranchName, CommitId},
  sharing::TaskRevision,
};
use orchester_exec::programs::Programs;
use orchester_ipc::Rejection;
use orchester_workspace::{
  actions::{Action, ClaimAction, Dispatcher, WorkAction},
  background,
  messages::Message,
  readiness::{McpPrograms, NotReadyHere, StartReadiness},
  runner::{RunRequestError, Runners},
  tasks::run::start_messages,
  team_work::{
    self, BranchNotFound, ClaimEnd, LastStart, NewestStart, ProjectRunner, StartFailure, StartOutcome, StartTurn,
    TeamStartRequest, TeamWork, TeamWorkError, bundle_missing_message,
  },
};

use super::{
  TeamSource,
  act::{Acted, Acting},
  invalidation::Stale,
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
  /// The task names no pipeline this computer has.
  NotReady,
  /// The task is in no module, and this computer has no branch checked out for its run to start on and merge into.
  NoBranch,
  /// The server refused the start for good; what became of the claim when the refusal released it.
  Refused(AuthorityErrorCode, Option<ClaimEnd>),
  /// The module's branch is not in this checkout.
  BranchNotFound(String),
  /// The pipeline is not ready on this computer: what is missing, each part with its fix, which the task's Summary
  /// lists. Before the server was asked for anything nothing was claimed; a version the server pinned that this
  /// computer is not ready for is found after it made the work ([`NotReadyHere::claim_end`] says what became of the
  /// claim).
  NotReadyHere(NotReadyHere),
  /// The team server could not read the pipeline or a profile, or they have no published version.
  NotReadable,
  /// The team server built no bundle for the start (`bundle_missing`) and says why, as a message that tells what to
  /// do; no work was made. The claim ended as `claim` says.
  BundleMissing(Message, Option<ClaimEnd>),
  /// The bundle the server built is in a schema this computer does not read (`read`); the older of the two is
  /// updated. The claim ended as `claim` says.
  BundleSchema { read: u32, built: u32, claim: ClaimEnd },
  /// The server answered something for the bundle that this computer cannot read.
  BundleUnreadable(ClaimEnd),
  /// The bundle or one of its files is not the one the server announced.
  BundleMismatch(ClaimEnd),
  /// The bundle does not make the run's inputs on this computer (a profile without a connection, …).
  InputsFailed(ClaimEnd),
  /// The runner refused the run.
  Runner(Vec<Message>),
  /// The start's record was written by a newer Orchester: this one cannot carry it on, and leaves it open.
  StartUnreadable,
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
  let Some(pipeline) = request.pipeline else {
    return Started::Stopped(Stop::NotReady);
  };
  let root = source.root().to_owned();
  let branch = request.module.as_ref().map(|(_, _, branch)| branch.clone());
  let checked = checked(acting, (root.clone(), programs), (pipeline, branch)).await;
  let ((base_commit, base_branch), mcp_programs) = match checked {
    Ok(checked) => checked,
    Err(stop) => return Started::Stopped(stop),
  };
  // The project's turn comes before the claim: the recovery of its other starts would otherwise release the claim
  // this Run takes (or already holds) while its own start is not recorded yet.
  let turn = StartTurn::wait(&acting.servers, &root).await;
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
  let work = WorkAction {
    authority: AuthorityRef {
      subject: Subject::Task { id: task },
      claim_id: claim,
      work_id: None,
    },
    base_commit,
    read,
    module: request.module.map(|(module, read, _)| (module, read)),
  };
  let start = TeamStartRequest {
    operation: OperationId::generate(),
    work,
    base_branch,
    programs: mcp_programs,
  };
  let runner = ProjectRunner::new(runners, root.clone(), request.project);
  let started = carry_out(acting, source, (runner, root), (start, turn)).await;
  // The server answered, and may have let go of the claim: the task is read again, so a Run pressed next does not
  // name the claim it released.
  Stale::EVERY.apply(&acting.team.client());
  started
}

/// A start's base: the commit it starts from, and, for a task in no module, the branch that commit was read from.
type Base = (CommitId, Option<BranchName>);

/// The start's base and the program paths set for the team's MCP servers; or why the pipeline does not start on this
/// computer. The base is the tip of the module's `branch` here, or, for a task in no module, the branch this computer
/// has checked out with its tip.
async fn checked(
  acting: &Acting,
  (root, programs): (PathBuf, Programs),
  (pipeline, branch): (ResourceId, Option<String>),
) -> Result<(Base, McpPrograms), Stop> {
  let (workspace, vault) = (acting.workspace.clone(), acting.vault.clone());
  background::run(move || {
    let not_found = |BranchNotFound(branch)| Stop::BranchNotFound(branch);
    let base = match &branch {
      Some(branch) => (
        team_work::branch_tip(programs.runner(), &root, branch).map_err(not_found)?,
        None,
      ),
      None => {
        let (branch, commit) = team_work::checked_out_tip(programs.runner(), &root)
          .map_err(not_found)?
          .ok_or(Stop::NoBranch)?;
        (commit, Some(branch))
      }
    };
    let ready = workspace
      .team_start_readiness(&root, vault.get(), pipeline)
      .map_err(|error| {
        tracing::warn!(%error, "the readiness of a team pipeline cannot be read for a start");
        Stop::Failed
      })?;
    match ready {
      StartReadiness::Ready(programs) => Ok((base, programs)),
      StartReadiness::NotReady(refusal) => Err(Stop::NotReadyHere(refusal)),
      StartReadiness::PipelineUnknown => Err(Stop::NotReady),
    }
  })
  .await
  .map_err(|_| Stop::Failed)?
}

/// The start through the team work, in the project's `turn`, on a worker: its futures hold the project database across
/// their awaits.
async fn carry_out(
  acting: &Acting,
  source: &TeamSource,
  (runner, root): (ProjectRunner, PathBuf),
  (start, turn): (TeamStartRequest, StartTurn),
) -> Started {
  let (servers, key, runtime) = (acting.servers.clone(), source.key(), acting.runtime.clone());
  let vault = acting.vault.clone();
  let outcome = background::run(move || {
    runtime.block_on(async move {
      let session = servers
        .team_session(key.server, key.project)
        .await
        .map_err(|_| Stop::Claim(Acted::SignedOut))?;
      let dispatcher = Dispatcher::new(root.clone(), servers, Some(session));
      let outcome = TeamWork::new(dispatcher, runner)
        .start_in(&turn, start, vault.get())
        .await
        .map_err(|error| work_failed(&error))?;
      Ok(started(outcome, &root))
    })
  })
  .await;
  match outcome {
    Ok(Ok(started)) => started,
    Ok(Err(stop)) => Started::Stopped(stop),
    Err(_) => Started::Stopped(Stop::Failed),
  }
}

/// A task's newest start as the project's records tell it, and whether the page tells it to a window that asked for
/// nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
  /// The start this is the record of, which a dismissal names.
  pub operation: OperationId,
  pub started: Started,
  pub telling: Telling,
}

impl lurq::app::component::DevtoolsInspectable for Recorded {}

/// How a recorded start is told to a window that asked for nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Telling {
  /// The start is not settled, or its run started: what it waits for, or what the run is, stands for as long as it
  /// holds.
  Standing,
  /// Settled for good, and not told yet: the page tells it once, until the person dismisses it or runs the task again.
  Once,
  /// Settled for good and told, or its reason no longer holds: the page does not tell it again.
  Told,
}

/// Where the task's newest start stands as the project's records tell it, e.g. after recovery settled one that
/// waited; as [`started`] reads the cached name of a resource a refusal names. Blocks on SQLite.
pub fn recorded(NewestStart { operation, last }: NewestStart, root: &Path) -> Recorded {
  let (started, telling) = match last {
    LastStart::Open => (Started::Waiting, Telling::Standing),
    LastStart::RunnerRefused(rejection) => (runner_refused(rejection), Telling::Standing),
    LastStart::Settled(outcome) => {
      let telling = if matches!(outcome, StartOutcome::Started(_)) {
        Telling::Standing
      } else {
        Telling::Once
      };
      (started(outcome, root), telling)
    }
    LastStart::Told(outcome) => (started(outcome, root), Telling::Told),
    LastStart::Unreadable => (Started::Stopped(Stop::StartUnreadable), Telling::Standing),
  };
  Recorded {
    operation,
    started,
    telling,
  }
}

fn runner_refused(rejection: Rejection) -> Started {
  Started::Stopped(Stop::Runner(start_messages(&RunRequestError::Rejected(rejection))))
}

/// Where `outcome` leaves the start, as the page says it; reads the cached name of a resource a refusal names from the
/// project at `root`. Blocks on SQLite.
fn started(outcome: StartOutcome, root: &Path) -> Started {
  match outcome {
    StartOutcome::Started(run) => Started::Run(run),
    StartOutcome::Waiting(_) => Started::Waiting,
    StartOutcome::Refused(error, claim) => Started::Stopped(refused(&error, claim, root)),
    StartOutcome::Failed(failure, claim) => Started::Stopped(failed(failure, claim)),
    StartOutcome::RunnerRefused(rejection) => runner_refused(rejection),
    StartOutcome::RunnerUnavailable(_) => Started::Waiting,
  }
}

/// Why a start the server refused for good did not start, as the page says it.
fn refused(error: &AuthorityError, claim: Option<ClaimEnd>, root: &Path) -> Stop {
  if error.code() != AuthorityErrorCode::BundleMissing {
    return Stop::Refused(error.code(), claim);
  }
  let reason = match error.details() {
    ErrorDetails::BundleMissing { reason } => Some(reason),
    _ => None,
  };
  match bundle_missing_message(root, reason) {
    Ok(message) => Stop::BundleMissing(message, claim),
    Err(error) => {
      tracing::warn!(%error, "the resource a bundle_missing refusal names cannot be read");
      Stop::Refused(AuthorityErrorCode::BundleMissing, claim)
    }
  }
}

/// Why a start that failed for good did not start, as the page says it.
fn failed(failure: StartFailure, claim: ClaimEnd) -> Stop {
  match failure {
    StartFailure::NotReadyHere { pipeline, missing } => {
      Stop::NotReadyHere(NotReadyHere::after_start(pipeline, missing, claim))
    }
    StartFailure::BundleSchema { read, built } => Stop::BundleSchema { read, built, claim },
    StartFailure::BundleUnreadable => Stop::BundleUnreadable(claim),
    StartFailure::Bundle(why) => {
      tracing::warn!(%why, "a team start's bundle is not the announced one");
      Stop::BundleMismatch(claim)
    }
    StartFailure::Inputs(why) => {
      tracing::warn!(%why, "a team start's bundle does not make the run's inputs");
      Stop::InputsFailed(claim)
    }
  }
}

fn work_failed(error: &TeamWorkError) -> Stop {
  tracing::warn!(%error, "a team start could not go on");
  Stop::Failed
}
