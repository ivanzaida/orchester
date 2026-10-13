//! The clients that connected to the app's MCP server since the app started, or since the token last changed, as
//! each named itself in its `initialize` request: nothing is stored, and nothing is inferred beyond what the
//! client sent (`docs/design/app-mcp-connection.md`, "Who is connected"). A client is connected while one of its
//! MCP sessions is open; a session ends when its service is dropped (the client ended it, it expired, or the
//! server stopped or changed its token).

use std::{sync::Arc, time::SystemTime};

use parking_lot::Mutex;
use tokio::sync::watch;

use super::status::{McpStatus, SeenClient};

/// The clients seen, shared by the server's sessions; changes are published in the server's status.
#[derive(Clone)]
pub struct Clients(Arc<Shared>);

struct Shared {
  seen: Mutex<Seen>,
  status: watch::Sender<McpStatus>,
}

#[derive(Default)]
struct Seen {
  /// Raised when the list starts again, so a session of before cannot change a client of after.
  generation: u64,
  clients: Vec<Client>,
}

struct Client {
  name: String,
  version: String,
  open: usize,
  last_request: SystemTime,
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
    let mut seen = self.0.seen.lock();
    seen.generation += 1;
    seen.clients.clear();
    drop(seen);
    self.publish();
  }

  fn opened(&self, name: &str, version: &str) -> Named {
    let mut seen = self.0.seen.lock();
    let now = SystemTime::now();
    let index = match seen
      .clients
      .iter()
      .position(|client| client.name == name && client.version == version)
    {
      Some(index) => {
        let client = &mut seen.clients[index];
        client.open += 1;
        client.last_request = now;
        index
      }
      None => {
        seen.clients.push(Client {
          name: name.to_owned(),
          version: version.to_owned(),
          open: 1,
          last_request: now,
        });
        seen.clients.len() - 1
      }
    };
    let named = Named {
      generation: seen.generation,
      index,
    };
    drop(seen);
    self.publish();
    named
  }

  fn requested(&self, named: &Named) {
    let mut seen = self.0.seen.lock();
    if seen.generation != named.generation {
      return;
    }
    if let Some(client) = seen.clients.get_mut(named.index) {
      client.last_request = SystemTime::now();
    }
  }

  fn closed(&self, named: &Named) {
    let mut seen = self.0.seen.lock();
    if seen.generation != named.generation {
      return;
    }
    if let Some(client) = seen.clients.get_mut(named.index) {
      client.open = client.open.saturating_sub(1);
    }
    drop(seen);
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
  named: Mutex<Option<Named>>,
}

struct Named {
  generation: u64,
  index: usize,
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
    if let Some(named) = self.0.named.lock().as_ref() {
      self.0.clients.requested(named);
    }
  }
}

impl Drop for Mark {
  fn drop(&mut self) {
    if let Some(named) = self.named.get_mut().take() {
      self.clients.closed(&named);
    }
  }
}
