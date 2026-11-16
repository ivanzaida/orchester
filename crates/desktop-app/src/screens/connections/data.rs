//! The project's connections as the screen reads them: all of them together (`ProjectConnections`) through their
//! query, which the project's loader read as the project opened, and which a change of them, the screen's own work
//! included, reads again (`queries`); and each of them, in the order the tab's groups show them: agent providers,
//! GitHub, MCP servers.

use std::path::PathBuf;

use lurq::{app::ctx::Ctx, query::QueryHandle};
use orchester_core::{
  connection::{GitHubConnection, GitHubState, mcp::McpServer},
  id::ConnectionId,
  time::Timestamp,
};
use orchester_workspace::{connections::data::ProjectConnections, messages::Message};

use crate::queries::{Session, project};

/// The project's connections at `repo_root`: their last read in the first frame, read again behind it when a change
/// invalidated them.
pub(in crate::screens) fn load(ctx: &mut Ctx, repo_root: PathBuf) -> QueryHandle<ProjectConnections, Message> {
  let session = Session::of(ctx);
  ctx.query(project::connections(session, repo_root))
}

/// The connection a form saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Saved(pub ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// One connection of the list.
#[derive(Clone, Copy)]
pub(super) enum Entry<'a> {
  GitHub(&'a GitHubConnection),
  Mcp(&'a McpServer),
}

impl<'a> Entry<'a> {
  pub(super) fn id(self) -> ConnectionId {
    match self {
      Self::GitHub(connection) => connection.id(),
      Self::Mcp(server) => server.id(),
    }
  }

  pub(super) fn name(self) -> &'a str {
    match self {
      Self::GitHub(connection) => connection.name().as_str(),
      Self::Mcp(server) => server.name().as_str(),
    }
  }

  /// When its program was last checked; an MCP server is never checked.
  pub(super) fn checked_at(self) -> Option<Timestamp> {
    match self {
      Self::GitHub(connection) => connection.last_check().map(|check| check.at),
      Self::Mcp(_) => None,
    }
  }
}

/// Every connection that has a page: GitHub, then the MCP servers. An agent connection has none: Orchester keeps
/// it for an account, which Settings › Accounts shows.
pub(super) fn entries(listed: &ProjectConnections) -> Vec<Entry<'_>> {
  let github = listed.github.iter().map(Entry::GitHub);
  let mcp = listed.mcp.iter().map(Entry::Mcp);
  github.chain(mcp).collect()
}

/// How a GitHub connection stands for the project: the account its repository link expects, if the link
/// goes through it.
pub(super) fn github_state(listed: &ProjectConnections, connection: &GitHubConnection) -> GitHubState {
  let expected = listed
    .link_of(connection.id())
    .map(orchester_core::repository::RepositoryLink::expected_login);
  connection.state(expected)
}
