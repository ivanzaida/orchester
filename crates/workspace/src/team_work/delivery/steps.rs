//! A delivery carried forward from the boundary its record stands at: `delivery.request` through the dispatcher, the
//! receipt's delivery kept once its effects allow the merge, the merge marked begun, made or found made, its outcome
//! kept with the report made of it, and the report sent until the server answers it.

use orchester_api::v2::{
  AuthorityError, AuthorityRef, CommandRequest, DeliveryOutcome, ExpectedRevisions, LocalEffect, ModuleDeliveryRead,
  Subject, TeamCommand, TeamCommandValue,
};
use orchester_core::{
  id::OperationId,
  integration::{GitError, MergeError, ModuleDelivery},
  revision::Revision,
  run::CommitId,
  time::Timestamp,
};
use orchester_store::{DeliveryMerge, DeliveryReport, TeamDelivery, TeamDeliveryStage};

use super::{DeliveryProgress, KeptDelivery, ModuleDeliveries};
use crate::{
  actions::{Accepted, Action, ActionOutcome, Intent, Refusal, TeamOutcome},
  tasks::module_delivery::{ModuleMergeError, merge_module_in, module_merged_in},
  team_work::{Answer, TeamWorkError, answer, start::complete_operation},
};

/// The record a step leaves the delivery at, or the outcome it stops with.
type Step = Result<TeamDelivery, DeliveryProgress>;

impl ModuleDeliveries {
  /// Carries `record` on as far as it goes now; with `report_waits`, an earlier report waits for the server, so this
  /// one is not sent before it.
  pub(super) async fn carry_on(
    &self,
    mut record: TeamDelivery,
    report_waits: bool,
  ) -> Result<DeliveryProgress, TeamWorkError> {
    let operation = record.operation;
    let kept: KeptDelivery =
      serde_json::from_str(&record.request).map_err(|_| TeamWorkError::UnreadableDelivery(operation))?;
    loop {
      let step = match record.stage {
        TeamDeliveryStage::Requested => self.request_delivery(operation, &kept).await?,
        TeamDeliveryStage::Accepted => {
          self.db()?.begin_team_delivery_merge(operation, Timestamp::now())?;
          Ok(self.reload(operation)?)
        }
        TeamDeliveryStage::Merging => self.merge(operation, &kept)?,
        TeamDeliveryStage::Reporting if report_waits => Err(DeliveryProgress::Waiting(TeamDeliveryStage::Reporting)),
        TeamDeliveryStage::Reporting => Err(self.send_report(&record).await?),
        TeamDeliveryStage::Refused => Err(DeliveryProgress::Refused(kept_refusal(&record)?)),
        TeamDeliveryStage::Reported => Err(DeliveryProgress::Reported(kept_delivery(&record)?)),
        TeamDeliveryStage::ReportRefused => Err(DeliveryProgress::ReportRefused(kept_refusal(&record)?)),
      };
      match step {
        Ok(next) => record = next,
        Err(outcome) => return Ok(outcome),
      }
    }
  }

  fn reload(&self, operation: OperationId) -> Result<TeamDelivery, TeamWorkError> {
    Ok(
      self
        .db()?
        .team_delivery(operation)?
        .ok_or(orchester_store::StoreError::TeamDeliveryNotFound(operation))?,
    )
  }

  /// `delivery.request` under the delivery's operation, looked up rather than sent again when it was sent before; its
  /// receipt's delivery kept once its effects allow exactly this merge.
  async fn request_delivery(&self, operation: OperationId, kept: &KeptDelivery) -> Result<Step, TeamWorkError> {
    let intent = Intent::with_operation(operation, Action::Delivery(kept.action()));
    let ActionOutcome::Team(outcome) = self.dispatcher.dispatch(&intent).await? else {
      return Err(TeamWorkError::NotADelivery(operation));
    };
    let receipt = match outcome {
      TeamOutcome::AcceptedPending { receipt, .. } | TeamOutcome::Completed(Accepted::Command(receipt)) => receipt,
      TeamOutcome::Rejected {
        refusal: Refusal::Command(error),
        retryable: false,
        ..
      } => {
        // A refusal serializes to JSON without fail: it holds no map with non-string keys.
        let text = serde_json::to_string(&error).expect("a refusal serializes");
        self.db()?.refuse_team_delivery(operation, &text, Timestamp::now())?;
        return Ok(Err(DeliveryProgress::Refused(error)));
      }
      TeamOutcome::Rejected { .. } | TeamOutcome::AcceptanceUnknown { .. } => {
        return Ok(Err(DeliveryProgress::Waiting(TeamDeliveryStage::Requested)));
      }
      TeamOutcome::Completed(Accepted::Definition(_)) | TeamOutcome::LocalEffectFailed { .. } => {
        return Err(TeamWorkError::NotADelivery(operation));
      }
    };
    let TeamCommandValue::Delivery { delivery, effects } = &receipt.result else {
      return Err(TeamWorkError::NotADelivery(operation));
    };
    if !effects.iter().any(|effect| allows(effect, kept)) {
      return Err(TeamWorkError::NotADelivery(operation));
    }
    let text = serde_json::to_string(delivery).expect("a delivery serializes");
    Ok(Ok(self.db()?.accept_team_delivery(
      operation,
      &text,
      Timestamp::now(),
    )?))
  }

  /// Takes a merge found made, else makes it, and keeps its outcome with the report of it; then the request's
  /// operation is completed, its effect carried out. A busy checkout leaves the merge to try again.
  fn merge(&self, operation: OperationId, kept: &KeptDelivery) -> Result<Step, TeamWorkError> {
    let (head, base) = (revision(&kept.source_commit)?, revision(&kept.base_commit)?);
    let delivery = ModuleDelivery {
      module: kept.module,
      branch: &kept.branch,
      head: &head,
      into: &kept.base_branch,
      base: &base,
    };
    let root = self.dispatcher.root();
    let found = match module_merged_in(root, &self.programs, &delivery) {
      Ok(found) => found,
      Err(GitError::Run(_)) => return Ok(Err(DeliveryProgress::MergeWaiting)),
      Err(error) => return Err(TeamWorkError::DeliveryGit(error)),
    };
    let merge = match found {
      Some(commit) => merged(&commit)?,
      None => match merge_module_in(root, &self.programs, &delivery) {
        Ok(made) => merged(&made.revision)?,
        Err(ModuleMergeError::Busy | ModuleMergeError::Merge(MergeError::Git(GitError::Run(_)))) => {
          return Ok(Err(DeliveryProgress::MergeWaiting));
        }
        Err(ModuleMergeError::Merge(error)) => DeliveryMerge::Failed {
          failure: error.to_string(),
        },
        Err(ModuleMergeError::Store(error)) => return Err(error.into()),
      },
    };
    let report = report(kept, &merge);
    let mut db = self.db()?;
    db.queue_team_delivery_report(operation, &merge, &report, Timestamp::now())?;
    complete_operation(&mut db, operation)?;
    Ok(Ok(self.reload(operation)?))
  }

  /// Sends the delivery's kept report, with the same bytes every time, and keeps the server's answer.
  async fn send_report(&self, record: &TeamDelivery) -> Result<DeliveryProgress, TeamWorkError> {
    let operation = record.operation;
    let report = record
      .report
      .as_ref()
      .ok_or(TeamWorkError::UnreadableDelivery(operation))?;
    let body: CommandRequest<TeamCommand> =
      serde_json::from_str(&report.request).map_err(|_| TeamWorkError::UnreadableDelivery(operation))?;
    let (_, session) = self.dispatcher.team(&self.db()?)?;
    let answered = session.authority().command(&body).await;
    let mut db = self.db()?;
    match answered {
      Ok(receipt) if receipt.operation_id == report.operation => {
        let TeamCommandValue::Delivery { delivery, .. } = receipt.result else {
          return Err(TeamWorkError::NotADelivery(operation));
        };
        let text = serde_json::to_string(&delivery).expect("a delivery serializes");
        db.acknowledge_team_delivery_report(operation, &text, Timestamp::now())?;
        Ok(DeliveryProgress::Reported(*delivery))
      }
      Ok(_) => Err(TeamWorkError::NotADelivery(operation)),
      Err(error) => match answer(error) {
        Answer::Unreachable => Ok(DeliveryProgress::Waiting(TeamDeliveryStage::Reporting)),
        Answer::Refused(error) => {
          let text = serde_json::to_string(&error).expect("a refusal serializes");
          db.refuse_team_delivery_report(operation, &text, Timestamp::now())?;
          Ok(DeliveryProgress::ReportRefused(error))
        }
      },
    }
  }
}

/// Whether `effect` lets this computer merge exactly the delivery `kept` requested.
fn allows(effect: &LocalEffect, kept: &KeptDelivery) -> bool {
  matches!(
    effect,
    LocalEffect::DeliverModule { module_id, branch, source_commit, base_commit }
      if *module_id == kept.module && *branch == kept.branch && *source_commit == kept.source_commit
        && *base_commit == kept.base_commit
  )
}

/// The `delivery.report` of `merge`, made once with its own operation id.
fn report(kept: &KeptDelivery, merge: &DeliveryMerge) -> DeliveryReport {
  let outcome = match merge {
    DeliveryMerge::Merged { commit } => DeliveryOutcome::Delivered {
      merge_commit: commit.clone(),
    },
    DeliveryMerge::Failed { .. } => DeliveryOutcome::Failed {},
  };
  let operation = OperationId::generate();
  let body = CommandRequest {
    operation_id: operation,
    expected: ExpectedRevisions::default(),
    command: TeamCommand::DeliveryReport {
      authority: AuthorityRef {
        subject: Subject::Module { id: kept.module },
        claim_id: kept.claim,
        work_id: None,
      },
      source_commit: kept.source_commit.clone(),
      base_commit: kept.base_commit.clone(),
      outcome,
    },
  };
  DeliveryReport {
    operation,
    request: serde_json::to_string(&body).expect("a command request serializes"),
  }
}

fn merged(commit: &Revision) -> Result<DeliveryMerge, TeamWorkError> {
  let commit = CommitId::new(commit.as_str()).map_err(|_| {
    TeamWorkError::DeliveryGit(GitError::Unrecognized {
      expected: "a full commit id",
      detail: commit.as_str().to_owned(),
    })
  })?;
  Ok(DeliveryMerge::Merged { commit })
}

fn revision(commit: &CommitId) -> Result<Revision, TeamWorkError> {
  Revision::new(commit.as_str()).map_err(|_| {
    TeamWorkError::DeliveryGit(GitError::Unrecognized {
      expected: "a revision",
      detail: commit.as_str().to_owned(),
    })
  })
}

fn kept_refusal(record: &TeamDelivery) -> Result<AuthorityError, TeamWorkError> {
  let text = record
    .refusal
    .as_deref()
    .ok_or(TeamWorkError::UnreadableDelivery(record.operation))?;
  serde_json::from_str(text).map_err(|_| TeamWorkError::UnreadableDelivery(record.operation))
}

fn kept_delivery(record: &TeamDelivery) -> Result<ModuleDeliveryRead, TeamWorkError> {
  let text = record
    .delivery
    .as_deref()
    .ok_or(TeamWorkError::UnreadableDelivery(record.operation))?;
  serde_json::from_str(text).map_err(|_| TeamWorkError::UnreadableDelivery(record.operation))
}
