//! The project's connections as the screen reads them: all of them together (`ProjectConnections`), off the
//! UI thread, again whenever the screen's revision moves; and each of them, in the order the tab's groups show
//! them: agent providers, GitHub, MCP servers.

use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureHandle, FutureState};
use orchester_core::{
  connection::{AgentConnection, GitHubConnection, GitHubState, mcp::McpServer},
  id::ConnectionId,
  time::Timestamp,
};
use orchester_workspace::{
  background,
  connections::data::ProjectConnections,
  messages::{self, Message},
};

/// The project's connections at `repo_root`, read when the screen shows and again for each `revision`.
pub(super) fn load(ctx: &mut Ctx, repo_root: PathBuf, revision: u64) -> FutureState<ProjectConnections, Message> {
  reading(ctx, repo_root, revision).state().get()
}

/// [`load`]'s reading itself, for a page that follows its outcome.
pub(super) fn reading(ctx: &mut Ctx, repo_root: PathBuf, revision: u64) -> FutureHandle<ProjectConnections, Message> {
  let hold = crate::services::Services::of(ctx).hold;
  ctx.future((repo_root, revision), move |(repo_root, _)| {
    tracing::trace!(target: crate::app_log::READ_LOG, read = "connections");
    let hold = hold.clone();
    async move {
      hold.released().await;
      background::run(move || ProjectConnections::load_in(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "the project's connections cannot be read");
          messages::store_error(&error)
        })
    }
  })
}

/// The connection a form saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Saved(pub ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// One connection of the list.
#[derive(Clone, Copy)]
pub(super) enum Entry<'a> {
  Agent(&'a AgentConnection),
  GitHub(&'a GitHubConnection),
  Mcp(&'a McpServer),
}

impl<'a> Entry<'a> {
  pub(super) fn id(self) -> ConnectionId {
    match self {
      Self::Agent(connection) => connection.id(),
      Self::GitHub(connection) => connection.id(),
      Self::Mcp(server) => server.id(),
    }
  }

  pub(super) fn name(self) -> &'a str {
    match self {
      Self::Agent(connection) => connection.name().as_str(),
      Self::GitHub(connection) => connection.name().as_str(),
      Self::Mcp(server) => server.name().as_str(),
    }
  }

  /// When its program was last checked; an MCP server is never checked.
  pub(super) fn checked_at(self) -> Option<Timestamp> {
    match self {
      Self::Agent(connection) => connection.last_check().map(|check| check.at),
      Self::GitHub(connection) => connection.last_check().map(|check| check.at),
      Self::Mcp(_) => None,
    }
  }
}

/// Every connection, agent providers first, then GitHub, then MCP servers.
pub(super) fn entries(listed: &ProjectConnections) -> Vec<Entry<'_>> {
  let agents = listed.connections.iter().map(Entry::Agent);
  let github = listed.github.iter().map(Entry::GitHub);
  let mcp = listed.mcp.iter().map(Entry::Mcp);
  agents.chain(github).chain(mcp).collect()
}

/// How a GitHub connection stands for the project: the account its repository link expects, if the link
/// goes through it.
pub(super) fn github_state(listed: &ProjectConnections, connection: &GitHubConnection) -> GitHubState {
  let expected = listed
    .link_of(connection.id())
    .map(orchester_core::repository::RepositoryLink::expected_login);
  connection.state(expected)
}
