//! Keeps each signed-in server's connection current: asks it whether it still accepts this computer's
//! token (`GET /auth/me`) when signed in, every minute while connected, and on the backoff while it
//! cannot be reached. Until the live event streams of shared projects (S2d) this question is what
//! tells a server is there. A server that refuses the token is signed out and not asked again until
//! the person signs in.

use std::time::Instant;

use tokio::time::sleep_until;

use super::{IDLE, ServerEntry, Servers, SharingError};
use crate::background;

/// Runs until the window closes; meant for a lurq stream of the root.
pub async fn run(servers: Servers) {
  let mut changes = servers.subscribe();
  loop {
    changes.borrow_and_update();
    let entries = {
      let servers = servers.clone();
      background::run(move || servers.entries()).await
    };
    let next = match entries {
      Ok(Ok(entries)) => servers.check_due(&entries).await,
      Ok(Err(error)) => {
        tracing::error!(%error, "cannot read the shared servers");
        None
      }
      // The blocking pool is shutting down: so is the window.
      Err(_) => return,
    };
    let wake = next.unwrap_or_else(|| Instant::now() + IDLE);
    tokio::select! {
      () = sleep_until(wake.into()) => {}
      changed = changes.changed() => if changed.is_err() { return },
    }
  }
}

impl Servers {
  /// Asks every server that is due; returns when the next one is due.
  async fn check_due(&self, entries: &[ServerEntry]) -> Option<Instant> {
    let (due, mut next) = self.due(entries, Instant::now());
    for entry in due {
      self.ask(&entry).await;
      // After the check the server has a new time; read it again.
      let (_, after) = self.due(std::slice::from_ref(&entry), Instant::now());
      next = match (next, after) {
        (Some(next), Some(after)) => Some(next.min(after)),
        (next, after) => next.or(after),
      };
    }
    next
  }

  /// Asks `entry` who is signed in, and keeps the account's name and admin flag current.
  async fn ask(&self, entry: &ServerEntry) {
    match self.me(entry).await {
      Ok(user) => {
        let Some(account) = entry.stored.account.as_ref() else {
          return;
        };
        if account.admin == user.admin && account.display_name == user.display_name {
          return;
        }
        let mut account = account.clone();
        account.admin = user.admin;
        account.display_name = user.display_name;
        let (workspace, server) = (self.inner.workspace.clone(), entry.stored.id);
        match background::run(move || workspace.set_server_account(server, Some(&account))).await {
          Ok(Ok(())) => self.changed(),
          Ok(Err(error)) => tracing::error!(%error, %server, "cannot record the server account's changes"),
          // The blocking pool is shutting down with the window; nothing is left to show it.
          Err(_) => {}
        }
      }
      // `me` recorded how the server answered; the connection shows it.
      Err(SharingError::Client(_) | SharingError::SignedOut) => {}
      Err(error) => {
        tracing::error!(%error, server = %entry.stored.id, "cannot ask the server who is signed in");
        self.failed(entry.stored.id);
      }
    }
  }
}
