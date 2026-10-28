//! The QA app's `settings` scenario (`--settings`): what app Settings shows that needs more than the seed's database,
//! set up as a person would have before the window opens. Fixture data for QA, deterministic and never shipped: the
//! installed app's MCP server on a free port with a token in memory, and two invites accepted here whose join did not
//! finish (a join of *Mobile app* waiting for approval and one of *Docs* that didn't finish). The seed's `settings`
//! scenario wrote the rest. Team servers answering on loopback are the platform repository's (`crates/e2e`, the
//! `n1_qa` example), where the team server lives.

use std::{error::Error, net::TcpListener, path::Path, sync::Arc};

use orchester_app::mcp::McpMode;
use orchester_core::{
  app_mcp::McpPort,
  id::{ApprovalId, InviteId, TeamServerId},
  time::Timestamp,
};
use orchester_store::{AcceptedInvite, AppDb};
use orchester_workspace::{secrets::Vault, workspace::Workspace};

use super::signed_in::MemoryKeychain;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Sets the scenario up for the session of `workspace`, whose database is `database`; the installed app's MCP server,
/// for Settings to show.
pub fn prepare(workspace: &Workspace, database: &Path) -> Result<McpMode, Box<dyn Error>> {
  keep_joins(database)?;
  installation(workspace)
}

/// The installed app's MCP server as Settings shows it: on, at a free port of this computer, its token in a keychain
/// in memory, with two fixture agents connected to it ([`super::agents`]). The window's own server stays the QA
/// copy's, whose discovery file the verifier connects with.
fn installation(workspace: &Workspace) -> Result<McpMode, Box<dyn Error>> {
  let port = TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port();
  workspace.set_mcp_port(McpPort::new(u32::from(port))?)?;
  workspace.set_mcp_server_enabled(true)?;
  let keychain = Arc::new(MemoryKeychain::default());
  super::agents::connect(port, Arc::clone(&keychain));
  Ok(McpMode::Installation {
    workspace: workspace.clone(),
    vault: Vault::with(keychain),
  })
}

/// Two invites accepted here whose join did not finish: *Mobile app*'s waits for approval and is not asked again for a
/// day, so nothing reaches a server; *Docs*'s stopped and is asked again only when a person presses *Try again*.
fn keep_joins(database: &Path) -> Result<(), Box<dyn Error>> {
  let now = Timestamp::now();
  let at = |days: i64| Timestamp::from_unix_millis(now.unix_millis() + days * DAY_MS);
  let kept = |server: &str, project: &str| {
    AcceptedInvite::new(
      (InviteId::generate(), TeamServerId::generate(), server.to_owned()),
      Some(project.to_owned()),
      at(7),
      now,
    )
  };
  let mut db = AppDb::open(database)?;
  db.keep_accepted_invite(&AcceptedInvite {
    approval: Some(ApprovalId::generate()),
    approval_expires_at: Some(at(7)),
    asks: 1,
    next_check_at: Some(at(1)),
    ..kept("Mobile team", "Mobile app")
  })?;
  db.keep_accepted_invite(&AcceptedInvite {
    asks: 1,
    ..kept("Docs team", "Docs")
  })?;
  Ok(())
}
