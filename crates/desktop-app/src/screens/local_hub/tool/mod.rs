//! The panel of a local tool (`Detail` in L1, `docs/design/team-configuration.md` rule 11), edited in place as a team
//! admin edits one (A29 `FrraS`; L21 `ldRQO`, L24 `OSniP`): the head with *Versions* and the … menu, the chips, *Tool*
//! (the profiles that give it to their agents, then the form that makes one,
//! [`crate::screens::hub::forms::ToolFields`]: its kind, its description, and what it runs or calls), an HTTP or
//! database tool's *Access*, *Ready?* and the pinned *Save* and *Cancel*
//! ([`super::save_bar`]). What is typed is only the panel's until *Save*, which publishes it as the next version at
//! once ([`orchester_workspace::tools::local_edit`]); a change that widens what agents may run asks first, and only a
//! person answers. *Cancel* puts back what is published. Its name was fixed when it was made; its exit codes, output
//! limit and environment stay as the version has them.
//!
//! The project's tools are read once for every tool the panel shows, through their query (`queries::hub::tools`), kept
//! for the session, and again when the tools change (the panel's own *Save* records such a change). Only the first read
//! of one shows the panel in bones. The panel has no drafts (decided by the person 2026-11-09): a tool that has a
//! published version opens on it, unchanged, even when a draft of it is stored (an MCP client's), which *Save* replaces
//! with what it publishes and *Cancel* leaves alone.

mod editor;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Text,
  core::Ref,
  node::Element,
};
use orchester_core::id::ResourceId;

use self::editor::EditorProps;
use super::{
  mcp::UnsavedSignal,
  model::Entry,
  panel::{Bone, BoneRow, bones},
};
use crate::{
  project::ShownProject,
  queries::{
    self, Session,
    hub::{self, ToolsRead},
  },
  theme::HubText,
  ui::{field::field_error, message::message_text},
};

/// The ids of the panel's parts start with it.
const PREFIX: &str = "local-hub-tool";
/// The arguments' field is at least 64 high, as the form draws it.
const ARGUMENTS_AREA: f32 = 64.0;

/// The editor's rows in bones, with the labels and the words under them it has.
const BONES: [BoneRow; 6] = [
  BoneRow {
    label: "local_hub.given.usedBy",
    sub: Some("local_hub.given.usedBySub"),
    bone: Bone::Words,
  },
  BoneRow {
    label: "team_hub.tool.kindLabel",
    sub: Some("team_hub.tool.kindSub"),
    bone: Bone::Words,
  },
  BoneRow {
    label: "team_hub.plain.description",
    sub: None,
    bone: Bone::Field,
  },
  BoneRow {
    label: "team_hub.tool.program",
    sub: None,
    bone: Bone::Field,
  },
  BoneRow {
    label: "team_hub.tool.arguments",
    sub: Some("team_hub.tool.argumentsSub"),
    bone: Bone::Area(ARGUMENTS_AREA),
  },
  BoneRow {
    label: "team_hub.tool.timeoutField",
    sub: None,
    bone: Bone::Field,
  },
];

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
  pub id: ResourceId,
  /// What the list says of edits that are not saved, which the panel keeps.
  pub unsaved: UnsavedSignal,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.entry == other.entry && self.id == other.id
  }
}
impl DevtoolsInspectable for Props {}

/// The panel of the tool `props.id`. One reader serves every tool of the hub's project; the editor under it is keyed by
/// the tool, so what is typed never carries over to another.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<Loader>("local-hub-tool-reader", props)
}

/// Reads the project's tools and shows the editor of one over them.
struct Loader {
  /// The tool whose editor shows: it keeps what it typed and saved itself while the tools are read again.
  shown: Ref<Option<ResourceId>>,
}

impl Component for Loader {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      shown: ctx.create_ref(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let session = Session::of(ctx);
    let query = ctx.query(hub::tools(session, props.project.open.repo_root.clone()));
    let read = queries::reading(&query);
    let (pending, rejected) = (read.is_pending(), read.is_rejected());
    // A tool opened while the read is under way again waits for it: the last answer may hold an older version.
    let fresh = !pending || self.shown.get() == Some(props.id);
    let Some(ToolsRead { listed, published }) = read.data.filter(|_| fresh) else {
      return match read.error.filter(|_| rejected) {
        Some(error) => field_error("local-hub-tool-unread", &message_text(ctx, &error)).into(),
        None => bones(
          ctx,
          (&props.project, &props.entry, props.id),
          PREFIX,
          ("local_hub.tool.title", &BONES),
        ),
      };
    };
    let Some(entry) = listed.tool(props.id) else {
      return Text::new(&ctx.t("local_hub.text.gone"))
        .id("local-hub-tool-gone")
        .variant(HubText::Value.role())
        .into();
    };
    let published = published
      .into_iter()
      .find_map(|(id, body)| (id == props.id).then_some(body))
      .unwrap_or_else(|| entry.tool.clone());
    self.shown.set(Some(props.id));
    let key = format!("local-hub-tool-edit-{}", props.id);
    ctx.mount_keyed::<editor::Editor>(&key, EditorProps { props, published })
  }
}
