//! The clients that connected to the app's MCP server since the app started, or since the token last changed, as
//! each named itself in its `initialize` request: nothing is stored, and nothing is inferred beyond what the
//! client sent (`docs/design/app-mcp-connection.md`, "Who is connected"). A client is connected while one of its
//! MCP sessions is open; a session ends when its service is dropped (the client ended it, it expired, or the
//! server stopped or changed its token). The list keeps [`KEPT`] clients, connected ones before those that left, so
//! clients that name themselves anew each time cannot grow it without end.

use std::{sync::Arc, time::SystemTime};

use parking_lot::Mutex;
use tokio::sync::watch;

use super::status::{McpStatus, SeenClient};

/// The most clients the list keeps.
pub const KEPT: usize = 20;

/// The clients seen, shared by the server's sessions; changes are published in the server's status.
#[derive(Clone)]
pub struct Clients(Arc<Shared>);

struct Shared {
  seen: Mutex<Seen>,
  status: watch::Sender<McpStatus>,
}

#[derive(Default)]
struct Seen {
  /// Never used twice, so a session of a client that left the list cannot change another.
  next: u64,
  clients: Vec<Client>,
}

struct Client {
  id: u64,
  name: String,
  version: String,
  open: usize,
  last_request: SystemTime,
}

impl Seen {
  fn client(&mut self, id: u64) -> Option<&mut Client> {
    self.clients.iter_mut().find(|client| client.id == id)
  }

  /// A new client in the list, which drops one once it holds more than [`KEPT`]: the one that asked something
  /// longest ago among those not connected, or among all when every one is connected.
  fn add(&mut self, name: &str, version: &str, now: SystemTime) -> u64 {
    let id = self.next;
    self.next += 1;
    self.clients.push(Client {
      id,
      name: name.to_owned(),
      version: version.to_owned(),
      open: 1,
      last_request: now,
    });
    if self.clients.len() > KEPT
      && let Some(oldest) =
        (0..self.clients.len()).min_by_key(|index| (self.clients[*index].open > 0, self.clients[*index].last_request))
    {
      self.clients.remove(oldest);
    }
    id
  }
}

impl Clients {
  pub fn new(status: watch::Sender<McpStatus>) -> Self {
    Self(Arc::new(Shared {
      seen: Mutex::new(Seen::default()),
      status,
    }))
  }

  /// A mark for one new session, which names its client once it initializes.
  pub fn session(&self) -> SessionMark {
    SessionMark(Arc::new(Mark {
      clients: self.clone(),
      named: Mutex::new(None),
    }))
  }

  /// Forgets every client: the token changed, so the list starts again.
  pub fn start_again(&self) {
    self.0.seen.lock().clients.clear();
    self.publish();
  }

  fn opened(&self, name: &str, version: &str) -> u64 {
    let mut seen = self.0.seen.lock();
    let now = SystemTime::now();
    let known = seen
      .clients
      .iter_mut()
      .find(|client| client.name == name && client.version == version);
    let id = match known {
      Some(client) => {
        client.open += 1;
        client.last_request = now;
        client.id
      }
      None => seen.add(name, version, now),
    };
    drop(seen);
    self.publish();
    id
  }

  fn requested(&self, id: u64) {
    if let Some(client) = self.0.seen.lock().client(id) {
      client.last_request = SystemTime::now();
    }
  }

  fn closed(&self, id: u64) {
    if let Some(client) = self.0.seen.lock().client(id) {
      client.open = client.open.saturating_sub(1);
    }
    self.publish();
  }

  fn publish(&self) {
    let clients: Vec<SeenClient> = self
      .0
      .seen
      .lock()
      .clients
      .iter()
      .map(|client| SeenClient {
        name: client.name.clone(),
        version: client.version.clone(),
        connected: client.open > 0,
        last_request: client.last_request,
      })
      .collect();
    self.0.status.send_if_modified(|status| {
      let changed = status.clients != clients;
      status.clients = clients;
      changed
    });
  }
}

/// One session's place in the list; clones share it, and the last one dropped ends the session.
#[derive(Clone)]
pub struct SessionMark(Arc<Mark>);

struct Mark {
  clients: Clients,
  /// The session's client, once it named itself.
  named: Mutex<Option<u64>>,
}

impl SessionMark {
  /// The session's client named itself; a second `initialize` on the same session is not counted again.
  pub fn initialized(&self, name: &str, version: &str) {
    let mut named = self.0.named.lock();
    if named.is_some() {
      return;
    }
    *named = Some(self.0.clients.opened(name, version));
  }

  /// The session's client sent a request.
  pub fn requested(&self) {
    if let Some(id) = *self.0.named.lock() {
      self.0.clients.requested(id);
    }
  }
}

impl Drop for Mark {
  fn drop(&mut self) {
    if let Some(id) = self.named.get_mut().take() {
      self.clients.closed(id);
    }
  }
}
