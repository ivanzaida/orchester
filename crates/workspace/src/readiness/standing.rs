//! How a resource stands on this computer.

use orchester_core::{
  id::{ConnectionId, ResourceId, SecretId},
  resource::{ResourceKind, TeamProfileError},
  sharing::{Missing, Readiness},
};

/// Why what a resource needs of this computer cannot be known. It is not ready, and not assumed to need nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Undecided {
  /// The version this computer uses is not a team body it can read: a profile the server sent in a form this version
  /// cannot read (one cached before the team configuration model is such a form) or a reviewer that names a connection
  /// (ORC-887), or one that is neither the server's nor waiting to be sent to it.
  NotTeamForm,
  /// A version of a team profile published here, which the server has not numbered, cannot be put in the team's form,
  /// so it cannot be published either: this says why (writable folders, a local MCP server that is no team one, a
  /// connection that is no agent connection).
  CannotBePublished(TeamProfileError),
  /// A team profile chooses an MCP server the team does not offer here: not synced yet (an archived one is
  /// [`Self::ChoosesArchivedMcpServer`]).
  ChoosesMcpServer(ResourceId),
  /// A team profile chooses an MCP server the team archived; see [`Self::UsesArchivedTool`]. The server's id and name.
  ChoosesArchivedMcpServer(ResourceId, String),
  /// A team profile names a team tool the team archived: the server refuses a run on an archived resource, so the
  /// profile cannot run until an admin restores the tool or the profile stops naming it (edge case 8). The tool's id
  /// and its name.
  UsesArchivedTool(ResourceId, String),
  /// A team profile names a team instruction the team archived; see [`Self::UsesArchivedTool`].
  UsesArchivedInstruction(ResourceId, String),
  /// A pipeline runs a profile or tool, or merges with an integrator, that this computer has no version of.
  RunsUnknown(ResourceId),
  /// A step of a local pipeline names a profile or tool that was archived here: its id and its name. The pipeline keeps
  /// naming it and cannot start until it is restored or the step names another (ORC-842).
  RunsArchived(ResourceId, String),
  /// A local profile chooses a local MCP server that was archived here: its id and its name. The profile cannot run
  /// until the server is restored or the profile stops choosing it.
  ChoosesArchivedLocalMcpServer(ConnectionId, String),
  /// A local profile runs on a connection that is not an agent connection of the project.
  ConnectionGone(ConnectionId),
  /// A tool of a team project holds a project secret by id. A team names its secrets, so nobody can say whether the
  /// secret this id stands for has a value on the teammates' computers (ORC-830 puts the tool in the team's form).
  TeamToolSecretById(SecretId),
  /// A tool of a local project uses a secret the project does not have, or has archived: no value can be set for it.
  SecretGone(SecretId),
  /// The keychain refused to say whether a tool's secret has a value here, or this platform has none.
  SecretUnreadable(SecretId),
  /// A pipeline runs the profile or tool `id` (or merges with it), which is undecided for `why`: the pipeline is, and
  /// the first member that is says which.
  Member { id: ResourceId, why: Box<Undecided> },
}

/// Whether a team profile that still names an archived resource of `kind` cannot run here: an MCP server, a tool or an
/// instruction ([`Undecided::ChoosesArchivedMcpServer`], [`Undecided::UsesArchivedTool`],
/// [`Undecided::UsesArchivedInstruction`]). A skill does not, because this computer keeps none of the team's and cannot
/// tell an archived one; no other kind is named by a profile.
pub const fn archive_blocks_profiles(kind: ResourceKind) -> bool {
  matches!(
    kind,
    ResourceKind::McpServer | ResourceKind::Tool | ResourceKind::Instruction
  )
}

/// Ready, what is missing, or why that cannot be known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadinessStanding {
  Ready,
  /// Not ready; [`Readiness::missing`] is not empty.
  Missing(Readiness),
  Undecided(Undecided),
}

impl ReadinessStanding {
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
  /// Not ready, or not decidable: a resource that lacks a connection, a path or a value, or a server that is off, or
  /// whose standing is undecided. It is where a person looks first; for an undecided one there may be nothing to do
  /// here, and its [`Undecided`] says what waits for what. A profile that chooses a server that is off is here: the
  /// profile is what cannot run.
  NeedsYouHere,
  Ready,
  /// A team MCP server a person switched off here, or that automation did: switching off is the safe direction, so
  /// automation may (`set_mcp_server_enabled` with `enabled: false`) and the choice is kept as a person's would be.
  /// One nobody has decided on is not here but in [`Self::NeedsYouHere`], missing [`Missing::McpOff`]. Its own
  /// missing parts still show on it, and a profile that chooses it shows [`Missing::McpOff`] in
  /// [`Self::NeedsYouHere`].
  OffHere,
}
