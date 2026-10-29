//! Add MCP server and Edit MCP server (`Connections · Edit MCP server · kontur-docs · full page · dark`
//! `M20DGB`, remote fields and values it cannot save in `euVYY`): one page in the Connections tab with the
//! server's name, how it is reached (a program on this computer with its arguments, folder and variables,
//! or a remote server with its endpoint and bearer secret) and the tools that run without asking. Add is
//! the same page with empty fields. The page reads the project's connections first, then shows the editor
//! (`editor`) with the fields filled from them. The fields are checked and saved by `connections::mcp`, the
//! rules the MCP tools use; a save that widens what agents may do asks a person first (`widen`).

mod editor;
mod fields;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::id::ConnectionId;
use orchester_workspace::{connections::mcp::McpServerText, tasks::outside::Outside};

use self::editor::{EditorProps, McpEditor};
use super::data;
use crate::{
  project::ShownProject,
  routes::Screen,
  theme::ExtraSpacing,
  ui::{
    icon::Icon,
    message::message_text,
    skeleton::{self, Skeleton},
    state::{EmptyState, load_error},
  },
};

#[derive(Clone, PartialEq)]
pub struct McpFormProps {
  pub project: ShownProject,
  /// The server being edited; `None` adds one.
  pub server: Option<ConnectionId>,
}

impl DevtoolsInspectable for McpFormProps {}

pub struct McpForm {
  /// Counts Try again presses after reading the connections failed.
  revision: Signal<u64>,
}

impl Component for McpForm {
  type Props = McpFormProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<McpFormProps>().clone();
    // Read again when automation changed the project's connections: the fields keep what was typed, and a save
    // that widens asks against the server as it is now.
    let revision = self.revision.get() + props.project.feed.outside.get().of(Outside::Connections);
    let loaded = data::load(ctx, props.project.open.repo_root.clone(), revision);
    let screen = if props.server.is_some() {
      Screen::EditConnection
    } else {
      Screen::NewConnection
    };
    let Some(listed) = loaded.data else {
      let waiting = match loaded.error {
        Some(error) if !loaded.is_pending() => {
          let (body, revision) = (message_text(ctx, &error), self.revision.clone());
          load_error(
            ctx,
            "connection-form-load-error",
            (&ctx.t("connections.loadFailed"), &body),
            move || revision.update(|revision| *revision += 1),
          )
        }
        _ => {
          // No loading frame draws the server's form: its fields' bones stand in one outline until it is read.
          let bones = skeleton::list(
            ctx,
            "connection-form-rows",
            &[(90.0, 320.0), (120.0, 280.0), (80.0, 240.0)],
          );
          Skeleton::new("connection-form-loading", ctx.t("connections.reading")).show(ctx, bones)
        }
      };
      return framed(screen, waiting);
    };
    let stored = props.server.and_then(|id| listed.mcp_server(id).cloned());
    if props.server.is_some() && stored.is_none() {
      let missing = EmptyState::new(
        "connection-form-gone",
        Icon::SearchX,
        ctx.t("connections.mcp.dialog.goneTitle"),
        ctx.t("connections.mcp.dialog.goneDetail"),
      );
      return framed(
        screen,
        Column::new().padding(ExtraSpacing::Roomy.role()).child(missing).into(),
      );
    }
    let text = stored.as_ref().map_or_else(McpServerText::new_server, |server| {
      McpServerText::of(server, &listed.secrets)
    });
    let key = format!(
      "connection-mcp-editor-{}-{}",
      props.project.open.project.id(),
      props.server.map_or_else(String::new, |id| id.to_string())
    );
    ctx.mount_keyed::<McpEditor>(
      &key,
      EditorProps {
        project: props.project,
        stored,
        text,
        listed,
      },
    )
  }
}

/// `content` as the page while there is no form to show.
fn framed(screen: Screen, content: Element) -> Element {
  crate::screens::project_settings::kit::scroll(
    Column::new()
      .id(format!("screen-{}", screen.name()))
      .width(lurq::node::dimension::Dimension::Pct(100.0))
      .child(content),
  )
}
