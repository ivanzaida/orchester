//! A local MCP server's panel (`Agents · MCP servers` L2 `IpzZ7`, editing L8 `u6EkvN`, save refused L12 `uzwOD`): the
//! server's fields in one panel, every one the person's own and editable in place, the secret values its variables take
//! among them. A local MCP server has no versions: no draft, no Publish, no Versions. Nothing changes until *Save*,
//! which applies it to the project at once; *Cancel* puts back what was saved. Between the two the row and the panel
//! say *Unsaved changes*; a save that fails (a program that is not there, the secret store refusing a value) says why
//! in place, marks the field that failed, saves nothing, not even the other fields, and keeps everything typed. Archive
//! is in the head's … menu. While the project's connections are read for the first time the panel stands in bones.

mod actions;
mod editor;
mod fix;
mod form;
mod notes;
mod rows;
mod secrets;

use std::{
  collections::hash_map::DefaultHasher,
  hash::{Hash, Hasher},
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  components::Text,
  core::Signal,
  node::Element,
};
use orchester_core::id::ConnectionId;
use orchester_workspace::{connections::mcp::McpServerText, local_archive::Archivable};

use self::editor::EditorProps;
pub(super) use self::editor::UnsavedSignal;
pub(in crate::screens) use self::{
  form::Form,
  rows::{Look, switch_row},
  secrets::{Mark, Pending, Taken, rows_of},
};
use super::{
  model::Entry,
  panel::{Bone, BoneRow, Known, bones_with, chips_of},
  save_bar::{self, Save},
};
use crate::{project::ShownProject, queries, screens::connections::data, theme::HubText};

/// The ids of the panel's parts start with it.
const PREFIX: &str = "local-hub-mcp";
/// *Variables*: its field, 50 high, and *Add a variable from a secret…* under it.
const VARIABLES_AREA: f32 = 86.0;

/// *Server*'s rows of a program in bones, with their labels.
const BONES: [BoneRow; 6] = [
  BoneRow {
    label: "local_hub.mcp.runsAs",
    sub: None,
    bone: Bone::Field,
  },
  BoneRow {
    label: "connections.mcp.property.program",
    sub: None,
    bone: Bone::Field,
  },
  BoneRow {
    label: "connections.mcp.property.arguments",
    sub: None,
    bone: Bone::Field,
  },
  BoneRow {
    label: "connections.mcp.property.folder",
    sub: None,
    bone: Bone::Field,
  },
  BoneRow {
    label: "local_hub.mcp.tools",
    sub: None,
    bone: Bone::Field,
  },
  BoneRow {
    label: "connections.mcp.variables",
    sub: None,
    bone: Bone::Area(VARIABLES_AREA),
  },
];

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

/// A number that differs when the saved fields do.
fn saved_stamp(text: &McpServerText) -> u64 {
  let mut hasher = DefaultHasher::new();
  format!("{text:?}").hash(&mut hasher);
  hasher.finish()
}

/// The panel of the server while the project's connections are read for the first time: its head, chips and bar as
/// the hub knows them, and *Server*'s rows in bones, as a program's are laid out.
fn bones(ctx: &mut Ctx, props: &Props) -> Element {
  let entry = &props.entry;
  let known = Known {
    head: editor::head(ctx, props, None),
    chips: chips_of(notes::chips(ctx, entry, false), notes::used_by(ctx, entry)),
    bar: save_bar::bar(
      ctx,
      PREFIX,
      (Save::Unchanged, None),
      (ctx.t("local_hub.mcp.onlyHere").to_string(), PaletteColor::TextMuted),
    ),
  };
  let what = Archivable::McpServer(props.server);
  bones_with(ctx, what, PREFIX, ("connections.mcp.dialog.server", &BONES), known)
}

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
      return bones(ctx, &props);
    };
    let Some(server) = listed.mcp.iter().find(|server| server.id() == props.server).cloned() else {
      return Text::new(&ctx.t("local_hub.mcp.gone"))
        .id("local-hub-mcp-gone")
        .variant(HubText::Value.role())
        .into();
    };
    let text = McpServerText::of(&server, &listed.secrets);
    // The editor holds the fields as they were when it opened: a save opens it again at once, before the project's
    // connections are read again, so it opens once more on what the read brings.
    let key = format!(
      "local-hub-mcp-edit-{}-{}-{:x}",
      props.server,
      self.revision.get(),
      saved_stamp(&text)
    );
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
