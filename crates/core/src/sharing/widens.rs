//! Whether a teammate's version widens what this computer's agents may do (`docs/architecture/server.md`, row 35,
//! "A teammate's version that widens"): then it waits for the person's *Use v<N> here*, otherwise it applies at once.
//! It compares as the MCP person gate does, with the same functions ([`AgentPermissions::widenings_from`],
//! [`McpServerSelection::adds_to`], [`CommandAction::widenings_from`](crate::resource::CommandAction::widenings_from),
//! [`tool_uses`]), and adds what a shared tool can change beyond a command.

use std::collections::BTreeSet;

use crate::{
  connection::mcp::McpServerSelection,
  id::ResourceId,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess,
    widening::{CommandWidening, Widening, changed_variables},
  },
  resource::{
    AgentProfile, CommandEnvironment, HttpRequest, Instruction, Pipeline, RequestTarget, Skill, TeamAgentProfile, Tool,
    ToolAction,
  },
  secret::tool_uses,
};

/// One way a version widens what agents may do over the version this computer uses.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VersionWidening {
  /// An agent profile's permissions are broader.
  Permissions(Widening),
  /// An agent profile chooses an MCP server the previous version did not.
  McpServer,
  /// A team agent profile gives its agents a tool the previous version did not.
  Tool,
  /// The version references a secret the previous version did not.
  Secret,
  /// A command tool's command runs what it did not, or a tool now runs a command.
  Command(CommandWidening),
  /// An HTTP tool's address changed, or a decision reviewer permits requests to additional hosts.
  Address,
  /// An HTTP tool's method changed.
  Method,
  /// The headers an HTTP tool's inputs are sent in changed.
  Headers,
  /// A database tool's database (the credentials it connects with) changed, or a tool now queries one.
  Database,
  /// A database tool's query changed.
  Query,
  /// A tool's access policy changed, or a decision reviewer's minimum approval threshold decreased.
  Access,
}

/// A body of a kind a server project shares, which knows how a version of it widens over the previous one.
pub trait SharedBody {
  /// How this version widens over `previous`; over nothing granted when it is the first version received.
  fn widenings_over(&self, previous: Option<&Self>) -> Vec<VersionWidening>;
}

/// How `next`, a version published from another computer, widens what this computer's agents may do over
/// `previous`, the version this computer uses now (`None` before the first one). Empty when it applies at once.
pub fn widens<B: SharedBody>(previous: Option<&B>, next: &B) -> Vec<VersionWidening> {
  next.widenings_over(previous)
}

/// The local body of a team profile ([`AgentProfile::from_team`]), which runs on the connection this computer picked
/// for it; the server's body is a [`TeamAgentProfile`].
impl SharedBody for AgentProfile {
  /// A first version is compared with permissions that grant nothing: read-only, no command, network off and no MCP
  /// server. Unlike the person gate, it does not fit them to the profile's connection, which this computer picks.
  fn widenings_over(&self, previous: Option<&Self>) -> Vec<VersionWidening> {
    let servers = |profile: &Self| profile.mcp_servers().clone();
    let mut widenings = permission_widenings(self.permissions(), previous.map(AgentProfile::permissions));
    if self
      .mcp_servers()
      .adds_to(&previous.map_or_else(McpServerSelection::none, servers))
    {
      widenings.push(VersionWidening::McpServer);
    }
    widenings
  }
}

/// A team profile widens as a local one does, and also when it names a team MCP server or a tool, or needs a secret
/// name, the previous version did not; naming fewer widens nothing. Its references are the server's ids and secret
/// names, which are the same on every computer, so they are compared as they are. Skills and instructions are not
/// counted: they change what agents are told, not what they may do.
impl SharedBody for TeamAgentProfile {
  fn widenings_over(&self, previous: Option<&Self>) -> Vec<VersionWidening> {
    let mut widenings = permission_widenings(self.permissions(), previous.map(TeamAgentProfile::permissions));
    let added = |now: &[ResourceId], before: Option<&[ResourceId]>| {
      now
        .iter()
        .any(|item| !before.is_some_and(|before| before.contains(item)))
    };
    if added(self.mcp_servers(), previous.map(TeamAgentProfile::mcp_servers)) {
      widenings.push(VersionWidening::McpServer);
    }
    if added(self.tools(), previous.map(TeamAgentProfile::tools)) {
      widenings.push(VersionWidening::Tool);
    }
    let needs_new_secret = self
      .secrets()
      .iter()
      .any(|secret| !previous.is_some_and(|previous| previous.secrets().contains(secret)));
    if needs_new_secret {
      widenings.push(VersionWidening::Secret);
    }
    widenings
  }
}

fn permission_widenings(next: &AgentPermissions, previous: Option<&AgentPermissions>) -> Vec<VersionWidening> {
  let nothing = nothing_granted();
  next
    .widenings_from(previous.unwrap_or(&nothing))
    .into_iter()
    .map(VersionWidening::Permissions)
    .collect()
}

fn nothing_granted() -> AgentPermissions {
  AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
}

impl SharedBody for Tool {
  /// A tool that did not exist or ran another kind of action grants what its action does. A changed description,
  /// timeout or output limit, or narrower exit codes, widen nothing; any change of its access policy widens.
  fn widenings_over(&self, previous: Option<&Self>) -> Vec<VersionWidening> {
    let mut widenings = match (previous.map(Tool::action), self.action()) {
      (Some(ToolAction::Command(before)), ToolAction::Command(after)) => after
        .widenings_from(before)
        .into_iter()
        .map(VersionWidening::Command)
        .collect(),
      (Some(ToolAction::Http(before)), ToolAction::Http(after)) => http(before, after),
      (Some(ToolAction::DatabaseQuery(before)), ToolAction::DatabaseQuery(after)) => changed([
        (before.credentials() != after.credentials(), VersionWidening::Database),
        (before.query() != after.query(), VersionWidening::Query),
      ]),
      (_, action) => granted(action),
    };
    if previous.is_some_and(|previous| previous.access() != self.access()) {
      widenings.push(VersionWidening::Access);
    }
    if self
      .secret()
      .is_some_and(|secret| !previous.is_some_and(|previous| tool_uses(previous, secret)))
    {
      widenings.push(VersionWidening::Secret);
    }
    widenings
  }
}

fn http(before: &HttpRequest, after: &HttpRequest) -> Vec<VersionWidening> {
  let headers = |request: &HttpRequest| {
    request
      .inputs()
      .iter()
      .filter_map(|(field, target)| match target {
        RequestTarget::Header(name) => Some((field.clone(), name.to_ascii_lowercase())),
        RequestTarget::Query(_) | RequestTarget::Body(_) => None,
      })
      .collect::<BTreeSet<_>>()
  };
  let address = before.url() != after.url() || !before.allowed_host().eq_ignore_ascii_case(after.allowed_host());
  changed([
    (address, VersionWidening::Address),
    (before.method() != after.method(), VersionWidening::Method),
    (headers(before) != headers(after), VersionWidening::Headers),
  ])
}

fn changed<const N: usize>(checks: [(bool, VersionWidening); N]) -> Vec<VersionWidening> {
  checks
    .into_iter()
    .filter_map(|(changed, widening)| changed.then_some(widening))
    .collect()
}

/// What an action grants that nothing did before: running its program with its variables, reaching its address,
/// or querying its database.
fn granted(action: &ToolAction) -> Vec<VersionWidening> {
  match action {
    ToolAction::Command(command) => std::iter::once(CommandWidening::Program)
      .chain(
        changed_variables(&CommandEnvironment::default(), command.environment())
          .into_iter()
          .map(CommandWidening::Variable),
      )
      .map(VersionWidening::Command)
      .collect(),
    ToolAction::Http(_) => vec![VersionWidening::Address],
    ToolAction::DatabaseQuery(_) => vec![VersionWidening::Database],
  }
}

/// A pipeline's steps can only narrow the permissions of the profiles they choose, and it names no MCP server or
/// secret, so no version of it widens.
impl SharedBody for Pipeline {
  fn widenings_over(&self, _previous: Option<&Self>) -> Vec<VersionWidening> {
    Vec::new()
  }
}

/// Instructions change what agents are told, not what they may do.
impl SharedBody for Instruction {
  fn widenings_over(&self, _previous: Option<&Self>) -> Vec<VersionWidening> {
    Vec::new()
  }
}

/// Skills change what agents are told, not what they may do.
impl SharedBody for Skill {
  fn widenings_over(&self, _previous: Option<&Self>) -> Vec<VersionWidening> {
    Vec::new()
  }
}
