//! The QA app's `settings` scenario (`--settings`): what app Settings shows that needs a cloud and team servers
//! answering while the window is open, set up as a person would have before the window opens. Fixture data for QA,
//! deterministic and never shipped: loopback servers with fixture accounts, the stand-in cloud's invite, and the
//! installed app's MCP server on a free port with a token in memory. The seed's `settings` scenario wrote the rest.
//!
//! Team servers, each joined with a code as Ivan Zaida (`ivan`) and renamed so each reads apart:
//! - *Kontur team*: connected, Kontur linked to it, a member of Billing too (its invite landing);
//! - *Build box*: offline, its server stopped once joined;
//! - *Old office*: its own certificate, which changed since it was compared (nothing is sent to it);
//! - *Design team*: its own certificate, signed out of on this computer, so *Connect a server* compares its check code
//!   again;
//! - *Team box*: its sign-in ended, after its admin signed Ivan's devices out.
//!
//! Invites: one to Billing on Kontur team waiting at the cloud; kept here, a join of *Mobile app* waiting for approval
//! and one of *Docs* that didn't finish.

use std::{error::Error, net::TcpListener, path::Path, sync::Arc};

use orchester_api::{
  SharedProject,
  client::{PlainHttp, ServerAddress},
};
use orchester_app::{mcp::McpMode, services::Services};
use orchester_core::{
  app_mcp::McpPort,
  id::{ApprovalId, IdempotencyKey, InviteId, ServerId, TeamServerId},
  time::Timestamp,
};
use orchester_store::{AcceptedInvite, AppDb};
use orchester_workspace::{
  secrets::Vault,
  sharing::{CodeRequest, PresentedCertificate, Trust, project::LinkFirst},
  workspace::Workspace,
};
use tokio::runtime::Runtime;

use super::{
  server_support::TestServer,
  signed_in::{
    MemoryKeychain,
    account_cloud::{AccountCloud, EMAIL},
  },
};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// A certificate pin no server presents: *Old office*'s compared one, before its certificate changed.
const EARLIER_PIN: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";

/// What answers while the window is open: the servers that stay up.
pub struct Fixtures {
  _servers: Vec<TestServer>,
}

/// Sets the scenario up for the session of `services` and `workspace`, whose database is `database`, signed in to
/// `cloud`; the installed app's MCP server, for Settings to show.
pub fn prepare(
  (services, workspace): (&Services, &Workspace),
  (runtime, database): (&Runtime, &Path),
  cloud: &AccountCloud,
) -> Result<(Fixtures, McpMode), Box<dyn Error>> {
  let join = Joiner {
    services,
    runtime,
    database,
  };
  let (kontur, billing) = connected(&join, cloud)?;
  let offline = TestServer::start();
  join.plain(&offline, "Build box")?;
  drop(offline);
  let changed = join_own(&join, "Old office")?;
  AppDb::open(database)?.set_server_trust(changed.1, Some(EARLIER_PIN), false)?;
  let (design, id) = join_own(&join, "Design team")?;
  let entry = entry(services, id)?;
  runtime.block_on(services.servers.sign_out(&entry))?;
  let ended = TestServer::start();
  join.plain(&ended, "Team box")?;
  ended.reset_password(&ended.device_of("alice"), "ivan");
  let invite = kontur.project_email_invite(EMAIL, &billing);
  cloud.invite(&invite, Some(("Billing", &format!("{}/billing", kontur.url()))));
  keep_joins(database)?;
  Ok((
    Fixtures {
      _servers: vec![kontur, changed.0, design, ended],
    },
    installation(workspace)?,
  ))
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

/// *Kontur team*, connected, with Kontur and Billing; the seeded Kontur linked to it. The stand-in cloud names it, so
/// its invite lists.
fn connected(join: &Joiner<'_>, cloud: &AccountCloud) -> Result<(TestServer, SharedProject), Box<dyn Error>> {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  cloud.name_server(&audience, &server.url(), None);
  let kontur = server.create_named_project(("Kontur", "KON"), "kontur", None);
  let billing = server.create_named_project(
    ("Billing", "BIL"),
    "billing",
    Some("https://github.com/acme/billing.git"),
  );
  let id = join.with(&server, server.project_code(&kontur), ("Kontur team", Own::No))?;
  server.add_member(&billing, "ivan");
  let repo = AppDb::open(join.database)?
    .every_project()?
    .into_iter()
    .next()
    .ok_or("the seed registered Kontur")?
    .repo_root;
  let request = LinkFirst {
    slug: "kontur".into(),
    remote: None,
    key: IdempotencyKey::generate(),
    write_discovery: false,
  };
  let entry = entry(join.services, id)?;
  join
    .runtime
    .block_on(join.services.servers.link_first(&entry, &repo, request))?;
  Ok((server, billing))
}

/// A server presenting its own certificate, joined once its check code was compared, as `name`.
fn join_own(join: &Joiner<'_>, name: &str) -> Result<(TestServer, ServerId), Box<dyn Error>> {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let code = server.code_for("ivan", false);
  server.serve_self_signed();
  let id = join.with(&server, code, (name, Own::Yes))?;
  Ok((server, id))
}

/// Whether a server presents its own certificate.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Own {
  Yes,
  No,
}

/// Joins servers from the window's session.
struct Joiner<'a> {
  services: &'a Services,
  runtime: &'a Runtime,
  database: &'a Path,
}

impl Joiner<'_> {
  /// Joins `server`, on plain HTTP to this computer, as Ivan with an account code once Alice set it up, as `name`.
  fn plain(&self, server: &TestServer, name: &str) -> Result<ServerId, Box<dyn Error>> {
    server.create_user("alice", "Alice Moreau", true);
    self.with(server, server.code_for("ivan", false), (name, Own::No))
  }

  /// Joins `server` as Ivan with `code`, trusting what it presents (its own certificate compared, as a person
  /// would), and calls it `name` in the list.
  fn with(
    &self,
    server: &TestServer,
    code: orchester_api::Credential,
    (name, own): (&str, Own),
  ) -> Result<ServerId, Box<dyn Error>> {
    let address = ServerAddress::parse(&server.url())?;
    let trust = match own {
      Own::Yes => self.runtime.block_on(PresentedCertificate::read(&address))?.confirm(),
      Own::No => Trust::System(PlainHttp::NotAllowed),
    };
    let request = CodeRequest {
      code,
      username: Some("ivan".into()),
      display_name: Some("Ivan Zaida".into()),
      proof: Some(server.code_proof("ivan")),
    };
    let id = self
      .runtime
      .block_on(self.services.servers.join_with_code(&address, trust, request))?;
    AppDb::open(self.database)?.set_server_name(id, name)?;
    Ok(id)
  }
}

/// The list's entry of `id`.
fn entry(services: &Services, id: ServerId) -> Result<orchester_workspace::sharing::ServerEntry, Box<dyn Error>> {
  services
    .servers
    .entries()?
    .into_iter()
    .find(|entry| entry.stored.id == id)
    .ok_or_else(|| "the joined server is in the list".into())
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
