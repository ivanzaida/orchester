//! *Merge and deliver* (the person's decision of 2026-11-13): once a team task's run merged into its module's branch,
//! the module's delivery to its base is asked for in the same press, as the module's page asks for it
//! ([`super::deliver`]). The module's claim is taken here, which answers the one this computer holds already; the
//! delivery is requested, which the workspace's Automatic merge authorizes on its own; and when it would wait for an
//! Admin and a person pressed the action, that person's authorization is tried and the delivery asked again on it,
//! which takes the merge. One who is no Admin is refused the authorization, and the delivery waits. What it came to
//! is an operation in the status bar ([`after_merge`]); the module's page shows where the delivery stands.

use std::sync::Arc;

use orchester_api::v2::Subject;
use orchester_core::id::{ClaimId, ModuleId, TaskId};
use orchester_exec::programs::Programs;
use orchester_store::DeliveryMerge;
use orchester_workspace::{
  actions::{Action, ClaimAction},
  activity::Activity,
  background,
  team_work::{
    DeliveryStatus, TeamDeliveryState,
    after_merge::{self, NotDelivered},
  },
  workspace::OpenProject,
};

use super::{
  TeamSource,
  act::{Acted, Acting},
  deliver::{self, Asked},
};

/// The delivery *Merge and deliver* asks for once its merge landed.
#[derive(Clone)]
pub struct Next {
  pub acting: Acting,
  pub source: TeamSource,
  pub programs: Programs,
  pub activity: Activity,
  pub open: OpenProject,
  /// The task whose page asked.
  pub task: TaskId,
  pub module: ModuleId,
  /// The module's name, as the status bar says it.
  pub name: Arc<str>,
  /// The module's branch, which the run merged into.
  pub branch: String,
  /// A person pressed the action: one who is an Admin authorizes the delivery that would wait for one. An MCP
  /// client's press never authorizes, as no MCP tool does.
  pub person: bool,
}

/// Where the delivery stands once it was asked for.
enum Came {
  /// Delivered, or merged here with its report on the way.
  Delivered,
  /// It waits for an Admin.
  Waits,
}

impl Next {
  /// Asks for the delivery and tells the status bar what it came to. Blocks on the server and on git: called on the
  /// merge's own thread, after the merge.
  pub fn deliver(self) {
    let running = self
      .activity
      .start(after_merge::operation(&self.open, self.task, &self.name));
    let runtime = self.acting.runtime.clone();
    match runtime.block_on(self.ask()) {
      Ok(Came::Delivered) => running.finish(),
      Ok(Came::Waits) => running.finish_as(after_merge::waits(&self.name)),
      Err(why) => running.fail(why.message()),
    }
  }

  async fn ask(&self) -> Result<Came, NotDelivered> {
    let subject = Subject::Module { id: self.module };
    let action = Action::Claim(ClaimAction::Acquire { subject });
    let claim = match self.acting.act(&self.source, action).await {
      (Acted::Done, Some(claim)) => claim,
      (acted, _) => return Err(not_acted(&acted)),
    };
    let target = (self.module, claim, self.branch.clone());
    let asked = deliver::request(&self.acting, &self.source, self.programs.clone(), target, None).await;
    let newest = self.newest(asked).await?;
    if newest.status != DeliveryStatus::AwaitingAuthorization || !self.person {
      return came(&newest);
    }
    self.authorized(claim, newest).await
  }

  /// The person's authorization of `waiting`, and the delivery asked again on it; left waiting when the server does
  /// not let this person authorize it.
  async fn authorized(&self, claim: ClaimId, waiting: TeamDeliveryState) -> Result<Came, NotDelivered> {
    let commits = (waiting.source_commit.clone(), waiting.base_commit.clone());
    let target = (self.module, claim, waiting.branch.clone());
    let Ok(authorization) = deliver::authorize(&self.acting, &self.source, target, commits).await else {
      return Ok(Came::Waits);
    };
    let tips = (
      waiting.branch,
      waiting.source_commit,
      waiting.base_branch,
      waiting.base_commit,
    );
    let target = (self.module, claim, tips);
    let asked = deliver::request_tips(
      &self.acting,
      &self.source,
      self.programs.clone(),
      target,
      Some(authorization),
    )
    .await;
    came(&self.newest(asked).await?)
  }

  /// The module's newest delivery recorded here once `asked` went to the server; why nothing was asked otherwise.
  async fn newest(&self, asked: Asked) -> Result<TeamDeliveryState, NotDelivered> {
    match asked {
      Asked::Recorded => {}
      Asked::WorkRunning => return Err(NotDelivered::WorkRunning),
      Asked::NoBase => return Err(NotDelivered::NoBase),
      Asked::BranchNotFound(branch) => return Err(NotDelivered::BranchNotFound(branch)),
      Asked::Acted(acted) => return Err(not_acted(&acted)),
      Asked::Failed => return Err(NotDelivered::Failed),
    }
    let root = self.source.root().to_owned();
    let (handles, module) = ((self.acting.servers.clone(), self.programs.clone()), self.module);
    let read = background::run(move || deliver::deliveries(root, handles, module)).await;
    read.ok().and_then(|mut all| all.pop()).ok_or(NotDelivered::Failed)
  }
}

/// Where a delivery that was asked for stands.
fn came(delivery: &TeamDeliveryState) -> Result<Came, NotDelivered> {
  match &delivery.status {
    DeliveryStatus::Delivered => Ok(Came::Delivered),
    // Authorized: merged here with its report on the way, or its merge still waits for the checkout.
    DeliveryStatus::Authorized { .. } | DeliveryStatus::AuthorizedAutomatically => match &delivery.merge {
      Some(DeliveryMerge::Merged { .. }) => Ok(Came::Delivered),
      Some(DeliveryMerge::Failed { .. }) => Err(NotDelivered::NotMerged),
      None => Err(NotDelivered::MergeWaits),
    },
    DeliveryStatus::AwaitingAuthorization => Ok(Came::Waits),
    DeliveryStatus::Requested => Err(NotDelivered::Unavailable),
    DeliveryStatus::Failed => Err(NotDelivered::NotMerged),
    DeliveryStatus::Refused(error) => Err(NotDelivered::Refused(error.code())),
  }
}

/// Why a team action that was not done leaves the module undelivered.
fn not_acted(acted: &Acted) -> NotDelivered {
  match acted {
    Acted::Refused(code) => NotDelivered::Refused(*code),
    Acted::SignedOut => NotDelivered::SignedOut,
    Acted::Unavailable | Acted::NeedsVersion(_) => NotDelivered::Unavailable,
    Acted::Done | Acted::Changed(_) | Acted::PipelineRefused(_) | Acted::Lifecycle(_) | Acted::Failed => {
      NotDelivered::Failed
    }
  }
}
