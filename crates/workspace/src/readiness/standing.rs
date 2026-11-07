//! How a resource stands on this computer.

use orchester_core::{
  id::{ConnectionId, ResourceId},
  sharing::{Missing, Readiness},
};

/// Why what a resource needs of this computer cannot be known. It is not ready, and not assumed to need nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undecided {
  /// The version this computer uses is not a team profile body it can read: one that waits for the server, or one
  /// cached before the team configuration model (ORC-845 removes those).
  NotTeamForm,
  /// A team profile chooses an MCP server the team does not offer here: archived, or not synced yet.
  ChoosesMcpServer(ResourceId),
  /// A pipeline runs a profile or tool, or merges with an integrator, that this computer has no version of.
  RunsUnknown(ResourceId),
  /// A local profile runs on a connection that is not an agent connection of the project.
  ConnectionGone(ConnectionId),
}

/// Ready, what is missing, or why that cannot be known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
  Ready,
  /// Not ready; [`Readiness::missing`] is not empty.
  Missing(Readiness),
  Undecided(Undecided),
}

impl Standing {
  pub(super) fn of(readiness: Readiness) -> Self {
    if readiness.is_ready() {
      Self::Ready
    } else {
      Self::Missing(readiness)
    }
  }

  pub const fn is_ready(&self) -> bool {
    matches!(self, Self::Ready)
  }

  /// The parts missing, in the order to fix them; empty when ready or undecided.
  pub fn missing(&self) -> &[Missing] {
    match self {
      Self::Missing(readiness) => readiness.missing(),
      Self::Ready | Self::Undecided(_) => &[],
    }
  }

  /// The part to fix first.
  pub fn first(&self) -> Option<&Missing> {
    self.missing().first()
  }
}

/// The hub's groups of resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
  /// Not ready, or not decidable, and a person here can act: add a connection, set a path or a value, switch a server
  /// on. A profile that chooses a server that is off is here: the profile is what cannot run.
  NeedsYouHere,
  Ready,
  /// A team MCP server switched off on this computer. Its own missing parts still show on it.
  OffHere,
}
