//! The shared projects' background work, a task of the application's runtime beside the connection
//! monitor (`docs/architecture/server.md`, "Claims", "Live updates"): every minute, one heartbeat
//! request per server names the claims the person holds in the projects this computer shares there,
//! and what they lost is written to the cache; then each shared project's cache is read again, and
//! the claims of runs that ended are ended ([`super::claim_ends`]). A request to sync now (opening a
//! project, *Try now*, *Check again*, after Share) reads the caches at once without a heartbeat,
//! which keeps to the server's limit of 12 heartbeats a minute.

use std::{
  collections::{BTreeMap, HashMap},
  time::{Duration, Instant},
};

use orchester_api::Heartbeat;
use orchester_core::id::{CloudAccountId, ServerId, TaskId};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use tokio::{
  sync::watch,
  time::{sleep, sleep_until},
};

use super::{ProjectServer, claim_ends, convert};
use crate::{
  background,
  sharing::{Servers, SharingError},
};

/// How often claims are kept fresh and the caches read again.
pub const INTERVAL: Duration = Duration::from_secs(60);
/// The most task ids one heartbeat may name.
const HEARTBEAT_IDS: usize = 256;
/// The most heartbeats a session may send in a minute.
const HEARTBEATS_PER_MINUTE: usize = 12;

/// How long run changes settle before their workflow statuses are sent, so a burst of them sends
/// once.
const RUN_SETTLE: Duration = Duration::from_secs(1);

/// Runs until the window closes, as a task on the app runtime that `Root` starts and aborts when it
/// drops. `runs` changes whenever a runner reports a run change: the workflow statuses runs reached
/// are sent then, between the minute's syncs, and the claims of runs that ended are ended. Git,
/// through `programs`, tells which runs a person merged without Orchester.
pub async fn run(servers: Servers, mut runs: watch::Receiver<u64>, programs: Programs) {
  let mut requests = servers.subscribe_sync();
  let mut last_heartbeat: Option<Instant> = None;
  let mut runs_open = true;
  loop {
    requests.borrow_and_update();
    let beat = last_heartbeat.is_none_or(|at| at.elapsed() >= INTERVAL);
    if beat {
      last_heartbeat = Some(Instant::now());
    }
    servers.sync_shared(beat, &programs).await;
    let next = tokio::time::Instant::now() + INTERVAL;
    loop {
      tokio::select! {
        () = sleep_until(next) => break,
        changed = requests.changed() => if changed.is_err() { return } else { break },
        changed = runs.changed(), if runs_open => {
          if changed.is_err() {
            runs_open = false;
            continue;
          }
          sleep(RUN_SETTLE).await;
          runs.borrow_and_update();
          servers.send_automatic_statuses().await;
          servers.end_claims(&programs).await;
        }
      }
    }
  }
}

impl Servers {
  /// One round over every shared project of the workspace, by server: the heartbeat when `beat`,
  /// then each project's sync and the end of the claims whose runs ended.
  pub async fn sync_shared(&self, beat: bool, programs: &Programs) {
    let servers = self.clone();
    let Ok(projects) = background::run(move || shared_projects(&servers)).await else {
      // The blocking pool is shutting down with the window.
      return;
    };
    let mut by_server: BTreeMap<ServerId, Vec<ProjectServer>> = BTreeMap::new();
    for project in projects {
      by_server.entry(project.link().server).or_default().push(project);
    }
    for projects in by_server.values() {
      if beat {
        self.heartbeat(projects, programs).await;
      }
      for project in projects {
        match project.sync().await {
          Ok(()) => claim_ends::end_claims_of(project, programs).await,
          Err(SharingError::Offline | SharingError::SignedOut) => {}
          Err(error) => tracing::warn!(%error, project = %project.link().project, "cannot read the shared project"),
        }
      }
    }
  }

  /// One heartbeat request (more for more than 256 claims) for the claims the person holds in
  /// `projects`, which share one server, except those whose run ended; a claim the server reports
  /// lost is written to the cache.
  async fn heartbeat(&self, projects: &[ProjectServer], programs: &Programs) {
    let Some(first) = projects.first() else {
      return;
    };
    let (Some(me), Ok(())) = (first.me(), super::reachable(self.connection(first.link().server))) else {
      return;
    };
    let roots: Vec<_> = projects.iter().map(|project| project.repo_root().to_owned()).collect();
    let (asked, programs) = (self.work_asked(), programs.clone());
    let held = background::run(move || held_claims(&roots, me, &asked, &programs)).await;
    let held: HashMap<TaskId, usize> = match held {
      Ok(Ok(held)) => held,
      Ok(Err(error)) => {
        tracing::error!(%error, "cannot read the claims to keep fresh");
        return;
      }
      Err(_) => return,
    };
    let ids: Vec<TaskId> = held.keys().copied().collect();
    if ids.len() > HEARTBEAT_IDS * HEARTBEATS_PER_MINUTE {
      tracing::warn!(claims = ids.len(), "more claims than one minute of heartbeats can name");
    }
    for chunk in ids.chunks(HEARTBEAT_IDS).take(HEARTBEATS_PER_MINUTE) {
      let heartbeat = Heartbeat { tasks: chunk.to_vec() };
      let answer = first
        .call(|client, token, _| async move { client.heartbeat(&token, &heartbeat).await })
        .await;
      match answer {
        Ok(answer) => self.lost(projects, &held, answer.lost).await,
        Err(error) => {
          tracing::warn!(%error, "the heartbeat was not answered");
          return;
        }
      }
    }
  }

  /// Writes the claims the server reported lost to the caches of the projects they are in.
  async fn lost(&self, projects: &[ProjectServer], held: &HashMap<TaskId, usize>, lost: Vec<orchester_api::LostClaim>) {
    if lost.is_empty() {
      return;
    }
    let writes: Vec<_> = lost
      .into_iter()
      .filter_map(|lost| {
        let project = held.get(&lost.task)?;
        Some((projects[*project].repo_root().to_owned(), lost))
      })
      .collect();
    let written = background::run(move || -> Result<(), SharingError> {
      for (root, lost) in writes {
        let claim = lost.claim.as_ref().map(convert::claim);
        ProjectDb::open(&root)?.cache_claim(lost.task, claim.as_ref())?;
      }
      Ok(())
    })
    .await;
    if let Ok(Err(error)) = written {
      tracing::error!(%error, "cannot record the claims a heartbeat reported lost");
    }
    self.shared_changed();
  }
}

/// The shared projects on this computer whose folder can be read, whichever account's: each keeps in step with its
/// server while another account is signed in, through its own account's servers only, so a checkout another account
/// linked and this one opened later reaches no server of that other account's.
pub(super) fn shared_projects(servers: &Servers) -> Vec<ProjectServer> {
  let owners = match servers.inner.workspace.server_owners() {
    Ok(owners) => owners,
    Err(error) => {
      tracing::error!(%error, "cannot read whose the servers are");
      return Vec::new();
    }
  };
  owners
    .iter()
    .flat_map(|owner| shared_projects_of(servers, owner.as_ref()))
    .collect()
}

/// [`shared_projects`] of `owner`.
fn shared_projects_of(servers: &Servers, owner: Option<&CloudAccountId>) -> Vec<ProjectServer> {
  let read = servers
    .inner
    .workspace
    .projects_of(owner)
    .map_err(SharingError::from)
    .and_then(|projects| Ok((projects, servers.entries_of(owner)?)));
  let (projects, entries) = match read {
    Ok(read) => read,
    Err(error) => {
      tracing::error!(%error, "cannot read an account's projects and servers");
      return Vec::new();
    }
  };
  projects
    .into_iter()
    .filter(|project| project.repo_root.is_dir())
    .filter_map(
      |project| match ProjectServer::in_background(servers, &entries, &project.repo_root) {
        Ok(found) => found,
        Err(error) => {
          // By id: the project may be another account's, whose folders the log does not name.
          tracing::debug!(%error, project = %project.id, "a project's sharing cannot be read");
          None
        }
      },
    )
    .collect()
}

/// The tasks whose claim `me` holds in the projects at `roots` and that the claim's run did not end
/// ([`claim_ends::HeldClaims`]), with the index of their project: a claim that ended is not kept
/// fresh, also while the server has not taken its end.
fn held_claims(
  roots: &[std::path::PathBuf],
  me: orchester_core::id::UserId,
  asked: &HashMap<TaskId, orchester_core::time::Timestamp>,
  programs: &Programs,
) -> Result<HashMap<TaskId, usize>, orchester_store::StoreError> {
  let mut held = HashMap::new();
  for (index, root) in roots.iter().enumerate() {
    for task in claim_ends::HeldClaims::read(root, me, asked, programs)?.kept {
      held.insert(task, index);
    }
  }
  Ok(held)
}
