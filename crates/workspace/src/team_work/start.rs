//! A team start, carried forward from the boundary its record stands at: `work.start` through the dispatcher, the
//! receipt's work and bundle reference kept, the bundle read, checked and kept as the run's accepted inputs, then the
//! runner's run. Every step is kept before the next, and each is idempotent, so carrying on after a crash, a lost
//! answer or an outage makes no second work and no second run.

use orchester_api::v2::{
  AuthorityError, AuthorityErrorCode, AuthorityRef, EXECUTION_BUNDLE_SCHEMA, Subject, TeamCommandValue,
};
use orchester_core::{
  id::{ModuleId, OperationId, RunId},
  run::{CommitId, inputs::TeamAcceptance},
  secret::SecretVault,
  sharing::TaskRevision,
  task::identity::TeamTaskRef,
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError, TeamOperationState, TeamSettlement, TeamStart, TeamStartStage};
use serde::{Deserialize, Serialize};

use super::{
  Answer, TeamRunner, TeamWork, TeamWorkError,
  bundle::{self, Expected},
  release::{ClaimEnd, Released},
  turn::StartTurn,
};
use crate::{
  actions::{Accepted, Action, ActionOutcome, Intent, Refusal, TeamOutcome, WorkAction, team_task},
  readiness::{McpPrograms, MissingPart},
  runner::{RequestError, RunRequestError},
};

/// A start of a team task's work from the claim its caller holds: the work action under the intent's operation, and
/// the program paths this computer has set for the team's MCP servers when the caller found the pipeline ready
/// ([`crate::readiness::StartReadiness`]). The run takes the rest of what this computer filled in (the connection of
/// each profile, which servers are on, the secrets' values by name) when the bundle is made into its inputs.
#[derive(Clone, Debug)]
pub struct TeamStartRequest {
  pub operation: OperationId,
  pub work: WorkAction,
  pub programs: McpPrograms,
}

/// Where a start stands after it was carried on.
#[derive(Debug)]
pub enum StartOutcome {
  /// The runner started the run, now or before.
  Started(RunId),
  /// The server cannot be reached, or refused for now: the start waits at this boundary and goes on from it.
  Waiting(TeamStartStage),
  /// The server refused the start for good: it made no work, or the authority the bundle was built for is gone. A
  /// `bundle_missing` refusal released the claim with it, and says what became of it.
  Refused(AuthorityError, Option<ClaimEnd>),
  /// The accepted bundle can never become the run's inputs here; the start goes no further. The claim is let go of
  /// unless it cannot be ([`ClaimEnd`]).
  Failed(StartFailure, ClaimEnd),
  /// The runner refused to run it on this computer; the accepted inputs stay kept for another try.
  RunnerRefused(Rejection),
  /// The runner could not be asked now; the start goes on from the kept inputs.
  RunnerUnavailable(RequestError),
}

/// Why an accepted start can never become a run here, kept with it. Reading the bundle again gives the same bundle,
/// which fails the same way, so the start is settled rather than tried on every recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reason", rename_all = "snake_case")]
pub enum StartFailure {
  /// The server built the bundle in schema `built`, and this computer reads schema `read`: the computer or the server
  /// is older than the other, and one of them has to be updated.
  BundleSchema { read: u32, built: u32 },
  /// The server answered JSON for the bundle that is not an execution bundle of the schema this computer reads.
  BundleUnreadable,
  /// The bundle the server answered, or one of its files, is not the one the receipt names or not built for the start.
  Bundle(String),
  /// The accepted bundle does not make the run's inputs here.
  Inputs(String),
  /// The versions the bundle pins need something this computer lacks (`not_ready_here`); nothing ran. The server made
  /// the work for the start.
  NotReadyHere {
    pipeline: String,
    missing: Vec<MissingPart>,
  },
}

impl StartFailure {
  /// The failure `error` is when it lasts: the same bundle and choices fail the same way every time.
  fn lasting(error: &TeamWorkError) -> Option<Self> {
    match error {
      TeamWorkError::BundleSchema { built } => Some(Self::BundleSchema {
        read: EXECUTION_BUNDLE_SCHEMA,
        built: *built,
      }),
      TeamWorkError::BundleUnreadable => Some(Self::BundleUnreadable),
      TeamWorkError::BundleMismatch { .. } | TeamWorkError::BundleFile(_) => Some(Self::Bundle(error.to_string())),
      TeamWorkError::Inputs(_) => Some(Self::Inputs(error.to_string())),
      TeamWorkError::NotReadyHere { pipeline, missing } => Some(Self::NotReadyHere {
        pipeline: pipeline.clone(),
        missing: missing.clone(),
      }),
      _ => None,
    }
  }
}

/// What a failed start keeps: why, and what became of its claim.
#[derive(Serialize, Deserialize)]
struct KeptFailure {
  failure: StartFailure,
  claim: ClaimEnd,
}

/// Each open start's operation with where it stands after recovery, or why it could not be carried on now.
pub type Recovered = Vec<(OperationId, Result<StartOutcome, TeamWorkError>)>;

/// What a start keeps of its request: the work action, with the server's id of its task, and the program paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct KeptStart {
  pub(super) authority: AuthorityRef,
  base_commit: CommitId,
  read: TaskRevision,
  module: ModuleId,
  module_read: TaskRevision,
  programs: McpPrograms,
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
  /// Starts the work `request` asks for in the project's `turn`, which the caller took before it claimed the task: the
  /// recovery of other starts cannot release that claim before this start is recorded. `turn` is the one of this
  /// dispatcher's project ([`StartTurn::wait`]). The start is recorded here first, then carried on as far as it goes
  /// now. Sending the same request again carries on the recorded start; the same operation with another request is
  /// refused. `vault` is asked whether this computer has the team secrets the pinned MCP servers take, when the bundle
  /// is checked. One start of a project is carried on at a time, and recovery leaves this one alone.
  pub async fn start_in(
    &self,
    _turn: &StartTurn,
    request: TeamStartRequest,
    vault: &dyn SecretVault,
  ) -> Result<StartOutcome, TeamWorkError> {
    let record = self.record(request)?;
    self.carry_on(record, vault).await
  }

  /// Carries on every start not settled yet, oldest first, e.g. after a restart, unless a start of the project is
  /// under way, which carries itself on: then `None`, and the caller asks again later. Each goes on its own: one that
  /// cannot be carried on now holds back none after it.
  pub async fn try_recover(&self, vault: &dyn SecretVault) -> Result<Option<Recovered>, TeamWorkError> {
    let Some(_turn) = StartTurn::try_take(self.dispatcher.servers(), self.dispatcher.root()) else {
      return Ok(None);
    };
    self.recover_alone(vault).await.map(Some)
  }

  async fn recover_alone(&self, vault: &dyn SecretVault) -> Result<Recovered, TeamWorkError> {
    let open = self.db()?.open_team_starts()?;
    let mut outcomes = Vec::with_capacity(open.len());
    for start in open {
      let operation = start.operation;
      outcomes.push((operation, self.carry_on(start, vault).await));
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
      programs: request.programs,
    };
    // The kept start's types serialize to JSON without fail: they hold no map with non-string keys.
    let text = serde_json::to_string(&kept).expect("a team start serializes");
    Ok(db.begin_team_start((request.operation, local), &text, Timestamp::now())?)
  }

  async fn carry_on(&self, mut record: TeamStart, vault: &dyn SecretVault) -> Result<StartOutcome, TeamWorkError> {
    let kept: KeptStart =
      serde_json::from_str(&record.request).map_err(|_| TeamWorkError::Unreadable(record.operation))?;
    loop {
      let step = match record.stage {
        TeamStartStage::Requested => self.request_work(&record, &kept).await?,
        TeamStartStage::Accepted => match self.keep_inputs(&record, kept.clone(), vault).await {
          Ok(step) => step,
          Err(error) => return self.fail((record.operation, kept.authority), error).await,
        },
        TeamStartStage::InputsStored => return self.start_run(record.operation).await,
        TeamStartStage::Started => {
          let run = record.run.ok_or(TeamWorkError::Unreadable(record.operation))?;
          return Ok(StartOutcome::Started(run));
        }
        TeamStartStage::Refused => return refused(&record),
        TeamStartStage::Failed => return failed(&record),
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
      } => return self.refuse((operation, kept.authority), error).await.map(Err),
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
    // The receipt says the schema before the bundle is read: a bundle of another schema never decodes here.
    if bundle.schema_version != EXECUTION_BUNDLE_SCHEMA {
      let built = bundle.schema_version;
      return self
        .fail((operation, kept.authority), TeamWorkError::BundleSchema { built })
        .await
        .map(Err);
    }
    Ok(Ok(accepted))
  }

  /// Reads the bundle the receipt named with every file it pins, checks them, and keeps them as the run's accepted
  /// inputs with the start's record.
  async fn keep_inputs(
    &self,
    record: &TeamStart,
    kept: KeptStart,
    vault: &dyn SecretVault,
  ) -> Result<Step, TeamWorkError> {
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
      Err(Answer::Refused(error)) => return self.refuse((operation, kept.authority), error).await.map(Err),
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
    let inputs = bundle::inputs((&db, vault), &fetched.bundle, acceptance, &kept.programs)?;
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
      Err(RunRequestError::Rejected(rejection)) => {
        // The task's page tells it too, also when no window asked: the start is asked again every round.
        // A rejection serializes to JSON without fail: it is a tagged enum of plain data.
        let text = serde_json::to_string(&rejection).expect("a runner rejection serializes");
        self
          .db()?
          .keep_team_start_runner_refusal(operation, Some(&text), Timestamp::now())?;
        return Ok(StartOutcome::RunnerRefused(rejection));
      }
      Err(RunRequestError::Request(error)) => {
        self
          .db()?
          .keep_team_start_runner_refusal(operation, None, Timestamp::now())?;
        return Ok(StartOutcome::RunnerUnavailable(error));
      }
    };
    let mut db = self.db()?;
    db.record_team_start_run(operation, run, Timestamp::now())?;
    complete_operation(&mut db, operation)?;
    Ok(StartOutcome::Started(run))
  }

  /// Settles the start under `operation` refused as the server did. A refusal that the bundle cannot be built
  /// (`bundle_missing`) leaves the claim held with no work made, so it is released first, and what became of the claim
  /// is kept with the refusal: while the server cannot be reached for that, the start waits at `Requested`, where the
  /// same operation answers the same refusal again.
  async fn refuse(
    &self,
    (operation, authority): (OperationId, AuthorityRef),
    error: AuthorityError,
  ) -> Result<StartOutcome, TeamWorkError> {
    let claim = if error.code() == AuthorityErrorCode::BundleMissing {
      match self.release_claim(operation, authority).await? {
        Released::Waits => return Ok(StartOutcome::Waiting(TeamStartStage::Requested)),
        Released::Ended(claim) => Some(claim),
      }
    } else {
      None
    };
    // A refusal serializes to JSON without fail: it holds no map with non-string keys.
    let text = serde_json::to_string(&KeptRefusal::of(&error, claim)).expect("a refusal serializes");
    self.db()?.refuse_team_start(operation, &text, Timestamp::now())?;
    Ok(StartOutcome::Refused(error, claim))
  }

  /// Settles the accepted start under `operation` failed when `error` lasts, after the claim is released; any other
  /// error stays the caller's, and the start goes on from where it stands next time. While the server cannot be
  /// reached for the release, the start waits at `Accepted` and fails the same way when it goes on.
  async fn fail(
    &self,
    (operation, authority): (OperationId, AuthorityRef),
    error: TeamWorkError,
  ) -> Result<StartOutcome, TeamWorkError> {
    let failure = StartFailure::lasting(&error).ok_or(error)?;
    let Released::Ended(claim) = self.release_claim(operation, authority).await? else {
      return Ok(StartOutcome::Waiting(TeamStartStage::Accepted));
    };
    // A failure is a kind and a message: it serializes to JSON without fail.
    let text = serde_json::to_string(&KeptFailure {
      failure: failure.clone(),
      claim,
    })
    .expect("a start failure serializes");
    self.db()?.fail_team_start(operation, &text, Timestamp::now())?;
    Ok(StartOutcome::Failed(failure, claim))
  }
}

/// What a refused start keeps: the server's refusal, and what became of the claim when the refusal released it. A
/// refusal kept before the claim's end was kept with it is the bare refusal.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum KeptRefusal {
  Ended { refusal: AuthorityError, claim: ClaimEnd },
  Bare(AuthorityError),
}

impl KeptRefusal {
  fn of(error: &AuthorityError, claim: Option<ClaimEnd>) -> Self {
    match claim {
      Some(claim) => Self::Ended {
        refusal: error.clone(),
        claim,
      },
      None => Self::Bare(error.clone()),
    }
  }
}

/// The refusal kept with a refused start.
pub(super) fn refused(record: &TeamStart) -> Result<StartOutcome, TeamWorkError> {
  let text = record
    .refusal
    .as_deref()
    .ok_or(TeamWorkError::Unreadable(record.operation))?;
  let kept: KeptRefusal = serde_json::from_str(text).map_err(|_| TeamWorkError::Unreadable(record.operation))?;
  Ok(match kept {
    KeptRefusal::Ended { refusal, claim } => StartOutcome::Refused(refusal, Some(claim)),
    KeptRefusal::Bare(refusal) => StartOutcome::Refused(refusal, None),
  })
}

/// The failure kept with a failed start.
pub(super) fn failed(record: &TeamStart) -> Result<StartOutcome, TeamWorkError> {
  let text = record
    .failure
    .as_deref()
    .ok_or(TeamWorkError::Unreadable(record.operation))?;
  let kept: KeptFailure = serde_json::from_str(text).map_err(|_| TeamWorkError::Unreadable(record.operation))?;
  Ok(StartOutcome::Failed(kept.failure, kept.claim))
}

/// Settles the operation completed once its local effect happened: the start's run is recorded.
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
