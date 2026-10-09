//! How a project's task list stands with its server, as every screen of the project shows it
//! (design 10.42, 10.44, decision 16): local, shared and writable, or shared and read-only because
//! the server cannot be reached, nobody is signed in to it, or the person is no longer a member.

use std::time::Instant;

use orchester_api::client::ServerAddress;
use orchester_core::{id::UserId, time::Timestamp};
use orchester_store::SharingLink;

use crate::sharing::{Connection, ServerEntry};

/// A shared project's link with what the desktop knows of its server now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSharing {
  pub link: SharingLink,
  /// The server's name and address; `None` when it is no longer in the list.
  pub server: Option<(String, ServerAddress)>,
  pub connection: Option<Connection>,
  /// The person signed in to the server from this computer, and whether they are its admin.
  pub me: Option<UserId>,
  /// The person's username there, for asking an owner to add them again.
  pub username: Option<String>,
  pub admin: bool,
}

/// Whether the task list takes changes, and why not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskListAccess {
  /// The server takes changes (or the project is local).
  Writable,
  /// The server cannot be reached, since `since` once it reads offline: the cache last matched it
  /// at `synced`; the next attempt is at `next`.
  Offline {
    synced: Timestamp,
    since: Option<Timestamp>,
    next: Instant,
  },
  /// Nobody on this computer is signed in to the server any more.
  SignedOut { synced: Timestamp },
  /// The person is no longer a member, since `since`.
  NotMember { since: Timestamp },
}

impl TaskListAccess {
  pub const fn read_only(self) -> bool {
    !matches!(self, Self::Writable)
  }
}

impl ProjectSharing {
  /// `link` with its server as `entry` holds it.
  pub fn new(link: SharingLink, entry: Option<&ServerEntry>) -> Self {
    Self {
      server: entry.map(|entry| (entry.stored.name.clone(), entry.address.clone())),
      connection: entry.and_then(|entry| entry.connection),
      me: entry.and_then(|entry| entry.stored.account.as_ref().map(|account| account.user)),
      username: entry.and_then(|entry| entry.stored.account.as_ref().map(|account| account.username.clone())),
      admin: entry
        .and_then(|entry| entry.stored.account.as_ref())
        .is_some_and(|account| account.admin),
      link,
    }
  }

  pub fn server_name(&self) -> &str {
    self.server.as_ref().map_or("", |(name, _)| name.as_str())
  }

  pub fn access(&self) -> TaskListAccess {
    let synced = self.link.last_synced_at;
    if let Some(since) = self.link.removed_at {
      return TaskListAccess::NotMember { since };
    }
    match self.connection {
      Some(Connection::Connected | Connection::Checking) => TaskListAccess::Writable,
      Some(Connection::Reconnecting { next, .. }) => TaskListAccess::Offline {
        synced,
        since: None,
        next,
      },
      Some(Connection::Offline { since, next }) => TaskListAccess::Offline {
        synced,
        since: Some(since),
        next,
      },
      Some(Connection::SignedOut) | None => TaskListAccess::SignedOut { synced },
    }
  }
}

/// The access of a project with `sharing`: a local project's list always takes changes.
pub fn access(sharing: Option<&ProjectSharing>) -> TaskListAccess {
  sharing.map_or(TaskListAccess::Writable, ProjectSharing::access)
}
