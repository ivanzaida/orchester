//! Sending the workflow statuses this computer's runs moved shared tasks to (`docs/architecture/
//! server.md`, "Claims"): each goes as Orchester's change, on the revision read, only while the
//! server still has the status the run changed from and the person still holds the task. A status
//! the server changed meanwhile stays the server's; one whose task the person no longer holds is
//! not sent; either says so on the task. One that cannot be sent now (offline, signed out) waits.
//! Statuses are sent as soon as a run changes, and with every sync.

use orchester_api::TaskChanges;
use orchester_core::task::{TaskUpdate, organization::ChangeActor};
use orchester_store::{AutomaticStatus, NotSent, ProjectDb};

use super::store::RemoteTaskStore;
use crate::{
  background,
  sharing::{Servers, SharingError},
};

/// How many times a status is tried on a task that keeps changing before it waits for later.
const ATTEMPTS: usize = 3;

impl Servers {
  /// Sends the waiting statuses of every shared project of the workspace, e.g. right after a run
  /// changed; the projects whose server cannot be asked now keep theirs.
  pub async fn send_automatic_statuses(&self) {
    let servers = self.clone();
    let sent = background::run(move || {
      for project in super::work::shared_projects(&servers) {
        if project.writable().is_err() {
          continue;
        }
        let root = project.repo_root().to_owned();
        let result = ProjectDb::open(&root)
          .map_err(SharingError::from)
          .and_then(|db| RemoteTaskStore::new(db, project))
          .and_then(|mut store| send(&mut store));
        if let Err(error) = result {
          tracing::warn!(%error, root = %root.display(), "cannot send the runs' workflow statuses");
        }
      }
    })
    .await;
    if sent.is_ok() {
      self.shared_changed();
    }
  }
}

/// Sends each waiting status of `store`'s project.
pub(super) fn send(store: &mut RemoteTaskStore) -> Result<(), SharingError> {
  for pending in store.db.automatic_statuses()? {
    match send_one(store, &pending) {
      Ok(()) => {}
      Err(error @ (SharingError::Offline | SharingError::SignedOut)) => return Err(error),
      Err(error) => tracing::warn!(%error, task = %pending.task, "a run's workflow status waits"),
    }
  }
  Ok(())
}

fn send_one(store: &mut RemoteTaskStore, pending: &AutomaticStatus) -> Result<(), SharingError> {
  store.server.writable()?;
  let me = store.server.me();
  let holder = store
    .db
    .shared_task_info(pending.task)?
    .and_then(|info| info.claim)
    .map(|claim| claim.holder.id);
  if me.is_none() || holder != me {
    store.db.record_not_sent(pending.task, pending.at, NotSent::NotHolder)?;
    return Ok(());
  }
  for _ in 0..ATTEMPTS {
    let current = store.cached(pending.task)?;
    let now = current.organization().status();
    if now == pending.status {
      store.db.clear_automatic_status(pending.task, pending.at)?;
      return Ok(());
    }
    if now != pending.from {
      store
        .db
        .record_not_sent(pending.task, pending.at, NotSent::Changed { current: now })?;
      return Ok(());
    }
    let changes = TaskChanges {
      workflow: Some(pending.status),
      actor: Some(ChangeActor::Orchester),
      ..TaskChanges::default()
    };
    // On the revision read: a change on the server meanwhile is a conflict, and the cache then has
    // the server's task to compare again.
    if let TaskUpdate::Saved(_) = store.patch(pending.task, current.revision(), changes)? {
      store.db.clear_automatic_status(pending.task, pending.at)?;
      return Ok(());
    }
  }
  Err(SharingError::KeptChanging)
}
