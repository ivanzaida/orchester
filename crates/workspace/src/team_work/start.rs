//! A team start, carried forward from the boundary its record stands at: `work.start` through the dispatcher, the
//! receipt's work and bundle reference kept, the bundle read, checked and kept as the run's accepted inputs, then the
//! runner's run. Every step is kept before the next, and each is idempotent, so carrying on after a crash, a lost
//! answer or an outage makes no second work and no second run.

use orchester_api::v2::{AuthorityError, AuthorityRef, Subject, TeamCommandValue};
use orchester_core::{
  id::{ModuleId, OperationId, RunId},
  run::{CommitId, inputs::TeamAcceptance},
  sharing::TaskRevision,
  task::identity::TeamTaskRef,
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError, TeamOperationState, TeamSettlement, TeamStart, TeamStartStage};
use serde::{Deserialize, Serialize};

use super::{
  Answer, StartChoices, TeamRunner, TeamWork, TeamWorkError,
  bundle::{self, Expected},
};
use crate::{
  actions::{Accepted, Action, ActionOutcome, Intent, Refusal, TeamOutcome, WorkAction, team_task},
  runner::{RequestError, RunRequestError},
};

/// A start of a team task's work from the claim its caller holds: the work action under the intent's operation, and
/// what the caller read and chose for the run.
#[derive(Clone, Debug)]
pub struct TeamStartRequest {
  pub operation: OperationId,
  pub work: WorkAction,
  pub choices: StartChoices,
}

/// Where a start stands after it was carried on.
#[derive(Debug)]
pub enum StartOutcome {
  /// The runner started the run, now or before.
  Started(RunId),
  /// The server cannot be reached, or refused for now: the start waits at this boundary and goes on from it.
  Waiting(TeamStartStage),
  /// The server refused the start for good: it made no work, or the authority the bundle was built for is gone.
  Refused(AuthorityError),
  /// The runner refused to run it on this computer; the accepted inputs stay kept for another try.
  RunnerRefused(Rejection),
  /// The runner could not be asked now; the start goes on from the kept inputs.
  RunnerUnavailable(RequestError),
}

/// What a start keeps of its request: the work action, with the server's id of its task, and the caller's choices.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeptStart {
  authority: AuthorityRef,
  base_commit: CommitId,
  read: TaskRevision,
  module: ModuleId,
  module_read: TaskRevision,
  choices: StartChoices,
}

impl KeptStart {
  fn work(&self) -> WorkAction {
    WorkAction {
      authority: self.authority,
      base_commit: self.base_commit.clone(),
      read: self.read,
      module: self.module,
      module_read: self.module_read,
    }
  }
}

/// The record a step leaves the start at, or the outcome it stops with.
type Step = Result<TeamStart, StartOutcome>;

impl<R: TeamRunner> TeamWork<R> {
  /// Starts the work `request` asks for: recorded here first, then carried on as far as it goes now. Sending the same
  /// request again carries on the recorded start; the same operation with another request is refused.
  pub async fn start(&self, request: TeamStartRequest) -> Result<StartOutcome, TeamWorkError> {
    let record = self.record(request)?;
    self.carry_on(record).await
  }

  /// Carries on every start not started yet, oldest first, e.g. after a restart.
  pub async fn recover(&self) -> Result<Vec<(OperationId, StartOutcome)>, TeamWorkError> {
    let open = self.db()?.open_team_starts()?;
    let mut outcomes = Vec::with_capacity(open.len());
    for start in open {
      let operation = start.operation;
      outcomes.push((operation, self.carry_on(start).await?));
    }
    Ok(outcomes)
  }

  fn record(&self, request: TeamStartRequest) -> Result<TeamStart, TeamWorkError> {
    let mut db = self.db()?;
    let (link, _) = self.dispatcher.team(&db)?;
    let task = bundle::task_subject(request.work.authority.subject).ok_or(TeamWorkError::NotATask)?;
    let team = team_task(&db, link, task)?;
    let identity = TeamTaskRef {
      server: link.server,
      project: link.project,
      task: team,
    };
    let local = db.record_team_task(identity, request.work.read)?;
    let kept = KeptStart {
      authority: AuthorityRef {
        subject: Subject::Task { id: team },
        ..request.work.authority
      },
      base_commit: request.work.base_commit,
      read: request.work.read,
      module: request.work.module,
      module_read: request.work.module_read,
      choices: request.choices,
    };
    // The kept start's types serialize to JSON without fail: they hold no map with non-string keys.
    let text = serde_json::to_string(&kept).expect("a team start serializes");
    Ok(db.begin_team_start((request.operation, local), &text, Timestamp::now())?)
  }

  async fn carry_on(&self, mut record: TeamStart) -> Result<StartOutcome, TeamWorkError> {
    let kept: KeptStart =
      serde_json::from_str(&record.request).map_err(|_| TeamWorkError::Unreadable(record.operation))?;
    loop {
      let step = match record.stage {
        TeamStartStage::Requested => self.request_work(&record, &kept).await?,
        TeamStartStage::Accepted => self.keep_inputs(&record, kept.clone()).await?,
        TeamStartStage::InputsStored => return self.start_run(record.operation).await,
        TeamStartStage::Started => {
          let run = record.run.ok_or(TeamWorkError::Unreadable(record.operation))?;
          return Ok(StartOutcome::Started(run));
        }
        TeamStartStage::Refused => return refused(&record),
      };
      match step {
        Ok(next) => record = next,
        Err(outcome) => return Ok(outcome),
      }
    }
  }

  /// `work.start` under the start's operation, looked up rather than sent again when it was sent before; its
  /// receipt's work and bundle reference kept.
  async fn request_work(&self, record: &TeamStart, kept: &KeptStart) -> Result<Step, TeamWorkError> {
    let operation = record.operation;
    let intent = Intent::with_operation(operation, Action::Work(kept.work()));
    let ActionOutcome::Team(outcome) = self.dispatcher.dispatch(&intent).await? else {
      return Err(TeamWorkError::NotAWork(operation));
    };
    let receipt = match outcome {
      TeamOutcome::AcceptedPending { receipt, .. } | TeamOutcome::Completed(Accepted::Command(receipt)) => receipt,
      TeamOutcome::Rejected {
        refusal: Refusal::Command(error),
        retryable: false,
        ..
      } => return self.refuse(operation, error).map(Err),
      TeamOutcome::Rejected { .. } | TeamOutcome::AcceptanceUnknown { .. } => {
        return Ok(Err(StartOutcome::Waiting(TeamStartStage::Requested)));
      }
      TeamOutcome::Completed(Accepted::Definition(_)) | TeamOutcome::LocalEffectFailed { .. } => {
        return Err(TeamWorkError::NotAWork(operation));
      }
    };
    let TeamCommandValue::Work { work, bundle, .. } = &receipt.result else {
      return Err(TeamWorkError::NotAWork(operation));
    };
    let accepted = self
      .db()?
      .accept_team_start(operation, (work.id, &bundle.sha256), Timestamp::now())?;
    Ok(Ok(accepted))
  }

  /// Reads the bundle the receipt named with every file it pins, checks them, and keeps them as the run's accepted
  /// inputs with the start's record.
  async fn keep_inputs(&self, record: &TeamStart, kept: KeptStart) -> Result<Step, TeamWorkError> {
    let operation = record.operation;
    let (work, sha256) = record.work.clone().ok_or(TeamWorkError::Unreadable(operation))?;
    let (link, session) = self.dispatcher.team(&self.db()?)?;
    let expected = Expected {
      sha256: &sha256,
      authority: kept.authority,
      base_commit: &kept.base_commit,
    };
    let fetched = match bundle::fetch(&session.authority(), &expected).await? {
      Ok(fetched) => fetched,
      Err(Answer::Unreachable) => return Ok(Err(StartOutcome::Waiting(TeamStartStage::Accepted))),
      Err(Answer::Refused(error)) => return self.refuse(operation, error).map(Err),
    };
    let team = bundle::task_subject(kept.authority.subject).ok_or(TeamWorkError::NotATask)?;
    let start = fetched.bundle.start();
    let acceptance = TeamAcceptance {
      task: TeamTaskRef {
        server: link.server,
        project: link.project,
        task: team,
      },
      definition_revision: start.definition_revision,
      work,
      claim: kept.authority.claim_id,
      operation,
      module: start.module_id,
      branch: start.branch.clone(),
      base_commit: start.base_commit.clone(),
      bundle: sha256,
    };
    let mut db = self.db()?;
    let inputs = bundle::inputs(&db, &fetched.bundle, acceptance, kept.choices)?;
    db.store_team_start_inputs(operation, &inputs, &fetched.files, Timestamp::now())?;
    let stored = db
      .team_start(operation)?
      .ok_or(StoreError::TeamStartNotFound(operation))?;
    Ok(Ok(stored))
  }

  /// Asks the runner to start the run of the kept inputs, then records the run and completes the start's operation.
  async fn start_run(&self, operation: OperationId) -> Result<StartOutcome, TeamWorkError> {
    let run = match self.runner.start_accepted_run(operation).await {
      Ok(run) => run,
      Err(RunRequestError::Rejected(rejection)) => return Ok(StartOutcome::RunnerRefused(rejection)),
      Err(RunRequestError::Request(error)) => return Ok(StartOutcome::RunnerUnavailable(error)),
    };
    let mut db = self.db()?;
    db.record_team_start_run(operation, run, Timestamp::now())?;
    complete_operation(&mut db, operation)?;
    Ok(StartOutcome::Started(run))
  }

  fn refuse(&self, operation: OperationId, error: AuthorityError) -> Result<StartOutcome, TeamWorkError> {
    // A refusal serializes to JSON without fail: it holds no map with non-string keys.
    let text = serde_json::to_string(&error).expect("a refusal serializes");
    self.db()?.refuse_team_start(operation, &text, Timestamp::now())?;
    Ok(StartOutcome::Refused(error))
  }
}

/// The refusal kept with a refused start.
fn refused(record: &TeamStart) -> Result<StartOutcome, TeamWorkError> {
  let text = record
    .refusal
    .as_deref()
    .ok_or(TeamWorkError::Unreadable(record.operation))?;
  let error = serde_json::from_str(text).map_err(|_| TeamWorkError::Unreadable(record.operation))?;
  Ok(StartOutcome::Refused(error))
}

/// Settles the start's operation completed once its run is recorded: its start effect happened.
fn complete_operation(db: &mut ProjectDb, operation: OperationId) -> Result<(), TeamWorkError> {
  let Some(kept) = db.team_operation(operation)? else {
    return Ok(());
  };
  let open = matches!(
    kept.state,
    TeamOperationState::Committed | TeamOperationState::EffectFailed
  );
  if let (true, Some(receipt)) = (open, kept.receipt.as_deref()) {
    db.settle_team_operation(operation, TeamSettlement::Completed { receipt }, Timestamp::now())?;
  }
  Ok(())
}
