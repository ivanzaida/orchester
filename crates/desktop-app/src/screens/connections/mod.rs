//! Project settings › Connections (`05 Connections` `dk46o` in `Project settings · redesign`): how the project's agents
//! run (agent CLIs, and native connections, whose agents Orchester runs itself on a model API), GitHub through `gh`,
//! and the MCP servers its agents get. The groups of connections (`list`), a connection's page (`inspector`), Add
//! provider for an agent CLI or a native connection (`add_agent`, `native_form`) and Add / Edit MCP server
//! (`mcp_form`), each a page under the tab with a crumb back to it; adding is a person's action in the window, and the
//! domain rules are `orchester_workspace::connections`', which the MCP setup tools share. Project settings draws the
//! page around them (`project_settings::page`).
//!
//! Edit connection of an agent CLI connection chooses its account, of a native connection its key, endpoint and models
//! (`edit_agent`); an agent or GitHub connection is renamed in place of its page's title, an MCP server in its form.
//! Not built in this app yet, and visibly unavailable: adding or editing a GitHub connection and changing its account
//! or access, choosing a program file other than the copies on the search path, the OpenAI API's native agents, and
//! listing an MCP server's tools.

mod account;
mod add_agent;
mod data;
mod edit_agent;
mod inspector;
mod list;
mod mcp_form;
mod native_form;
mod page;
mod provider_form;
pub(crate) mod widen;
pub(super) mod words;
mod work;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ConnectionId;

use self::{
  add_agent::{AddAgent, AddAgentProps},
  edit_agent::{EditConnection, EditProps},
  list::{ConnectionsScreen, ListProps},
  mcp_form::{McpForm, McpFormProps},
};
use crate::{
  project::ShownProject,
  routes::{ConnectionsView, NewConnection},
};

/// The Connections tab: its groups, or the page of the connection `view` names.
pub fn screen(ctx: &mut Ctx, project: &ShownProject, view: ConnectionsView) -> Element {
  let key = format!("connections-{}", project.open.project.id());
  ctx.mount_keyed::<ConnectionsScreen>(
    &key,
    ListProps {
      project: project.clone(),
      view,
    },
  )
}

/// Add provider or Add MCP server, as `kind` says.
pub fn new_connection(ctx: &mut Ctx, project: &ShownProject, kind: NewConnection) -> Element {
  let id = project.open.project.id();
  match kind {
    NewConnection::AgentProvider => ctx.mount_keyed::<AddAgent>(
      &format!("connection-add-agent-{id}"),
      AddAgentProps {
        project: project.clone(),
      },
    ),
    NewConnection::McpServer => ctx.mount_keyed::<McpForm>(
      &format!("connection-add-mcp-{id}"),
      McpFormProps {
        project: project.clone(),
        server: None,
      },
    ),
  }
}

/// Edit connection: an MCP server's form, or the account of an agent CLI connection.
pub fn edit_connection(ctx: &mut Ctx, project: &ShownProject, connection: ConnectionId) -> Element {
  ctx.mount_keyed::<EditConnection>(
    &format!("connection-edit-{}-{connection}", project.open.project.id()),
    EditProps {
      project: project.clone(),
      connection,
    },
  )
}
