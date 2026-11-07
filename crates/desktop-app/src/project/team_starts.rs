//! Carries on the team starts an open project left unsettled (ORC-885, `orchester_workspace::team_work`): a start waits
//! at its boundary while the team server or the runner cannot take it, and goes on from it by itself. Without this a
//! start waits for the person's next Run; the page says "starts once they answer". Mounted with the project's feed, so
//! it goes on while the project's tabs are inactive, and asked again every [`RETRY`] while a start still waits for the
//! server or the runner. A local project, and one with no open start, asks nothing. Renders nothing.

use std::{path::PathBuf, time::Duration};

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_workspace::{
  background,
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  team_work::{OpenStarts, recover_open_starts},
  workspace::OpenProject,
};
use tokio::runtime::Handle;

use crate::services::Services;

/// How long a start that waits for the team server or the runner is left before it is carried on again.
const RETRY: Duration = Duration::from_secs(60);

/// Carries on `open`'s unsettled team starts. Renders nothing.
pub(super) fn follow(ctx: &mut Ctx, open: &OpenProject) {
  let services = Services::of(ctx);
  let deps = (
    open.clone(),
    (services.servers.clone(), services.runners.clone()),
    services.vault,
  );
  ctx.stream(
    deps,
    |(open, (servers, runners), vault), emitter: StreamEmitter<(), ()>| async move {
      let name = open.project.name().as_str().to_owned();
      while carried_on(&open.repo_root, (&servers, &runners), &vault, &name).await {
        tokio::time::sleep(RETRY).await;
      }
      // The stream lives while its emitter does: lurq stops a stream whose emitter is gone.
      drop(emitter);
    },
  );
}

/// Carries the starts on once, on a worker: the futures hold the project database across their awaits. Whether one
/// still waits for the team server or the runner, and so is to be carried on again.
async fn carried_on(
  root: &std::path::Path,
  (servers, runners): (&Servers, &Runners),
  vault: &Vault,
  name: &str,
) -> bool {
  let (root, name): (PathBuf, String) = (root.to_owned(), name.to_owned());
  let (servers, runners, vault) = (servers.clone(), runners.clone(), vault.clone());
  // The stream runs on the app's runtime, which the worker blocks on.
  let runtime = Handle::current();
  let carried = background::run(move || {
    runtime.block_on(async move { recover_open_starts((&servers, runners), (&root, &name), vault.get()).await })
  })
  .await;
  match carried {
    Ok(Ok(OpenStarts::Waiting)) => true,
    Ok(Ok(OpenStarts::None | OpenStarts::Settled)) => false,
    Ok(Err(error)) => {
      tracing::warn!(%error, "the project's open team starts could not be carried on");
      false
    }
    Err(_) => false,
  }
}
