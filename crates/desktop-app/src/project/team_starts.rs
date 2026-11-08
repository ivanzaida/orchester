//! Carries on the team starts an open project left unsettled (ORC-885, `orchester_workspace::team_work`): a start waits
//! at its boundary while the team server or the runner cannot take it, and goes on from it by itself. Without this a
//! start waits for the person's next Run; the page says "starts once they answer". Mounted with the project's feed, so
//! it goes on while the project's tabs are inactive, and asks again every [`RETRY`] for as long as the project is a
//! team project: a Run that ends waiting later is carried on too, not only the starts found when the project opened.
//! While nothing is open each ask is one read of the project's database. A local project asks nothing. Renders nothing.

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

/// How long the project is left before its open starts are looked at again.
#[cfg(not(feature = "test-support"))]
const RETRY: Duration = Duration::from_secs(60);
/// Short in the QA windows, so a test sees a start carried on without waiting a minute.
#[cfg(feature = "test-support")]
const RETRY: Duration = Duration::from_secs(2);

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
      let mut said = None;
      loop {
        match carried_on(&open.repo_root, (&servers, &runners), &vault, &name).await {
          Ok(OpenStarts::Local) => break,
          Ok(_) => said = None,
          // Said once while it lasts, not every round.
          Err(error) if said.as_ref() != Some(&error) => {
            tracing::warn!(%error, "the project's open team starts could not be carried on");
            said = Some(error);
          }
          Err(_) => {}
        }
        tokio::time::sleep(RETRY).await;
      }
      // The stream lives while its emitter does: lurq stops a stream whose emitter is gone.
      drop(emitter);
    },
  );
}

/// Carries the starts on once, on a worker: the futures hold the project database across their awaits. What is left of
/// them, or why they could not be looked at.
async fn carried_on(
  root: &std::path::Path,
  (servers, runners): (&Servers, &Runners),
  vault: &Vault,
  name: &str,
) -> Result<OpenStarts, String> {
  let (root, name): (PathBuf, String) = (root.to_owned(), name.to_owned());
  let (servers, runners, vault) = (servers.clone(), runners.clone(), vault.clone());
  // The stream runs on the app's runtime, which the worker blocks on.
  let runtime = Handle::current();
  let carried = background::run(move || {
    runtime.block_on(async move { recover_open_starts((&servers, runners), (&root, &name), vault.get()).await })
  })
  .await;
  match carried {
    Ok(carried) => carried.map_err(|error| error.to_string()),
    Err(error) => Err(format!(
      "the worker that carries on the project's open team starts ended without an answer: {error:?}"
    )),
  }
}
