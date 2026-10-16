//! Keeps each signed-in server's connection current: asks it whether it still accepts this computer's
//! token (`GET /auth/me`) when signed in, every minute while connected, and on the backoff while it
//! cannot be reached. Until the live event streams of shared projects (S2d) this question is what
//! tells a server is there. A server that refuses the token is signed out and not asked again until
//! the person signs in. Every account's servers are kept current, also while another account is signed in: their
//! shared projects keep in step in the background ([`super::owners`]).

use std::time::Instant;

use tokio::time::sleep_until;

use super::{Behalf, IDLE, ServerEntry, Servers, SharingError};
use crate::background;

/// Runs until the window closes, as a task on the app runtime that `Root` starts and aborts when it drops.
pub async fn run(servers: Servers) {
  let mut changes = servers.subscribe();
  loop {
    changes.borrow_and_update();
    let entries = {
      let servers = servers.clone();
      background::run(move || servers.every_entry()).await
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
      self.ask(&entry, Behalf::Background).await;
      // After the check the server has a new time; read it again.
      let (_, after) = self.due(std::slice::from_ref(&entry), Instant::now());
      next = match (next, after) {
        (Some(next), Some(after)) => Some(next.min(after)),
        (next, after) => next.or(after),
      };
    }
    next
  }

  /// Asks `entry`, one of the signed-in account's servers, who is signed in now, as a screen that gates on the
  /// person's admin flag opens: the stored account follows what the server answers, not only the minute's check.
  pub async fn refresh_account(&self, entry: &ServerEntry) {
    if entry.stored.account.is_some() {
      self.ask(entry, Behalf::Account).await;
    }
  }

  /// Asks `entry` on `behalf` of the account or the background who is signed in, and keeps the account's name and
  /// admin flag current.
  async fn ask(&self, entry: &ServerEntry, behalf: Behalf) {
    let asked = self
      .with_token_for(behalf, entry, |client, token| async move { client.me(&token).await })
      .await;
    match asked {
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
      // The request recorded how the server answered; the connection shows it. Another account's server is asked
      // only in the background.
      Err(SharingError::Client(_) | SharingError::SignedOut | SharingError::ServerMissing) => {}
      Err(error) => {
        tracing::error!(%error, server = %entry.stored.id, "cannot ask the server who is signed in");
        self.failed(entry.stored.id);
      }
    }
  }
}
