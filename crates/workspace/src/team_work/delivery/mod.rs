//! A team module's delivery to base (team authority P4c, the server's `docs/module-branches.md`, S7c and S12): the
//! module's holder asks for it, this computer merges only what the server's receipt allows, and reports how the merge
//! went.
//!
//! - **Request.** The delivery is recorded in the project database (`team_deliveries`) with what its caller read here,
//!   including the branch of this repository the base commit was read from, then `delivery.request` goes through the
//!   dispatcher under the delivery's operation. An Admin's `delivery.authorize` (cited by its operation) or the
//!   workspace's automatic merge authorizes it; without either the server keeps it waiting for an Admin and answers it
//!   without an effect, which is kept as waiting (no refusal). An Admin's authorization of exactly that request then
//!   lets the holder's next request, under an operation of its own, take the merge; an Admin may also authorize first.
//!   It is not sent while a team run of the module still works here: what the module branch would deliver is not done
//!   yet.
//! - **Merge.** Only a receipt whose effects carry `deliver_module` for exactly the requested branch and commits lets
//!   this computer merge, by the merge a run takes into its base branch ([`crate::tasks::module_delivery`]), only while
//!   the module branch and the base are still at those commits. The merge is marked begun before git is asked, and a
//!   merge found on the base branch afterwards is taken as made, so a crash between the merge and its record recovers
//!   once and never merges twice.
//! - **Report.** How the merge went is kept with the `delivery.report` made of it, in one transaction, before the
//!   report is sent; it is sent again with the same bytes until the server answers it, oldest delivery first, and a
//!   report that waits for the server holds back the later ones.
//!
//! Each state the server answers is kept for the screens ([`TeamDeliveryState`]). The MCP tool `merge_run` of a team
//! task's run requests its module's delivery (P7b); the screens adopt it in P6.

mod state;
mod steps;

use orchester_api::v2::{AuthorityError, ModuleDeliveryRead};
use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, RunId},
  integration::base::RunBaseSource,
  run::{BranchName, CommitId},
  store::RunStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, TeamDelivery, TeamDeliveryStage};
use serde::{Deserialize, Serialize};

pub use self::state::{DeliveryStatus, TeamDeliveryState};
use super::TeamWorkError;
use crate::actions::{DeliveryAction, DeliveryStep, Dispatcher};

/// The delivery of a team module's branch its holder asks for: the delivery action (a request) under the delivery's
/// operation, and the branch of this repository whose commit `base_commit` is, which the merge lands on.
#[derive(Clone, Debug)]
pub struct TeamDeliveryRequest {
  pub operation: OperationId,
  pub delivery: DeliveryAction,
  pub base_branch: BranchName,
}

/// Where a delivery stands after it was carried on.
#[derive(Debug)]
pub enum DeliveryProgress {
  /// Team runs of the module still work here: nothing was recorded or sent.
  WorkRunning(Vec<RunId>),
  /// The server took the request without an effect, as it keeps it: it waits for an Admin's authorization, with the
  /// automatic merge off. Nothing was merged; the holder's next request, once an Admin authorized it, takes the merge.
  AwaitingAdmin(ModuleDeliveryRead),
  /// The server refused the request, e.g. citing an authorization that is not of this delivery. Nothing was merged.
  Refused(AuthorityError),
  /// The server cannot be reached, or refused for now: the delivery waits at this stage and goes on from it.
  Waiting(TeamDeliveryStage),
  /// Another merge or a local integration holds the checkout, or git cannot run now: the merge waits.
  MergeWaiting,
  /// The server answered the report: the delivery as it keeps it now, delivered or failed.
  Reported(ModuleDeliveryRead),
  /// The server refused the report; the merge stays as it is here.
  ReportRefused(AuthorityError),
}

/// Each open delivery's operation with where it stands after recovery, or why it could not be carried on now.
pub type RecoveredDeliveries = Vec<(OperationId, Result<DeliveryProgress, TeamWorkError>)>;

/// What a delivery keeps of its request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeptDelivery {
  module: ModuleId,
  claim: ClaimId,
  branch: BranchName,
  base_branch: BranchName,
  source_commit: CommitId,
  base_commit: CommitId,
  authorization: Option<OperationId>,
}

impl KeptDelivery {
  fn action(&self) -> DeliveryAction {
    DeliveryAction {
      module: self.module,
      claim: self.claim,
      branch: self.branch.clone(),
      source_commit: self.source_commit.clone(),
      base_commit: self.base_commit.clone(),
      step: DeliveryStep::Request {
        authorization: self.authorization,
      },
    }
  }
}

/// One team project's module deliveries in one session: its dispatcher, signed in to the project's server, and the
/// programs the merge runs.
pub struct ModuleDeliveries {
  dispatcher: Dispatcher,
  programs: Programs,
}

impl ModuleDeliveries {
  pub const fn new(dispatcher: Dispatcher, programs: Programs) -> Self {
    Self { dispatcher, programs }
  }

  fn db(&self) -> Result<ProjectDb, TeamWorkError> {
    Ok(ProjectDb::open(self.dispatcher.root())?)
  }

  /// The dispatcher the deliveries go through, e.g. to take the module's claim a request cites.
  pub const fn dispatcher(&self) -> &Dispatcher {
    &self.dispatcher
  }

  /// The team runs of `module` that still work on this computer: while there are any, a request is not sent
  /// ([`DeliveryProgress::WorkRunning`]).
  pub fn runs_at_work(&self, module: ModuleId) -> Result<Vec<RunId>, TeamWorkError> {
    module_runs_at_work(&self.db()?, module)
  }

  /// Requests the delivery `request` asks for: recorded here first, then carried on as far as it goes now, after the
  /// reports of earlier deliveries that wait for the server are sent. Sending the same request again carries on the
  /// recorded delivery; the same operation with another request is refused.
  pub async fn request(&self, request: TeamDeliveryRequest) -> Result<DeliveryProgress, TeamWorkError> {
    let DeliveryStep::Request { authorization } = request.delivery.step else {
      return Err(TeamWorkError::NotADeliveryRequest);
    };
    let mut db = self.db()?;
    let kept = KeptDelivery {
      module: request.delivery.module,
      claim: request.delivery.claim,
      branch: request.delivery.branch,
      base_branch: request.base_branch,
      source_commit: request.delivery.source_commit,
      base_commit: request.delivery.base_commit,
      authorization,
    };
    if db.team_delivery(request.operation)?.is_none() {
      let running = module_runs_at_work(&db, kept.module)?;
      if !running.is_empty() {
        return Ok(DeliveryProgress::WorkRunning(running));
      }
    }
    // The kept request holds ids, names and commits only: it serializes to JSON without fail.
    let text = serde_json::to_string(&kept).expect("a team delivery serializes");
    let record = db.begin_team_delivery(request.operation, &text, Timestamp::now())?;
    let earlier: Vec<_> = db
      .team_deliveries()?
      .into_iter()
      .take_while(|delivery| delivery.operation != record.operation)
      .filter(|delivery| delivery.stage == TeamDeliveryStage::Reporting)
      .collect();
    let mut reports_wait = false;
    for delivery in earlier {
      let operation = delivery.operation;
      // The earlier delivery's outcome is kept in its record, where its own caller reads it; here it only decides
      // whether this delivery's report waits behind it.
      if let Err(error) = self.carry_on_in_order(delivery, &mut reports_wait).await {
        tracing::warn!(%operation, %error, "the report of an earlier team delivery was not sent");
      }
    }
    self.carry_on_in_order(record, &mut reports_wait).await
  }

  /// Carries on every delivery not settled yet, oldest first, e.g. after a restart. Each goes on its own, but reports
  /// go in order: once one waits for the server, the later ones wait too.
  pub async fn recover(&self) -> Result<RecoveredDeliveries, TeamWorkError> {
    let open: Vec<_> = self
      .db()?
      .team_deliveries()?
      .into_iter()
      .filter(|delivery| !delivery.stage.settled())
      .collect();
    let mut outcomes = Vec::with_capacity(open.len());
    let mut reports_wait = false;
    for delivery in open {
      let operation = delivery.operation;
      outcomes.push((operation, self.carry_on_in_order(delivery, &mut reports_wait).await));
    }
    Ok(outcomes)
  }

  /// Carries `delivery` on, its report held back while `reports_wait`; then `reports_wait` holds the later reports back
  /// too while this one's still waits for the server.
  async fn carry_on_in_order(
    &self,
    delivery: TeamDelivery,
    reports_wait: &mut bool,
  ) -> Result<DeliveryProgress, TeamWorkError> {
    let operation = delivery.operation;
    let outcome = self.carry_on(delivery, *reports_wait).await;
    // A record that cannot be read back may still wait to report: the later reports wait rather than overtake it.
    *reports_wait |= self
      .reload(operation)
      .map_or(true, |record| record.stage == TeamDeliveryStage::Reporting);
    outcome
  }
}

/// The team runs of `module` that still work on this computer: their work would reach the module branch after the
/// delivery read its head.
fn module_runs_at_work(db: &ProjectDb, module: ModuleId) -> Result<Vec<RunId>, TeamWorkError> {
  let mut running = Vec::new();
  for team in db.team_runs()? {
    let run = db
      .run(team.run)?
      .ok_or(orchester_store::StoreError::RunNotFound(team.run))?;
    if run.base_source() == (RunBaseSource::Module { module }) && run.status().has_live_work() {
      running.push(team.run);
    }
  }
  Ok(running)
}
