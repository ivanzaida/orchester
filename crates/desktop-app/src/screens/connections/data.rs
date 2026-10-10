//! The project's connections as the screen reads them: all of them together (`ProjectConnections`), off the
//! UI thread, again whenever the screen's revision moves; and the connections of a tab, in the order the
//! list shows them: agent providers, GitHub, MCP servers.

use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureState};
use orchester_core::{
  connection::{AgentConnection, GitHubConnection, GitHubState, mcp::McpServer},
  id::ConnectionId,
};
use orchester_workspace::{
  background,
  connections::data::ProjectConnections,
  messages::{self, Message},
};

use crate::routes::ConnectionsTab;

/// The project's connections at `repo_root`, read when the screen shows and again for each `revision`.
pub(super) fn load(ctx: &mut Ctx, repo_root: PathBuf, revision: u64) -> FutureState<ProjectConnections, Message> {
  let read = ctx.future((repo_root, revision), |(repo_root, _)| async move {
    background::run(move || ProjectConnections::load_in(&repo_root))
      .await?
      .map_err(|error| {
        tracing::error!(%error, "the project's connections cannot be read");
        messages::store_error(&error)
      })
  });
  read.state().get()
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

  pub(super) const fn tab(self) -> ConnectionsTab {
    match self {
      Self::Agent(_) => ConnectionsTab::AgentProviders,
      Self::GitHub(_) => ConnectionsTab::GitHub,
      Self::Mcp(_) => ConnectionsTab::McpServers,
    }
  }
}

/// The connections `tab` lists, agent providers first.
pub(super) fn entries(listed: &ProjectConnections, tab: ConnectionsTab) -> Vec<Entry<'_>> {
  let agents = listed.connections.iter().map(Entry::Agent);
  let github = listed.github.iter().map(Entry::GitHub);
  let mcp = listed.mcp.iter().map(Entry::Mcp);
  agents
    .chain(github)
    .chain(mcp)
    .filter(|entry| tab == ConnectionsTab::All || entry.tab() == tab)
    .collect()
}

/// How many connections each tab lists, in the order of [`ConnectionsTab::ALL`].
pub(super) fn counts(listed: &ProjectConnections) -> [usize; 4] {
  let (agents, github, mcp) = (listed.connections.len(), listed.github.len(), listed.mcp.len());
  [agents + github + mcp, agents, github, mcp]
}

/// The count of `tab` among `counts`.
pub(super) fn count(counts: [usize; 4], tab: ConnectionsTab) -> usize {
  let index = ConnectionsTab::ALL
    .iter()
    .position(|counted| *counted == tab)
    .expect("every tab is counted");
  counts[index]
}

/// How a GitHub connection stands for the project: the account its repository link expects, if the link
/// goes through it.
pub(super) fn github_state(listed: &ProjectConnections, connection: &GitHubConnection) -> GitHubState {
  let expected = listed
    .link_of(connection.id())
    .map(orchester_core::repository::RepositoryLink::expected_login);
  connection.state(expected)
}
