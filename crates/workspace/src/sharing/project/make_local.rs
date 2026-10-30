//! *Make local* (`docs/architecture/server.md`, "Migration"; design 10.14 G, H, `woioY`): the person's
//! claims are released first when the server can be reached, and otherwise go stale on their own;
//! then every cached task becomes a local task of the project with its id, number and organization
//! as last synced, and every resource of the team's a local resource with every cached version, so past runs keep
//! what they pinned; the references the versions named on their publishers' computers and this computer's bindings of
//! them go. The membership stays: joining again later works. The server project is unchanged
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
  /// The tasks the person holds the claim of; `None` when this computer no longer knows who the person is there
  /// (signed out of the server, or it is no longer in the list): any claims they hold go stale on their own.
  pub claims: Option<Vec<TaskKey>>,
  /// Whether the server takes the claims' release now; otherwise they go stale on their own.
  pub releases: bool,
  /// When the tasks last matched the server.
  pub synced_at: Timestamp,
  /// Whether the person is still a member; making the project local keeps the membership.
  pub member: bool,
}

/// The project made local: its shared tasks, what became of the person's claims, and the *Not shared* tasks that
/// returned with their keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MadeLocal {
  pub tasks: usize,
  pub claims: ClaimRelease,
  pub returned: Vec<ReturnedTask>,
}

/// What became of the person's claims when the project was made local.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaimRelease {
  /// Every claim the person held was released on the server (none, when they held none).
  Released,
  /// The server did not take the release of `tasks` (it could not be reached, or no longer has the person as a
  /// member): their claims go stale on their own.
  Stale { tasks: Vec<TaskKey> },
  /// This computer no longer knows who the person is there: any claims they hold go stale on their own.
  Unknown,
}

impl ProjectServer {
  pub async fn make_local(&self) -> Result<Vec<ReturnedTask>, SharingError> {
    Ok(self.make_local_releasing().await?.1)
  }

  /// Releases the person's claims where the server takes it, then makes the project local.
  async fn make_local_releasing(&self) -> Result<(ClaimRelease, Vec<ReturnedTask>), SharingError> {
    let released = self.release_claims().await?;
    let root = self.repo_root.clone();
    let returned = background::run(move || ProjectDb::open(&root)?.make_local()).await??;
    self.servers.shared_changed();
    Ok((released, returned))
  }

  async fn release_claims(&self) -> Result<ClaimRelease, SharingError> {
    let Some(me) = self.me() else {
      return Ok(ClaimRelease::Unknown);
    };
    let root = self.repo_root.clone();
    let held = background::run(move || -> Result<_, StoreError> {
      let db = ProjectDb::open(&root)?;
      let mut held = Vec::new();
      for task in db.claims_held_by(me)? {
        if let Some(found) = db.task(task)? {
          held.push((task, found.key().clone()));
        }
      }
      Ok(held)
    })
    .await??;
    let mut stale = Vec::new();
    if let Err(error) = self.writable() {
      // The claims go stale on their own after 15 minutes.
      tracing::warn!(%error, "the claims were not released on Make local");
      stale = held.into_iter().map(|(_, key)| key).collect();
    } else {
      for (task, key) in held {
        let released = self
          .call(|client, token, project| async move { client.release_claim(&token, project, task).await })
          .await;
        if let Err(error) = released {
          // The claim goes stale on its own after 15 minutes, as when the server cannot be reached.
          tracing::warn!(%error, %task, "a claim was not released on Make local");
          stale.push(key);
        }
      }
    }
    if stale.is_empty() {
      return Ok(ClaimRelease::Released);
    }
    stale.sort_by_key(TaskKey::number);
    Ok(ClaimRelease::Stale { tasks: stale })
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
    let claims = match server.as_ref().and_then(ProjectServer::me) {
      Some(me) => {
        let mut claims = Vec::new();
        for task in db.claims_held_by(me)? {
          if let Some(task) = db.task(task)? {
            claims.push(task.key().clone());
          }
        }
        claims.sort_by_key(TaskKey::number);
        Some(claims)
      }
      None => None,
    };
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
    let (claims, returned) = match server {
      Some(server) => server.make_local_releasing().await?,
      None => {
        let root = repo_root.to_owned();
        let returned = background::run(move || ProjectDb::open(&root)?.make_local()).await??;
        self.shared_changed();
        (ClaimRelease::Unknown, returned)
      }
    };
    Ok(MadeLocal {
      tasks,
      claims,
      returned,
    })
  }
}
