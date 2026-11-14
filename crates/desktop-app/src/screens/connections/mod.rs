//! What is left of Project settings › Connections now that a project's agents run on the person's accounts
//! (`docs/design/agent-accounts.md`, "Agents run on accounts"; `Agents on accounts` D1 `E6kvpm`): the tab and the
//! pages of agent connections are gone, with their add and edit forms. What stays opens from where it belongs:
//! GitHub's page (`inspector::github`) from Project settings › General's *GitHub account* row, an MCP server's page
//! and its form (`inspector::mcp`, `mcp_form`) from the Agents hub, and a decision model's page with its test
//! (`decision`) from Project settings › Runs. The base URL, key and model fields of an API provider
//! (`provider_form`, `native_form`) are Settings › Accounts' now, and what a change widens (`widen`) is shared by
//! the forms that ask it.

pub(in crate::screens) mod data;
mod decision;
mod inspector;
mod list;
pub(in crate::screens) mod mcp_form;
mod naming;
pub(in crate::screens) mod native_form;
mod page;
pub(in crate::screens) mod provider_form;
pub(crate) mod widen;
pub(super) mod words;
mod work;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ConnectionId;

use self::{
  list::{ConnectionsScreen, ListProps},
  mcp_form::{McpForm, McpFormProps},
};
use crate::{project::ShownProject, routes::ConnectionsView};

/// The page of the connection `view` names: GitHub's, an MCP server's, or a decision model's.
pub fn screen(ctx: &mut Ctx, project: &ShownProject, view: ConnectionsView) -> Element {
  if let Some(connection) = view.selected_decision() {
    return ctx.mount_keyed::<decision::DecisionPage>(
      &format!("decision-page-{}-{connection}", project.open.project.id()),
      decision::Props {
        project: project.clone(),
        connection,
      },
    );
  }
  let key = format!("connections-{}", project.open.project.id());
  ctx.mount_keyed::<ConnectionsScreen>(
    &key,
    ListProps {
      project: project.clone(),
      view,
    },
  )
}

/// Add MCP server.
pub fn new_mcp_server(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<McpForm>(
    &format!("connection-add-mcp-{}", project.open.project.id()),
    McpFormProps {
      project: project.clone(),
      server: None,
    },
  )
}

/// Edit MCP server.
pub fn edit_connection(ctx: &mut Ctx, project: &ShownProject, connection: ConnectionId) -> Element {
  ctx.mount_keyed::<McpForm>(
    &format!(
      "connection-edit-mcp-{}-{connection}",
      project.open.project.id()
    ),
    McpFormProps {
      project: project.clone(),
      server: Some(connection),
    },
  )
}
