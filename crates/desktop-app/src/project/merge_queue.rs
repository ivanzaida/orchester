//! Moves an open project's merge queue on (ORC-19, `orchester_workspace::merge_queue::advance`) for as long as the
//! window follows the project: whenever its runs change state or the queue changes (a person approved, moved or
//! resolved a run), and again after a while when a blocked delivery waits for the person's checkout to be clean.
//! The Integrate step's return goes to the project's runner from here. Mounted with the project's feed, so delivery
//! goes on while the project's tabs are inactive. Renders nothing.

use std::{path::Path, time::Duration};

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  merge_queue::{self, Advance},
  runner::Runners,
  tasks::run::control_message,
  workspace::OpenProject,
};

use crate::services::Services;

/// How often a blocked delivery looks at the person's checkout again.
const CHECKOUT_POLL: Duration = Duration::from_secs(10);
/// How long the queue waits after it could not move on before it tries again.
const AFTER_FAILURE: Duration = Duration::from_secs(30);

/// Follows `open`'s merge queue. Renders nothing.
pub(super) fn follow(ctx: &mut Ctx, open: &OpenProject) {
  let services = Services::of(ctx);
  let deps = (open.clone(), services.runners.clone(), services.programs.clone());
  ctx.stream(deps, |(open, runners, programs), emitter: StreamEmitter<(), ()>| async move {
    drive(open, runners, programs).await;
    // The stream lives while its emitter does: lurq stops a stream whose emitter is gone.
    drop(emitter);
  });
}

async fn drive(open: OpenProject, runners: Runners, programs: Programs) {
  let root = open.repo_root.clone();
  let mut states = runners.states_of(&root);
  // Without a runner the runs' states do not change; the queue still moves on when a person changes it.
  let mut states_open = true;
  let mut queue = merge_queue::changes(&root);
  loop {
    let wait = match advance(&root, &programs).await {
      Ok(Advance::Changed) => continue,
      Ok(Advance::Integrate { run, command }) => {
        let sent = runners
          .control_run_in(&open, command)
          .await
          .map_err(|error| control_message(&error));
        let root = root.clone();
        if let Err(error) = background::run(move || merge_queue::integrate_sent(&root, run, sent)).await.and_then(|told| told) {
          tracing::warn!(key = error.key, "the merge queue could not record its Integrate step");
        }
        continue;
      }
      Ok(Advance::Idle { poll: true }) => Some(CHECKOUT_POLL),
      Ok(Advance::Idle { poll: false }) => None,
      Err(()) => Some(AFTER_FAILURE),
    };
    states.borrow_and_update();
    queue.borrow_and_update();
    let pause = async {
      match wait {
        Some(wait) => tokio::time::sleep(wait).await,
        None => std::future::pending().await,
      }
    };
    let runs = async {
      if states_open {
        states.changed().await.is_ok()
      } else {
        std::future::pending().await
      }
    };
    tokio::select! {
      changed = runs => states_open = changed,
      changed = queue.changed() => if changed.is_err() { return },
      () = pause => {}
    }
  }
}

async fn advance(root: &Path, programs: &Programs) -> Result<Advance, ()> {
  let (root, programs) = (root.to_path_buf(), programs.clone());
  match background::run(move || merge_queue::advance(&root, &programs)).await {
    Ok(Ok(advance)) => Ok(advance),
    Ok(Err(error)) | Err(error) => {
      tracing::warn!(key = error.key, "the merge queue could not move on; it tries again later");
      Err(())
    }
  }
}

/// The count of changes of the queue of the project at `root`, followed while the caller renders; reading it
/// renders the caller again whenever the queue changes.
pub(crate) fn changes(ctx: &mut Ctx, root: &Path) -> u64 {
  let changes = ctx.stream(root.to_path_buf(), |root, emitter: StreamEmitter<u64, ()>| async move {
    let mut changes = merge_queue::changes(&root);
    loop {
      let count = *changes.borrow_and_update();
      if !emitter.emit(count) || changes.changed().await.is_err() {
        return;
      }
    }
  });
  changes.state().get().data.unwrap_or(0)
}
