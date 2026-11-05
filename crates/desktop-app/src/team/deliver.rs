//! A team module's delivery to its base from its page (`04e` `GujCo`, `04f · Module delivery by state` `Wd4G1`,
//! `docs/design/team-authority.md` §4), as `merge_run` asks for it
//! (`orchester_workspace::team_work::ModuleDeliveries`): the module's holder on this computer asks, citing an Admin's
//! authorization or the workspace's Automatic merge; this computer merges only what the server's receipt allows, and
//! reports it. An Admin authorizes exactly the commits of a delivery that waits for it. What a delivery stands at is
//! read from this computer's records of it ([`deliveries`]): the server keeps no read of a module's deliveries yet, so
//! another member's request is not seen here.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::{ClaimId, ModuleId, OperationId},
  integration::checked_out_branch,
  run::{BranchName, CommitId},
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  actions::{Action, DeliveryAction, DeliveryStep, Dispatcher, Intent},
  background,
  sharing::Servers,
  tasks::changes::GIT,
  team_work::{self, DeliveryProgress, ModuleDeliveries, TeamDeliveryRequest, TeamDeliveryState},
};

use super::{
  TeamSource,
  act::{Acted, Acting},
};

/// What asking for a delivery came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asked {
  /// It went to the server; its record says where it stands now.
  Recorded,
  /// Team runs of the module still work here: nothing was asked.
  WorkRunning,
  /// The checkout has no branch checked out to deliver onto, or has the module's own.
  NoBase,
  /// The module's branch is not in this checkout.
  BranchNotFound(String),
  /// What the dispatcher made of an Admin's authorization, or that this computer is not signed in.
  Acted(Acted),
  /// It could not be carried out here.
  Failed,
}

impl lurq::app::component::DevtoolsInspectable for Asked {}

/// The deliveries of `module` recorded on this computer for the project at `root`, oldest first; none when they cannot
/// be read, which is logged. Blocks on SQLite.
pub fn deliveries(root: PathBuf, (servers, programs): (Servers, Programs), module: ModuleId) -> Vec<TeamDeliveryState> {
  // Reading the records sends nothing: the dispatcher needs no session for it.
  let dispatcher = Dispatcher::new(root, servers, None);
  match ModuleDeliveries::new(dispatcher, programs).deliveries() {
    Ok(all) => all.into_iter().filter(|delivery| delivery.module == module).collect(),
    Err(error) => {
      tracing::warn!(%error, "a team module's deliveries cannot be read");
      Vec::new()
    }
  }
}

/// A delivery's branch and its tip, and the branch it lands on and that one's tip.
pub type Tips = (BranchName, CommitId, BranchName, CommitId);

/// Asks for the delivery of `module`'s `branch`, as its tips are here now, under this computer's module claim `claim`,
/// citing `authorization`.
pub async fn request(
  acting: &Acting,
  source: &TeamSource,
  programs: Programs,
  (module, claim, branch): (ModuleId, ClaimId, String),
  authorization: Option<OperationId>,
) -> Asked {
  let tips = {
    let (programs, root) = (programs.clone(), source.root().to_owned());
    background::run(move || tips(&programs, &root, &branch)).await
  };
  match tips {
    Ok(Ok(tips)) => request_tips(acting, source, programs, (module, claim, tips), authorization).await,
    Ok(Err(asked)) => asked,
    Err(_) => Asked::Failed,
  }
}

/// Asks for exactly the delivery of `module` from `tips` under `claim`, citing `authorization`: after an Admin's
/// authorization, the commits it authorized, whatever the branches are at here now.
pub async fn request_tips(
  acting: &Acting,
  source: &TeamSource,
  programs: Programs,
  (module, claim, (branch, source_commit, base_branch, base_commit)): (ModuleId, ClaimId, Tips),
  authorization: Option<OperationId>,
) -> Asked {
  let request = TeamDeliveryRequest {
    operation: OperationId::generate(),
    delivery: DeliveryAction {
      module,
      claim,
      branch,
      source_commit,
      base_commit,
      step: DeliveryStep::Request { authorization },
    },
    base_branch,
  };
  carry_out(acting, source, (programs, source.root().to_owned()), request).await
}

/// An Admin's authorization of exactly the delivery of `module`'s `branch` under its claim `claim` from `source_commit`
/// onto `base_commit`; the authorization's operation, which the holder's request cites, once the server took it.
pub async fn authorize(
  acting: &Acting,
  source: &TeamSource,
  (module, claim, branch): (ModuleId, ClaimId, BranchName),
  (source_commit, base_commit): (CommitId, CommitId),
) -> Result<OperationId, Acted> {
  let intent = Intent::new(Action::Delivery(DeliveryAction {
    module,
    claim,
    branch,
    source_commit,
    base_commit,
    step: DeliveryStep::Authorize,
  }));
  let operation = intent.operation();
  match acting.act_intent(source, intent).await {
    (Acted::Done, _) => Ok(operation),
    (acted, _) => Err(acted),
  }
}

/// The module's `branch` and the branch checked out at `root`, each with its tip.
fn tips(programs: &Programs, root: &Path, branch: &str) -> Result<Tips, Asked> {
  let runner = programs.runner();
  let source_commit =
    team_work::branch_tip(runner, root, branch).map_err(|_| Asked::BranchNotFound(branch.to_owned()))?;
  let base_branch = checked_out_branch(&runner, Path::new(GIT), root)
    .map_err(|error| {
      tracing::warn!(%error, "cannot read the branch a team delivery lands on");
      Asked::NoBase
    })?
    .ok_or(Asked::NoBase)?;
  if base_branch.as_str() == branch {
    return Err(Asked::NoBase);
  }
  let base_commit = team_work::branch_tip(runner, root, base_branch.as_str()).map_err(|_| Asked::NoBase)?;
  let branch = BranchName::new(branch).map_err(|_| Asked::BranchNotFound(branch.to_owned()))?;
  Ok((branch, source_commit, base_branch, base_commit))
}

/// The request through the module deliveries, on a worker: their futures hold the project database across awaits.
async fn carry_out(
  acting: &Acting,
  source: &TeamSource,
  (programs, root): (Programs, PathBuf),
  request: TeamDeliveryRequest,
) -> Asked {
  let (servers, key, runtime) = (acting.servers.clone(), source.key(), acting.runtime.clone());
  let progress = background::run(move || {
    runtime.block_on(async move {
      let session = servers.team_session(key.server, key.project).await.ok()?;
      let dispatcher = Dispatcher::new(root, servers, Some(session));
      let deliveries = ModuleDeliveries::new(dispatcher, programs);
      Some(deliveries.request(request).await)
    })
  })
  .await;
  match progress {
    Ok(Some(Ok(DeliveryProgress::WorkRunning(_)))) => Asked::WorkRunning,
    Ok(Some(Ok(_))) => Asked::Recorded,
    Ok(None) => Asked::Acted(Acted::SignedOut),
    Ok(Some(Err(error))) => {
      tracing::warn!(%error, "a team delivery could not go on");
      Asked::Failed
    }
    Err(_) => Asked::Failed,
  }
}
