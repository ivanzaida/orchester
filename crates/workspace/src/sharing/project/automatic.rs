//! Sending the workflow statuses this computer's runs moved shared tasks to (`docs/architecture/
//! server.md`, "Claims"): each goes as Orchester's change, on the revision read, only while the
//! server still has the status the run changed from and the person still holds the task. A status
//! the server changed meanwhile stays the server's; one whose task the person no longer holds is
//! not sent; either says so on the task. One that cannot be sent now (offline, signed out) waits.
//! Statuses are sent as soon as a run changes, then again every minute while one waits
//! ([`super::work`]), and with every full read (*Check again*). A merged run's Done also goes right
//! before its claim is completed ([`super::claim_ends`]), whichever of the two comes first.

use orchester_api::TaskChanges;
use orchester_core::{
  id::TaskId,
  task::{TaskUpdate, organization::ChangeActor},
};
use orchester_store::{AutomaticStatus, NotSent, ProjectDb};

use super::{ProjectServer, store::RemoteTaskStore};
use crate::{
  background,
  sharing::{Servers, SharingError},
};

/// How many times a status is tried on a task that keeps changing before it waits for later.
const ATTEMPTS: usize = 3;

impl Servers {
  /// Sends the waiting statuses of every shared project of the workspace, e.g. right after a run
  /// changed or in the minute's round; the projects whose server cannot be asked now keep theirs. The
  /// screens hear of it only when a status waited.
  pub async fn send_automatic_statuses(&self) {
    let servers = self.clone();
    let sent = background::run(move || {
      let mut waited = false;
      for project in super::work::shared_projects(&servers) {
        if project.writable().is_err() {
          continue;
        }
        let shared = project.link().project;
        let result = ProjectDb::open(project.repo_root())
          .map_err(SharingError::from)
          .and_then(|db| RemoteTaskStore::new(db, project))
          .and_then(|mut store| send(&mut store));
        match result {
          Ok(sent) => waited |= sent,
          Err(error) => {
            waited = true;
            // By the server project's id: the project may be another account's, whose folders the log does not name.
            tracing::warn!(%error, project = %shared, "cannot send the runs' workflow statuses");
          }
        }
      }
      waited
    })
    .await;
    if matches!(sent, Ok(true)) {
      self.shared_changed();
    }
  }
}

impl ProjectServer {
  /// Sends the waiting status of `task` alone, as [`send`] does, while the person still holds its claim; whether it
  /// still waits. The claim of a merged run is completed only after this ([`super::claim_ends`]): once it is
  /// completed, the run's Done is no longer the person's to send, and the task would keep the server's In review
  /// for good (ORC-545). Sending first, rather than taking the status from whoever held the claim when the run
  /// ended, keeps the rule that the server's status, changed by a person meanwhile, wins.
  pub(super) async fn send_automatic_status(&self, task: TaskId) -> Result<bool, SharingError> {
    let server = self.clone();
    let (waited, waits) = background::run(move || {
      let mut store = RemoteTaskStore::new(ProjectDb::open(server.repo_root())?, server)?;
      let waited = send_of(&mut store, task)?;
      let waits = store
        .db
        .automatic_statuses()?
        .iter()
        .any(|pending| pending.task == task);
      Ok::<_, SharingError>((waited, waits))
    })
    .await??;
    if waited {
      self.servers.shared_changed();
    }
    Ok(waits)
  }
}

/// Sends each waiting status of `store`'s project; whether any waited. The tasks and their claims are
/// read from the server first, so a claim someone took over a moment ago (which raises no revision) is
/// known.
pub(super) fn send(store: &mut RemoteTaskStore) -> Result<bool, SharingError> {
  let waiting = store.db.automatic_statuses()?;
  if waiting.is_empty() {
    return Ok(false);
  }
  store.server.writable()?;
  let tasks = store.block_on(
    store
      .server
      .call(|client, token, project| async move { client.tasks(&token, project).await }),
  )?;
  store.cache(&tasks.tasks, None)?;
  send_each(store, waiting)?;
  Ok(true)
}

/// [`send`] for the waiting status of `task` alone, which is read from the server first.
fn send_of(store: &mut RemoteTaskStore, task: TaskId) -> Result<bool, SharingError> {
  let waiting: Vec<_> = store
    .db
    .automatic_statuses()?
    .into_iter()
    .filter(|pending| pending.task == task)
    .collect();
  if waiting.is_empty() {
    return Ok(false);
  }
  store.server.writable()?;
  let current = store.block_on(
    store
      .server
      .call(|client, token, project| async move { client.task(&token, project, task).await }),
  )?;
  store.cache(std::slice::from_ref(&current), None)?;
  send_each(store, waiting)?;
  Ok(true)
}

fn send_each(store: &mut RemoteTaskStore, waiting: Vec<AutomaticStatus>) -> Result<(), SharingError> {
  for pending in waiting {
    match send_one(store, &pending) {
      Ok(()) => {}
      Err(error @ (SharingError::Offline | SharingError::SignedOut)) => return Err(error),
      // The server answered that it does not take it: trying again changes nothing, so it is not
      // sent, and the task says so.
      Err(SharingError::Client(error)) if error.code().is_some() => {
        tracing::warn!(%error, task = %pending.task, "the server did not take a run's workflow status");
        store.db.record_not_sent(pending.task, pending.at, NotSent::Refused)?;
      }
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
