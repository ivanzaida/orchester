//! The project's Connections (layer `Connections` `mKKPM`): how its agents run (agent CLIs, and native
//! connections, which this version does not run), GitHub through `gh`, and the MCP servers its agents get.
//! The list with an inspector (`list`), Add connection for an agent CLI (`add_agent`) and Add / Edit MCP
//! server (`mcp_form`), each a page in the Connections tab; adding is a person's action in the window, and
//! the domain rules are `orchester_workspace::connections`', which the MCP setup tools share.
//!
//! Not built in this app yet, and visibly unavailable: adding or editing a GitHub connection and changing its
//! account or access, editing an agent CLI connection or choosing a program file other than the copies on
//! the search path, native connections (adding, testing, their models), and listing an MCP server's tools.

mod add_agent;
mod data;
mod inspector;
mod list;
mod mcp_form;
mod page;
mod widen;
mod words;
mod work;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ConnectionId;

use self::{
  add_agent::{AddAgent, AddAgentProps},
  list::{ConnectionsScreen, ListProps},
  mcp_form::{McpForm, McpFormProps},
};
use crate::{
  project::ShownProject,
  routes::{ConnectionsView, NewConnection},
};

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

pub fn edit_connection(ctx: &mut Ctx, project: &ShownProject, server: ConnectionId) -> Element {
  ctx.mount_keyed::<McpForm>(
    &format!("connection-edit-mcp-{}-{server}", project.open.project.id()),
    McpFormProps {
      project: project.clone(),
      server: Some(server),
    },
  )
}
