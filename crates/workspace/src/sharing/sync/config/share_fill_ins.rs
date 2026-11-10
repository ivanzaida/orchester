//! What a computer fills in after Share, from what it had locally (`docs/design/team-configuration.md`, "What a
//! computer fills in"), so the sharer's runs stay ready: the connection each profile ran on is its pick, each team MCP
//! server that was on is on, the program path of each stdio server is set for the team server (a program started by a
//! name on the search path is looked up here once, on the sharer's own computer), and the value of each team secret a
//! server takes is kept under that secret's name. The values are read from the keychain before anything is uploaded,
//! so a keychain that refuses stops the Share with nothing sent.
//!
//! Share is the person's own action in the window, so setting these here widens nothing: the servers, programs and
//! secrets are the ones the profiles already ran with. Every fill-in is applied, whatever happens to another; what
//! could not be set comes back for the Share to say ([`NotSet`]).

use std::{
  ffi::OsStr,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::ConnectionKind,
  id::{ResourceId, ServerId},
  resource::{AgentProfile, CommandProgram, ProgramLocation, ResourceKind},
  secret::{SecretName, SecretValue, SecretVault, VaultError},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ConnectionPick, ProjectDb};

use super::share_mcp::{FillSource, LocalProgram, McpPlan, PlannedServer};
use crate::{
  sharing::{NotSet, ShareNote, SharingError},
  tasks::outside::Outside,
  tools::program::fixed_program,
  workspace::Workspace,
};

/// The scheme a pasted Basic credential may start with; the team's form holds the part after it.
const BASIC: &str = "Basic ";

/// One thing this computer sets for the shared project. It holds secret values, so it has no `Debug`.
enum FillIn {
  Pick {
    profile: ResourceId,
    name: String,
    pick: ConnectionPick,
  },
  On {
    server: ResourceId,
    name: String,
  },
  Program {
    server: ResourceId,
    name: String,
    path: PathBuf,
  },
  Secret {
    name: SecretName,
    value: SecretValue,
  },
}

impl FillIn {
  /// What it is to the person when it could not be set.
  fn not_set(&self) -> NotSet {
    match self {
      Self::Pick { name, .. } => NotSet::Connection { profile: name.clone() },
      Self::On { name, .. } => NotSet::McpOn { server: name.clone() },
      Self::Program { name, .. } => NotSet::McpProgramKept { server: name.clone() },
      Self::Secret { name, .. } => NotSet::SecretKept { secret: name.clone() },
    }
  }
}

/// What a Share leaves for the person to read.
#[derive(Debug, Default)]
pub struct FillInOutcome {
  /// What this computer could not set, which stays missing where the team's readiness is shown.
  pub not_set: Vec<NotSet>,
  pub notes: Vec<ShareNote>,
}

/// The fill-ins of a Share, read from the project before it is uploaded. It holds secret values, so it has no `Debug`.
pub struct ShareFillIns {
  fill_ins: Vec<FillIn>,
  outcome: FillInOutcome,
}

impl ShareFillIns {
  /// What the project at `root` fills in once it is shared. A program started by a name is looked up on
  /// `search_path` (`orchester_exec::search_path::search_path()`). Blocking.
  pub fn read(root: &Path, vault: &dyn SecretVault, search_path: Option<&OsStr>) -> Result<Self, SharingError> {
    let db = ProjectDb::open(root)?;
    let plan = McpPlan::read(&db)?;
    let mut read = Self {
      fill_ins: picks(&db)?,
      outcome: FillInOutcome {
        not_set: Vec::new(),
        notes: plan.notes().to_vec(),
      },
    };
    for server in plan.servers() {
      read.read_server(server, (&db, vault), (root, search_path))?;
    }
    Ok(read)
  }

  fn read_server(
    &mut self,
    server: &PlannedServer,
    (db, vault): (&ProjectDb, &dyn SecretVault),
    (root, search_path): (&Path, Option<&OsStr>),
  ) -> Result<(), SharingError> {
    let name = server.body.name().as_str().to_owned();
    match &server.program {
      LocalProgram::None => {}
      LocalProgram::Path(path) => self.fill_ins.push(FillIn::Program {
        server: server.resource,
        name: name.clone(),
        path: path.clone(),
      }),
      LocalProgram::Name(program) => match found_here(program, search_path, root) {
        Some(path) => self.fill_ins.push(FillIn::Program {
          server: server.resource,
          name: name.clone(),
          path,
        }),
        None => self
          .outcome
          .not_set
          .push(NotSet::McpProgramPath { server: name.clone() }),
      },
    }
    if server.enabled {
      self.fill_ins.push(FillIn::On {
        server: server.resource,
        name,
      });
    }
    for fill in &server.fills {
      if self
        .fill_ins
        .iter()
        .any(|kept| matches!(kept, FillIn::Secret { name, .. } if *name == fill.name))
      {
        continue;
      }
      match secret_value(&fill.source, db, vault)? {
        Some(value) => self.fill_ins.push(FillIn::Secret {
          name: fill.name.clone(),
          value,
        }),
        None => self.outcome.not_set.push(NotSet::SecretValue {
          secret: fill.name.clone(),
        }),
      }
    }
    Ok(())
  }

  /// Keeps them for the project at `root`, shared on `server`, and says what could not be set. Blocking.
  pub fn apply(
    self,
    (workspace, vault): (&Workspace, &dyn SecretVault),
    root: &Path,
    server: ServerId,
  ) -> FillInOutcome {
    let Self { fill_ins, mut outcome } = self;
    let mut db = match ProjectDb::open(root) {
      Ok(db) => db,
      Err(error) => {
        tracing::error!(%error, "this computer's fill-ins for the shared configuration were not set: the project is unreadable");
        outcome.not_set.extend(fill_ins.iter().map(FillIn::not_set));
        return outcome;
      }
    };
    let mut changed = false;
    for fill_in in &fill_ins {
      match set(fill_in, (&mut db, vault), (workspace, server)) {
        Ok(()) => changed = true,
        Err(error) => {
          tracing::error!(%error, "a fill-in for the shared configuration was not set");
          outcome.not_set.push(fill_in.not_set());
        }
      }
    }
    if changed {
      workspace.changes.record(db.project().id(), Outside::Connections);
    }
    outcome
  }
}

fn set(
  fill_in: &FillIn,
  (db, vault): (&mut ProjectDb, &dyn SecretVault),
  (workspace, server): (&Workspace, ServerId),
) -> Result<(), SharingError> {
  match fill_in {
    FillIn::Pick { profile, pick, .. } => {
      db.set_connection_pick(*profile, *pick)?;
    }
    FillIn::On { server, .. } => db.set_team_mcp_on_here(*server, true)?,
    FillIn::Secret { name, value } => db.set_team_secret_value(vault, name, value)?,
    FillIn::Program {
      server: resource, path, ..
    } => {
      set_program(&mut workspace.db.lock(), server, *resource, path)?;
    }
  }
  Ok(())
}

fn set_program(app: &mut AppDb, server: ServerId, resource: ResourceId, path: &Path) -> Result<(), SharingError> {
  app.set_mcp_server_program(server, resource, path, Timestamp::now())?;
  Ok(())
}

/// The file `program` is on this computer's `search_path`; none when it is found nowhere (or lies in the project's
/// repository), where the person sets the path themselves.
fn found_here(program: &CommandProgram, search_path: Option<&OsStr>, root: &Path) -> Option<PathBuf> {
  match fixed_program(program, search_path, root) {
    Ok(found) => match found.location() {
      ProgramLocation::Absolute(path) => Some(path.to_path_buf()),
      ProgramLocation::OnPath(_) | ProgramLocation::InWorktree(_) => None,
    },
    Err(problem) => {
      tracing::warn!(
        ?problem,
        "the program of a shared MCP server was not found on this computer's search path"
      );
      None
    }
  }
}

/// The value `source` has here for its team secret; none when there is none to carry (a secret with no value in the
/// keychain, or a blank one). A keychain that refuses is an error: the Share stops with nothing sent.
fn secret_value(
  source: &FillSource,
  db: &ProjectDb,
  vault: &dyn SecretVault,
) -> Result<Option<SecretValue>, SharingError> {
  let text = match source {
    FillSource::Text(text) => text.expose().to_owned(),
    FillSource::Local { secret, basic } => match vault.read(&secret.vault_key(db.project().id())) {
      Ok(value) if *basic => basic_part(value.expose()).to_owned(),
      Ok(value) => value.expose().to_owned(),
      // The secret has no value here already, so there is none to carry; the team secret shows as missing.
      Err(VaultError::NotFound) => return Ok(None),
      Err(error) => return Err(error.into()),
    },
  };
  // A blank value is none to carry.
  Ok(SecretValue::new(text).ok())
}

/// A pasted Basic credential as the team holds it: the part after the scheme, whether it was pasted with the scheme
/// (`Basic dXNlcjpwYXNz`) or without it. The server sends `Basic ` followed by it either way.
fn basic_part(credential: &str) -> &str {
  credential
    .get(..BASIC.len())
    .filter(|start| start.eq_ignore_ascii_case(BASIC))
    .map_or(credential, |_| &credential[BASIC.len()..])
}

/// The connection of each profile's newest published version, which the profile keeps running on.
fn picks(db: &ProjectDb) -> Result<Vec<FillIn>, SharingError> {
  let connections = db.connections()?;
  let mut picks = Vec::new();
  for profile in db.resources(ResourceKind::AgentProfile)? {
    let Some(newest) = db.latest_version::<AgentProfile>(profile.id())? else {
      continue;
    };
    let connection = newest.body().connection();
    let agent = connections
      .iter()
      .any(|found| found.id() == connection && matches!(found.kind(), ConnectionKind::Agent(_)));
    if agent {
      picks.push(FillIn::Pick {
        profile: profile.id(),
        name: profile.name().as_str().to_owned(),
        pick: ConnectionPick {
          connection,
          account: None,
        },
      });
    }
  }
  Ok(picks)
}
