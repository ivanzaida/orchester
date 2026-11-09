//! A local MCP server's panel (`Agents · MCP servers` L2 `IpzZ7`, editing L8 `u6EkvN`, save refused L12 `uzwOD`): the
//! server's fields in one panel, every one the person's own and editable in place, the secret values its variables take
//! among them. A local MCP server has no versions: no draft, no Publish, no Versions. Nothing changes until *Save*,
//! which applies it to the project at once; *Cancel* puts back what was saved. Between the two the row and the panel
//! say *Unsaved changes*; a save that fails (a program that is not there, the secret store refusing a value) says why
//! in place, marks the field that failed, saves nothing, not even the other fields, and keeps everything typed. Archive
//! is in the head's … menu.

mod editor;
mod notes;
mod rows;
mod secrets;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Text,
  core::Signal,
  node::Element,
};
use orchester_core::id::ConnectionId;
use orchester_workspace::{connections::mcp::McpServerText, local_archive::Archivable};

use self::editor::EditorProps;
pub(super) use self::editor::UnsavedSignal;
use super::model::Entry;
use crate::{project::ShownProject, queries, screens::connections::data, theme::HubText, ui::state::loading};

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
  pub server: ConnectionId,
  pub unsaved: UnsavedSignal,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.entry == other.entry && self.server == other.server
  }
}
impl DevtoolsInspectable for Props {}

/// The panel of the local MCP server `props.server`.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("local-hub-mcp-{}", props.server);
  ctx.mount_keyed::<Loader>(&key, props)
}

/// Reads the project's connections (the server and the secrets its variables can take) and shows the editor over them.
struct Loader {
  /// Counts the saves and the cancels, each of which opens the editor again on what is saved.
  revision: Signal<u64>,
}

impl Component for Loader {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let query = data::load(ctx, props.project.open.repo_root.clone());
    let loaded = queries::state(&query);
    let Some(listed) = loaded.data else {
      return loading("local-hub-mcp-loading", &ctx.t("local_hub.reading")).into();
    };
    let Some(server) = listed.mcp.iter().find(|server| server.id() == props.server).cloned() else {
      return Text::new(&ctx.t("local_hub.mcp.gone"))
        .id("local-hub-mcp-gone")
        .variant(HubText::Value.role())
        .into();
    };
    let text = McpServerText::of(&server, &listed.secrets);
    let key = format!("local-hub-mcp-edit-{}-{}", props.server, self.revision.get());
    ctx.mount_keyed::<editor::Editor>(
      &key,
      EditorProps {
        props,
        server,
        text,
        listed,
        revision: self.revision.clone(),
      },
    )
  }
}
