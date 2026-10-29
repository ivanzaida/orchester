//! The shared projects' live event streams (`docs/architecture/server.md`, "Live updates"): one per shared project on
//! this computer while its server takes this computer's token and the person is a member, each following the project's
//! events into its cache ([`stream`], [`apply`]) with the team's configuration ([`config`]) and saying how it stands
//! ([`state`]). Started with the connection monitor and kept in step with the servers and the projects: a project
//! shared, joined or made local, a sign-in or sign-out starts or ends its stream.

mod apply;
mod config;
mod state;
pub mod stream;

use std::{
  collections::{HashMap, HashSet},
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use orchester_api::{
  SharedTask, Versions,
  client::{ClientError, EventStream, StreamEnd},
};
use orchester_core::{
  id::{ProjectId, ResourceId, TaskId},
  resource::VersionNumber,
};
use tokio::task::JoinHandle;

pub(super) use self::state::SyncStates;
pub use self::{
  config::{ConfigServer, RemoteResourceStore, bind, keep_uploaded_configuration, upload_configuration},
  state::SyncState,
  stream::{Clock, Source, Stopped, SystemClock, follow},
};
use super::{Behalf, Connection, Servers, SharingError, project::ProjectServer};
use crate::background;

/// Runs until the window closes, as a task on the app runtime that `Root` starts and aborts when it drops.
pub async fn run(servers: Servers) {
  let mut changes = servers.subscribe();
  let mut shared = servers.subscribe_shared();
  let mut streams: HashMap<PathBuf, Following> = HashMap::new();
  loop {
    changes.borrow_and_update();
    shared.borrow_and_update();
    let listed = servers.clone();
    let Ok(projects) = background::run(move || super::project::work::shared_projects(&listed)).await else {
      // The blocking pool is shutting down with the window.
      return;
    };
    keep(&servers, &mut streams, projects);
    tokio::select! {
      changed = changes.changed() => if changed.is_err() { return },
      changed = shared.changed() => if changed.is_err() { return },
    }
  }
}

/// A project's stream as it runs; dropping it ends the stream.
struct Following {
  project: ProjectId,
  task: JoinHandle<()>,
  /// Whether the stream is still the project's: an aborted one ends only at its next await, and says no state before.
  current: Arc<AtomicBool>,
}

impl Drop for Following {
  fn drop(&mut self) {
    self.current.store(false, Ordering::Release);
    self.task.abort();
  }
}

/// Keeps one stream for each of `projects` that follows its server now, and ends the others; the projects whose person
/// is no longer a member read so, and those without a stream have no state.
fn keep(servers: &Servers, streams: &mut HashMap<PathBuf, Following>, projects: Vec<ProjectServer>) {
  let mut kept = HashMap::new();
  let mut not_members = Vec::new();
  for project in projects {
    let root = project.repo_root().to_owned();
    if project.link().removed_at.is_some() {
      not_members.push(root);
      continue;
    }
    if !follows(servers, &project) {
      continue;
    }
    let id = project.link().project;
    let running = streams
      .remove(&root)
      .filter(|following| following.project == id && !following.task.is_finished());
    let following = running.unwrap_or_else(|| start(servers, project));
    kept.insert(root, following);
  }
  *streams = kept;
  let followed: HashSet<PathBuf> = streams.keys().cloned().collect();
  servers.keep_sync_states(&followed, not_members);
}

/// Whether `project`'s server can stream to this computer: someone is signed in there, and the person is a member.
fn follows(servers: &Servers, project: &ProjectServer) -> bool {
  project.link().removed_at.is_none()
    && project.entry().in_use()
    && servers.connection(project.link().server) != Connection::SignedOut
}

fn start(servers: &Servers, project: ProjectServer) -> Following {
  let id = project.link().project;
  let current = Arc::new(AtomicBool::new(true));
  let live = Live {
    servers: servers.clone(),
    project,
    current: current.clone(),
  };
  let task = tokio::spawn(async move {
    let root = live.project.repo_root();
    let stopped = follow(&live, root, &SystemClock).await;
    tracing::debug!(?stopped, project = %live.project.link().project, "a project's event stream stopped");
    // Signed out, the project has no state: its server's connection says so. Not a member stays.
    if stopped == Stopped::SignedOut {
      live.servers.set_sync_state(root, None, &live.current);
    }
  });
  Following {
    project: id,
    task,
    current,
  }
}

/// A shared project's stream through its server's stored trust and token.
struct Live {
  servers: Servers,
  project: ProjectServer,
  /// The stream's [`Following::current`].
  current: Arc<AtomicBool>,
}

impl Live {
  /// Sends the versions of the team's configuration published here while the server could not be reached, now that
  /// the stream connected again (row 36). One the server does not take waits on its version with why; the stream goes
  /// on either way.
  async fn send_published(&self) {
    let project = self.project.clone();
    let shared = project.link().project;
    let sent = background::run(move || {
      let db = orchester_store::ProjectDb::open(project.repo_root())?;
      RemoteResourceStore::new(db, project)?.send_waiting()
    })
    .await;
    match sent {
      Ok(Ok(())) => {}
      Ok(Err(error)) => tracing::warn!(%error, project = %shared, "versions published here wait to be sent"),
      Err(_) => tracing::debug!(project = %shared, "sending versions published here was interrupted"),
    }
  }
}

impl Source for Live {
  async fn open(&self, cursor: Option<u64>) -> Result<EventStream, StreamEnd> {
    let entry = self.project.entry();
    let server = entry.stored.id;
    let (client, _) = Servers::client(entry).map_err(|error| {
      tracing::warn!(%error, %server, "cannot make the client of a shared project's server");
      StreamEnd::Failed(ClientError::Setup)
    })?;
    let token = match self.servers.token(server).await {
      Ok(token) => token,
      Err(SharingError::SignedOut) => {
        self.servers.mark_signed_out(server);
        return Err(StreamEnd::Unauthorized);
      }
      Err(error) => {
        tracing::error!(%error, %server, "cannot read a shared server's token");
        return Err(StreamEnd::Failed(ClientError::Setup));
      }
    };
    let opened = client.events(&token, self.project.link().project, cursor).await;
    match &opened {
      Ok(_) => {
        self.servers.reached(server);
        self.send_published().await;
      }
      Err(StreamEnd::Unauthorized) => self.servers.refused(server).await,
      Err(_) => {}
    }
    opened
  }

  async fn task(&self, task: TaskId) -> Result<SharedTask, SharingError> {
    let project = self.project.link().project;
    self
      .servers
      .with_token_for(Behalf::Background, self.project.entry(), |client, token| async move {
        client.task(&token, project, task).await
      })
      .await
  }

  async fn versions(&self, resource: ResourceId, after: Option<VersionNumber>) -> Result<Versions, SharingError> {
    let project = self.project.link().project;
    self
      .servers
      .with_token_for(Behalf::Background, self.project.entry(), |client, token| async move {
        client.versions(&token, project, resource, after).await
      })
      .await
  }

  fn changed(&self) {
    self.servers.shared_changed();
  }

  fn state(&self, state: SyncState) {
    self
      .servers
      .set_sync_state(self.project.repo_root(), Some(state), &self.current);
  }
}
