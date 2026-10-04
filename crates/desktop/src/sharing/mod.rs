//! Shared servers on the desktop (`docs/architecture/server.md`, phase S2a): the servers the person
//! added, kept in the application database's `servers` table, each one's token in the OS keychain
//! (`VaultKey::server_token`), signing in and out, the person's sessions and the admin's users, and
//! each signed-in server's connection ([`monitor`]).
//!
//! A server is *in use* while an account is signed in to it from this computer, also after the
//! server stopped accepting the token (signed out). Every request reports how the server answered, so
//! a refused token shows as signed out at once and a server that stopped answering is retried with
//! the backoff. Shared projects (S2b) are in [`project`].

mod account;
mod connection;
pub mod device;
mod error;
pub mod feed;
mod monitor;
mod password;
pub mod project;

use std::{
  collections::HashMap,
  sync::Arc,
  time::{Duration, Instant},
};

use orchester_api::client::{Client, ClientError, PlainHttp, ServerAddress};
use orchester_core::{
  id::ServerId,
  secret::{SecretValue, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_store::{StoreError, StoredServer};
use parking_lot::Mutex;
use tokio::sync::watch;

pub use self::{
  account::{PendingPasswordChange, SignedIn},
  connection::{Connection, Schedule},
  error::{SharingError, client_message},
  monitor::run as monitor,
  password::{PASSWORD_CHARS, PasswordProblem, password_problem},
};
use crate::{background, secrets::Vault, workspace::Workspace};

/// A server in the list with its connection; `connection` is `None` while nobody is signed in there
/// from this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerEntry {
  pub stored: StoredServer,
  pub address: ServerAddress,
  pub connection: Option<Connection>,
  /// When the server last answered this session.
  pub last_reached: Option<Timestamp>,
}

impl ServerEntry {
  /// Whether the server is in use: an account is signed in, or was until the server refused it.
  pub fn in_use(&self) -> bool {
    self.stored.account.is_some()
  }
}

/// The servers and what the desktop knows about each one's connection. Every method that reaches the
/// database, the keychain or a server is meant for a lurq future: the blocking parts run on the
/// blocking pool.
#[derive(Clone)]
pub struct Servers {
  inner: Arc<Inner>,
}

struct Inner {
  workspace: Workspace,
  vault: Vault,
  schedule: Schedule,
  watched: Mutex<HashMap<ServerId, Watched>>,
  /// Counts changes of the list and of connections, for the monitor and the screens.
  changes: watch::Sender<u64>,
  /// Counts changes of shared projects' caches, for the screens.
  shared: watch::Sender<u64>,
  /// Counts requests to read the shared projects' caches again now.
  sync: watch::Sender<u64>,
}

/// One signed-in server's connection and when to ask it again.
#[derive(Clone, Copy, Debug)]
struct Watched {
  connection: Connection,
  /// When the monitor asks the server next; `None` while it is signed out.
  due: Option<Instant>,
  failures: u32,
  trouble_since: Option<Timestamp>,
  last_reached: Option<Timestamp>,
}

impl PartialEq for Servers {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl lurq::app::component::DevtoolsInspectable for Servers {}

impl Servers {
  pub fn new(workspace: Workspace, vault: Vault) -> Self {
    Self::with_schedule(workspace, vault, Schedule::DEFAULT)
  }

  /// Servers asked on `schedule`, e.g. a short one in tests.
  pub fn with_schedule(workspace: Workspace, vault: Vault, schedule: Schedule) -> Self {
    Self {
      inner: Arc::new(Inner {
        workspace,
        vault,
        schedule,
        watched: Mutex::new(HashMap::new()),
        changes: watch::channel(0).0,
        shared: watch::channel(0).0,
        sync: watch::channel(0).0,
      }),
    }
  }

  /// Wakes whenever the list or a connection changed.
  pub fn subscribe(&self) -> watch::Receiver<u64> {
    self.inner.changes.subscribe()
  }

  fn changed(&self) {
    self.inner.changes.send_modify(|count| *count += 1);
  }

  /// Wakes whenever a shared project's cache changed.
  pub fn subscribe_shared(&self) -> watch::Receiver<u64> {
    self.inner.shared.subscribe()
  }

  fn shared_changed(&self) {
    self.inner.shared.send_modify(|count| *count += 1);
  }

  fn subscribe_sync(&self) -> watch::Receiver<u64> {
    self.inner.sync.subscribe()
  }

  /// Reads the shared projects' caches again now (opening a project, *Try now*, *Check again*).
  pub fn sync_now(&self) {
    self.inner.sync.send_modify(|count| *count += 1);
  }

  /// Asks `server` again now (*Try now*): the monitor checks it at once, then the caches are read.
  pub fn retry_now(&self, server: ServerId) {
    {
      let mut watched = self.inner.watched.lock();
      if let Some(watched) = watched.get_mut(&server).filter(|watched| watched.due.is_some()) {
        watched.due = Some(Instant::now());
      }
    }
    self.changed();
    self.sync_now();
  }

  /// The connection of `server` as last seen: checking until it answered this session.
  pub fn connection(&self, server: ServerId) -> Connection {
    self
      .inner
      .watched
      .lock()
      .get(&server)
      .map_or(Connection::Checking, |watched| watched.connection)
  }

  /// The servers with their connections, in the order they were added. Blocking.
  pub fn entries(&self) -> Result<Vec<ServerEntry>, SharingError> {
    let stored = self.inner.workspace.servers()?;
    let watched = self.inner.watched.lock();
    stored
      .into_iter()
      .map(|stored| {
        let address = ServerAddress::parse(&stored.url).map_err(|error| StoreError::Corrupt {
          what: "server address",
          detail: error.to_string(),
        })?;
        let state = watched.get(&stored.id).filter(|_| stored.account.is_some());
        Ok(ServerEntry {
          connection: stored
            .account
            .as_ref()
            .map(|_| state.map_or(Connection::Checking, |state| state.connection)),
          last_reached: state.and_then(|state| state.last_reached),
          address,
          stored,
        })
      })
      .collect()
  }

  /// What answers at `address`, asked before the server is added (10.02, 10.03).
  pub async fn check(
    &self,
    address: ServerAddress,
    plain_http: PlainHttp,
  ) -> Result<orchester_api::ServerInfo, ClientError> {
    Client::new(address, plain_http)?.server_info().await
  }

  /// Adds the checked server to the list; returns its id.
  pub async fn add(
    &self,
    address: &ServerAddress,
    name: String,
    plain_http: PlainHttp,
  ) -> Result<ServerId, SharingError> {
    let stored = StoredServer {
      id: ServerId::generate(),
      url: address.as_str().to_owned(),
      name,
      plain_http_allowed: plain_http == PlainHttp::Allowed,
      added_at: Timestamp::now(),
      account: None,
    };
    let workspace = self.inner.workspace.clone();
    let id = stored.id;
    background::run(move || workspace.add_server(&stored)).await??;
    self.changed();
    Ok(id)
  }

  /// The client of `server`, with the plain HTTP opt-in stored with it.
  fn client(server: &ServerEntry) -> Result<Client, SharingError> {
    let plain_http = if server.stored.plain_http_allowed {
      PlainHttp::Allowed
    } else {
      PlainHttp::NotAllowed
    };
    Ok(Client::new(server.address.clone(), plain_http)?)
  }

  /// The token of `server` from the keychain; signed out when there is none.
  async fn token(&self, server: ServerId) -> Result<SecretValue, SharingError> {
    let vault = self.inner.vault.clone();
    match background::run(move || vault.get().read(&VaultKey::server_token(server))).await? {
      Ok(token) => Ok(token),
      Err(VaultError::NotFound) => Err(SharingError::SignedOut),
      Err(error) => Err(error.into()),
    }
  }

  async fn store_token(&self, server: ServerId, token: SecretValue) -> Result<(), SharingError> {
    let vault = self.inner.vault.clone();
    background::run(move || vault.get().store(&VaultKey::server_token(server), &token)).await??;
    Ok(())
  }

  async fn delete_token(&self, server: ServerId) -> Result<(), SharingError> {
    let vault = self.inner.vault.clone();
    background::run(move || vault.get().delete(&VaultKey::server_token(server))).await??;
    Ok(())
  }

  /// Runs `call` with `server`'s client and token and records how the server answered: a refused
  /// token signs this computer out of it, a server that cannot be reached is retried.
  async fn with_token<T, F, Fut>(&self, server: &ServerEntry, call: F) -> Result<T, SharingError>
  where
    F: FnOnce(Client, SecretValue) -> Fut,
    Fut: Future<Output = Result<T, ClientError>>,
  {
    let client = Self::client(server)?;
    let token = match self.token(server.stored.id).await {
      Ok(token) => token,
      Err(SharingError::SignedOut) => {
        self.mark_signed_out(server.stored.id);
        return Err(SharingError::SignedOut);
      }
      Err(error) => return Err(error),
    };
    let answer = call(client, token).await;
    self.record(server.stored.id, answer.as_ref().err()).await;
    match answer {
      Err(ClientError::Api {
        code: orchester_api::ErrorCode::Unauthorized,
        ..
      }) => Err(SharingError::SignedOut),
      answer => Ok(answer?),
    }
  }

  /// Records an answer of `server`: `None` for a success.
  async fn record(&self, server: ServerId, error: Option<&ClientError>) {
    match error {
      None => self.reached(server),
      Some(ClientError::Api {
        code: orchester_api::ErrorCode::Unauthorized,
        ..
      }) => self.refused(server).await,
      Some(error) if error.is_connection() => self.failed(server),
      // Any other answer came from the server, which is therefore reachable.
      Some(_) => self.reached(server),
    }
  }

  /// The server answered.
  fn reached(&self, server: ServerId) {
    self.update(server, |_| Watched {
      connection: Connection::Connected,
      due: Some(Instant::now() + self.inner.schedule.connected_check),
      failures: 0,
      trouble_since: None,
      last_reached: Some(Timestamp::now()),
    });
  }

  /// An attempt to reach the server failed.
  fn failed(&self, server: ServerId) {
    self.update(server, |watched| {
      let failures = watched.map_or(1, |watched| watched.failures + 1);
      let trouble_since = watched
        .and_then(|watched| watched.trouble_since)
        .unwrap_or_else(Timestamp::now);
      let connection = self
        .inner
        .schedule
        .after_failure(failures, Instant::now(), trouble_since, connection::jitter());
      let due = match connection {
        Connection::Reconnecting { next, .. } | Connection::Offline { next, .. } => Some(next),
        _ => None,
      };
      Watched {
        connection,
        due,
        failures,
        trouble_since: Some(trouble_since),
        last_reached: watched.and_then(|watched| watched.last_reached),
      }
    });
  }

  /// The server refused the token: it can never work again, so it leaves the keychain.
  async fn refused(&self, server: ServerId) {
    if let Err(error) = self.delete_token(server).await {
      // The token stays in the keychain, unused: the server refuses it, and signing in again
      // replaces it.
      tracing::error!(%error, %server, "cannot remove a refused server token");
    }
    self.mark_signed_out(server);
  }

  fn mark_signed_out(&self, server: ServerId) {
    self.update(server, |watched| Watched {
      connection: Connection::SignedOut,
      due: None,
      failures: 0,
      trouble_since: None,
      last_reached: watched.and_then(|watched| watched.last_reached),
    });
  }

  fn update(&self, server: ServerId, change: impl FnOnce(Option<&Watched>) -> Watched) {
    {
      let mut watched = self.inner.watched.lock();
      let next = change(watched.get(&server));
      watched.insert(server, next);
    }
    self.changed();
  }

  /// Forgets the connection of `server`, e.g. after signing out.
  fn forget(&self, server: ServerId) {
    self.inner.watched.lock().remove(&server);
    self.changed();
  }

  /// The servers the monitor should ask now, and when it should look again.
  fn due(&self, entries: &[ServerEntry], now: Instant) -> (Vec<ServerEntry>, Option<Instant>) {
    let watched = self.inner.watched.lock();
    let mut due = Vec::new();
    let mut next: Option<Instant> = None;
    for entry in entries.iter().filter(|entry| entry.in_use()) {
      match watched.get(&entry.stored.id) {
        None => due.push(entry.clone()),
        Some(Watched { due: Some(at), .. }) if *at <= now => due.push(entry.clone()),
        Some(Watched { due: Some(at), .. }) => next = Some(next.map_or(*at, |next| next.min(*at))),
        Some(Watched { due: None, .. }) => {}
      }
    }
    (due, next)
  }
}

/// How long the monitor waits when nothing is due: until the list or a connection changes.
const IDLE: Duration = Duration::from_secs(3600);
