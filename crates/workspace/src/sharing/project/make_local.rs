//! *Make local* (`docs/architecture/server.md`, "Migration"; design 10.14 G, H, `woioY`): the person's
//! claims are released first when the server can be reached, and otherwise go stale on their own;
//! then every cached task becomes a local task of the project with its id, number and organization
//! as last synced. The membership stays: joining again later works. The server project is unchanged
//! for everyone else. The tasks kept *Not shared* when the project joined return to the list with
//! numbers after the highest (10.26 C); they are returned with their keys before and after. It works without the
//! server too: when this computer signed out of it, or it is no longer in the list, nothing is released.

use std::path::Path;

use orchester_core::{project::TaskKey, store::TaskStore, time::Timestamp};
use orchester_store::{ProjectDb, ReturnedTask, StoreError};

use super::ProjectServer;
use crate::{
  background,
  sharing::{Servers, SharingError},
};

/// What making the project local would do, as its confirmation says it (`woioY`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakeLocalPlan {
  /// The shared tasks, which stay as local tasks with their numbers.
  pub tasks: usize,
  /// The tasks kept *Not shared* at the join, which return with numbers after the highest.
  pub not_shared: usize,
  /// The tasks the person holds the claim of.
  pub claims: Vec<TaskKey>,
  /// Whether the server takes the claims' release now; otherwise they go stale on their own.
  pub releases: bool,
  /// When the tasks last matched the server.
  pub synced_at: Timestamp,
  /// Whether the person is still a member; making the project local keeps the membership.
  pub member: bool,
}

/// The project made local: its shared tasks, and the *Not shared* tasks that returned with their keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MadeLocal {
  pub tasks: usize,
  pub returned: Vec<ReturnedTask>,
}

impl ProjectServer {
  pub async fn make_local(&self) -> Result<Vec<ReturnedTask>, SharingError> {
    if self.writable().is_ok()
      && let Some(me) = self.me()
    {
      let root = self.repo_root.clone();
      let held = background::run(move || ProjectDb::open(&root)?.claims_held_by(me)).await??;
      for task in held {
        let released = self
          .call(|client, token, project| async move { client.release_claim(&token, project, task).await })
          .await;
        if let Err(error) = released {
          // The claim goes stale on its own after 15 minutes, as when the server cannot be reached.
          tracing::warn!(%error, %task, "a claim was not released on Make local");
        }
      }
    }
    let root = self.repo_root.clone();
    let returned = background::run(move || ProjectDb::open(&root)?.make_local()).await??;
    self.servers.shared_changed();
    Ok(returned)
  }
}

impl Servers {
  /// What making the project at `repo_root` local would do. Blocking.
  pub fn make_local_plan(&self, repo_root: &Path) -> Result<MakeLocalPlan, SharingError> {
    let db = ProjectDb::open(repo_root)?;
    let link = db.sharing()?.ok_or(StoreError::NotShared)?;
    let server = match ProjectServer::open(self, repo_root) {
      Ok(server) => server,
      Err(SharingError::ServerMissing) => None,
      Err(error) => return Err(error),
    };
    let mut claims = Vec::new();
    if let Some(me) = server.as_ref().and_then(ProjectServer::me) {
      for task in db.claims_held_by(me)? {
        if let Some(task) = db.task(task)? {
          claims.push(task.key().clone());
        }
      }
    }
    claims.sort_by_key(|key| key.number());
    Ok(MakeLocalPlan {
      tasks: db.tasks()?.len(),
      not_shared: db.not_shared_tasks()?.len(),
      releases: server.as_ref().is_some_and(|server| server.writable().is_ok()),
      claims,
      synced_at: link.last_synced_at,
      member: link.removed_at.is_none(),
    })
  }

  /// Makes the project at `repo_root` local, also when its server is no longer in the list.
  pub async fn make_local(&self, repo_root: &Path) -> Result<MadeLocal, SharingError> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let (server, tasks) = background::run(move || -> Result<_, SharingError> {
      let tasks = ProjectDb::open(&root)?.tasks()?.len();
      match ProjectServer::open(&servers, &root) {
        Ok(Some(server)) => Ok((Some(server), tasks)),
        Ok(None) => Err(StoreError::NotShared.into()),
        Err(SharingError::ServerMissing) => Ok((None, tasks)),
        Err(error) => Err(error),
      }
    })
    .await??;
    let returned = match server {
      Some(server) => server.make_local().await?,
      None => {
        let root = repo_root.to_owned();
        let returned = background::run(move || ProjectDb::open(&root)?.make_local()).await??;
        self.shared_changed();
        returned
      }
    };
    Ok(MadeLocal { tasks, returned })
  }
}
