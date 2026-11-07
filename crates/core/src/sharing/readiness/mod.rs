//! Whether a team resource is ready here, or exactly what is missing (`docs/design/team-configuration.md`,
//! "Readiness"). A pure rule: [`Needs`] is what the pinned versions ask of a computer and [`Here`] is what this
//! computer has, both plain data, so the workspace reads the filesystem and the database and a run can decide
//! `not_ready_here` from the pinned versions alone. Local projects use the same rule; only the words differ.
//!
//! The missing parts come in a stable order, the one a person fixes them in: the connection of each profile, then
//! the MCP servers that are off, then their program paths (a team server's, then a local one's), then the secret
//! values (a team secret's, by name, then a project secret's, by id). Inside a kind they keep the order the needs were
//! asked in. The first is the one a pinned action offers to fix.

mod here;
mod needs;

use std::path::PathBuf;

use self::needs::Need;
pub use self::{
  here::Here,
  needs::{McpNeed, Needs, NeedsError},
};
use crate::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId, SecretId},
  secret::SecretName,
};

/// Why a profile has no connection here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionProblem {
  /// No connection of the profile's CLI kind: one has to be added.
  NoneOfKind,
  /// Several connections of the kind and none picked for this profile: the person picks one.
  PickNeeded,
}

/// Why a team MCP server's program cannot start here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramProblem {
  /// No path is set on this computer.
  NotSet,
  /// A path is set and its file is gone.
  FileGone(PathBuf),
}

/// One part a computer lacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
  Connection {
    profile: ResourceId,
    cli: AgentProvider,
    problem: ConnectionProblem,
  },
  /// The team MCP server is not switched on here.
  McpOff { server: ResourceId },
  McpProgram {
    server: ResourceId,
    problem: ProgramProblem,
  },
  /// No value is set here for the team secret.
  Secret { name: SecretName },
  /// The program of a local project's MCP server, which a profile chooses, is no file on the `PATH` or in the project.
  LocalMcpProgram { server: ConnectionId },
  /// The project secret a local tool sends or connects with has no value here.
  LocalSecret { secret: SecretId, name: SecretName },
}

/// The answer: ready, or the parts missing, ordered and deduplicated. Only [`readiness`] makes one that is not ready.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readiness {
  missing: Vec<Missing>,
}

impl Readiness {
  /// Nothing is missing.
  pub const READY: Self = Self { missing: Vec::new() };

  pub const fn is_ready(&self) -> bool {
    self.missing.is_empty()
  }

  /// The parts missing, in the order to fix them; empty when ready.
  pub fn missing(&self) -> &[Missing] {
    &self.missing
  }

  /// The part to fix first.
  pub fn first(&self) -> Option<&Missing> {
    self.missing().first()
  }
}

impl Missing {
  const fn rank(&self) -> u8 {
    match self {
      Self::Connection { .. } => 0,
      Self::McpOff { .. } => 1,
      Self::McpProgram { .. } => 2,
      Self::LocalMcpProgram { .. } => 3,
      Self::Secret { .. } => 4,
      Self::LocalSecret { .. } => 5,
    }
  }
}

/// Whether `needs` are met by `here`: [`Readiness::READY`] or what is missing, ordered and without repeats.
pub fn readiness(needs: &Needs, here: &Here) -> Readiness {
  let mut missing: Vec<Missing> = Vec::new();
  for part in needs.iter().filter_map(|need| lacking(need, here)) {
    if !missing.contains(&part) {
      missing.push(part);
    }
  }
  // Stable, so inside a kind the parts keep the order they were asked in.
  missing.sort_by_key(Missing::rank);
  Readiness { missing }
}

fn lacking(need: &Need, here: &Here) -> Option<Missing> {
  match need {
    Need::Connection { profile, cli } => connection_problem(*profile, *cli, here).map(|problem| Missing::Connection {
      profile: *profile,
      cli: *cli,
      problem,
    }),
    Need::McpOn(server) => (!here.mcp_on.contains(server)).then_some(Missing::McpOff { server: *server }),
    Need::McpProgram(server) => match here.programs.get(server) {
      Some(file) if file.exists => None,
      Some(file) => Some(Missing::McpProgram {
        server: *server,
        problem: ProgramProblem::FileGone(file.path.clone()),
      }),
      None => Some(Missing::McpProgram {
        server: *server,
        problem: ProgramProblem::NotSet,
      }),
    },
    Need::Secret(name) => (!here.secrets.contains(name)).then(|| Missing::Secret { name: name.clone() }),
    Need::LocalMcpProgram(server) => {
      (!here.local_programs.contains(server)).then_some(Missing::LocalMcpProgram { server: *server })
    }
    Need::LocalSecret { secret, name } => (!here.local_secrets.contains(secret)).then(|| Missing::LocalSecret {
      secret: *secret,
      name: name.clone(),
    }),
  }
}

/// A pick of a connection of the kind wins; without one a single connection of the kind is used, several need a pick
/// and none need one added.
fn connection_problem(profile: ResourceId, cli: AgentProvider, here: &Here) -> Option<ConnectionProblem> {
  let mut of_kind = here.connections.iter().filter(|(_, kind)| *kind == cli);
  let picked = here
    .picks
    .get(&profile)
    .is_some_and(|pick| here.connections.iter().any(|(id, kind)| id == pick && *kind == cli));
  if picked {
    return None;
  }
  match (of_kind.next(), of_kind.next()) {
    (None, _) => Some(ConnectionProblem::NoneOfKind),
    (Some(_), None) => None,
    (Some(_), Some(_)) => Some(ConnectionProblem::PickNeeded),
  }
}
