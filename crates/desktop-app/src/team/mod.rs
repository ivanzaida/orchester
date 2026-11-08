//! A session's reads of its team projects from their servers (`docs/architecture/team-authority-plan.md` §6, milestone
//! P6): one lurq [`QueryClient`] for the session, shared by its windows, and per linked project the reads of
//! `orchester_workspace::team_reads` with their subscription followed on the runtime ([`TeamSource`]). The lists'
//! queries are in [`queries`], the pages' in [`pages`]; what the subscription makes stale is in [`invalidation`]; how a
//! view finds its project's source is in [`view`]. The screens' actions go through the dispatcher ([`act`]): a task's
//! edit ([`edit`]), a task's work starts through the team server ([`start`]) and a module's delivery is asked for there
//! ([`deliver`]); their lines share one strip ([`strip`]).
//!
//! The client and every source belong to one generation of the session. Another account on a server, signing out of
//! one, or a membership revoked ends the generation: a new client replaces the old one, whose reads no view observes
//! again, and the old sources' reads end with `SessionChanged`, so no late answer of the old account reaches a view. A
//! project whose reads ended for the person (signed out, no longer a member) stays ended, with why, until the accounts
//! change or the person asks again ([`TeamQueries::retry`]).

pub mod act;
pub mod deliver;
pub mod edit;
pub mod invalidation;
pub mod last_start;
pub mod lock;
pub mod outage;
pub mod pages;
pub mod queries;
pub mod reached;
mod source;
pub mod start;
pub mod strip;
pub mod view;

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::{Arc, Weak},
};

use lurq::query::QueryClient;
use orchester_core::id::{ServerId, UserId};
use orchester_workspace::{
  background,
  sharing::{Servers, project::ProjectServer, sync::SystemClock},
  team_reads::{TeamProject, TeamReadError},
};
use parking_lot::Mutex;
use tokio::{runtime::Handle, sync::watch, task::JoinHandle};

use self::reached::ReadsReached;
pub use self::source::{SourceKey, TeamFailure, TeamSource};

/// The session's team reads. Clones share them.
#[derive(Clone)]
pub struct TeamQueries {
  inner: Arc<Inner>,
}

struct Inner {
  servers: Servers,
  runtime: Handle,
  generation: watch::Sender<u64>,
  current: Mutex<Current>,
  /// How each project's reads last ended, across generations: the latest answer stays what is known of its server.
  reached: ReadsReached,
}

/// What belongs to the session's current generation.
struct Current {
  client: QueryClient,
  /// Who is signed in to which server, as this generation found it; `None` before the first look.
  accounts: Option<Vec<(ServerId, Option<UserId>)>>,
  sources: HashMap<PathBuf, Opened>,
  /// The projects whose reads ended for the person, and why.
  ended: HashMap<PathBuf, TeamFailure>,
}

/// A project's source with the tasks that follow its subscription and apply its changes; they end with it.
struct Opened {
  source: TeamSource,
  tasks: [JoinHandle<()>; 2],
}

impl Drop for Opened {
  fn drop(&mut self) {
    for task in &self.tasks {
      task.abort();
    }
  }
}

impl PartialEq for TeamQueries {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl TeamQueries {
  /// The team reads of the session whose servers are `servers`, followed on `runtime`.
  pub fn new(servers: Servers, runtime: Handle) -> Self {
    Self {
      inner: Arc::new(Inner {
        servers,
        runtime,
        generation: watch::channel(0).0,
        current: Mutex::new(Current {
          client: QueryClient::new(),
          accounts: None,
          sources: HashMap::new(),
          ended: HashMap::new(),
        }),
        reached: ReadsReached::new(),
      }),
    }
  }

  /// The current generation's client; views provide it before they query.
  pub fn client(&self) -> QueryClient {
    self.inner.current.lock().client.clone()
  }

  pub fn generation(&self) -> u64 {
    *self.inner.generation.borrow()
  }

  /// Wakes whenever a generation ends.
  pub fn subscribe(&self) -> watch::Receiver<u64> {
    self.inner.generation.subscribe()
  }

  /// How each project's reads last ended with its server.
  pub fn reached(&self) -> &ReadsReached {
    &self.inner.reached
  }

  /// Ends the current generation: a new client, no source, nothing ended. *Try again* after an outage or after a
  /// project's reads ended does so: the sources subscribe anew at once instead of waiting out their backoff, and every
  /// view reads again.
  pub fn renew(&self) {
    self.inner.renew();
  }

  /// The source of the project at `root` in this generation; `None` while the project is local. A project whose reads
  /// ended for the person fails with why.
  pub async fn source(&self, root: &Path) -> Result<Option<TeamSource>, TeamFailure> {
    self.follow_accounts().await;
    let generation = self.generation();
    {
      let current = self.inner.current.lock();
      if let Some(failure) = current.ended.get(root) {
        return Err(failure.clone());
      }
      if let Some(opened) = current.sources.get(root) {
        return Ok(Some(opened.source.clone()));
      }
    }
    let (servers, folder) = (self.inner.servers.clone(), root.to_owned());
    let opened = background::run(move || ProjectServer::open(&servers, &folder))
      .await
      .map_err(|_| TeamFailure::Refused)?;
    let server = match opened {
      Ok(Some(server)) => server,
      Ok(None) => return Ok(None),
      Err(error) => return Err(TeamFailure::from(&error)),
    };
    let session = server
      .team_session(self.subscribe())
      .await
      .map_err(|error| TeamFailure::from(&error))?;
    let key = SourceKey {
      server: server.server(),
      generation,
      project: server.team_project(),
    };
    let project = TeamProject::new(session);
    let source = TeamSource::new(
      key,
      project,
      (root.to_owned(), self.inner.servers.clone(), self.inner.reached.clone()),
    );
    Ok(Some(self.inner.open(root, source, generation)))
  }

  /// Ends the generation when who is signed in to the servers changed since it began.
  async fn follow_accounts(&self) {
    let servers = self.inner.servers.clone();
    let Ok(Ok(entries)) = background::run(move || servers.entries()).await else {
      // The servers cannot be read now: the generation stays, and the next look decides.
      return;
    };
    let mut accounts: Vec<_> = entries
      .iter()
      .map(|entry| {
        (
          entry.stored.id,
          entry.stored.account.as_ref().map(|account| account.user),
        )
      })
      .collect();
    accounts.sort();
    let changed = {
      let mut current = self.inner.current.lock();
      let changed = current.accounts.as_ref().is_some_and(|seen| *seen != accounts);
      current.accounts = Some(accounts);
      changed
    };
    if changed {
      self.inner.renew();
    }
  }
}

impl Inner {
  fn renew(&self) {
    {
      let mut current = self.current.lock();
      current.client = QueryClient::new();
      current.sources.clear();
      current.ended.clear();
    }
    self.generation.send_modify(|generation| *generation += 1);
  }

  /// Keeps `source` for `root` in `generation`, its subscription followed and its changes applied; another source
  /// opened meanwhile for the same project wins.
  fn open(self: &Arc<Self>, root: &Path, source: TeamSource, generation: u64) -> TeamSource {
    let mut current = self.current.lock();
    if let Some(opened) = current.sources.get(root) {
      return opened.source.clone();
    }
    if *self.generation.borrow() != generation {
      // A generation ended while the source was opened: its reads end at once, as the old session's.
      return source;
    }
    let (project, client) = (source.project().clone(), current.client.clone());
    let follower = {
      let (project, inner, root) = (project.clone(), Arc::downgrade(self), root.to_owned());
      self.runtime.spawn(async move {
        let ended = project.follow(&SystemClock).await;
        ended_for(&inner, &root, generation, &ended);
      })
    };
    let pump = self.runtime.spawn(async move {
      invalidation::pump(project, client).await;
    });
    current.sources.insert(
      root.to_owned(),
      Opened {
        source: source.clone(),
        tasks: [follower, pump],
      },
    );
    source
  }
}

/// The reads of the project at `root` in `generation` ended with `ended`: when it ended them for the person, the
/// project stays ended and the session's generation ends with it, so nothing the old reads cached is shown again.
fn ended_for(inner: &Weak<Inner>, root: &Path, generation: u64, ended: &TeamReadError) {
  let Some(inner) = inner.upgrade() else {
    return;
  };
  if *inner.generation.borrow() != generation || *ended == TeamReadError::SessionChanged {
    return;
  }
  tracing::info!(%ended, "a team project's reads ended");
  inner.renew();
  inner
    .current
    .lock()
    .ended
    .insert(root.to_owned(), TeamFailure::from(ended));
}
