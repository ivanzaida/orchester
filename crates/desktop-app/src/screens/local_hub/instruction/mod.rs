//! The panel of a local instruction (`Detail` in L1, `docs/design/team-configuration.md` rule 11), edited in place:
//! the head with *Versions* and the … menu, the chips, *Instruction* (where it is added, which agents get it, when, and
//! its text), *Ready?* and the pinned *Save* and *Cancel* ([`super::save_bar`]). What is typed is only the panel's
//! until *Save*, which publishes it as the next version at once; *Cancel* puts back what is stored. The fields and
//! their checks are the Instructions editor's ([`crate::screens::instructions`]). Its name and where it is added were
//! fixed when it was made.
//!
//! The project's instructions are read once for every instruction the panel shows, through their query
//! (`queries::hub::instructions`), kept for the session, and again when they, the profiles or the pipelines change (the
//! panel's own *Save* and *Cancel* record such a change): going from one instruction to another, or coming back to the
//! hub, shows it at once. Only the first read of one shows the panel in bones. A draft
//! stored before (by the Instructions page) opens as the panel's changes over the published version: *Save* publishes
//! it, *Cancel* throws it away.

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
use orchester_workspace::messages::Message;

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
    hub::{self, InstructionsRead},
  },
  theme::HubText,
  ui::{field::field_error, message::message_text},
};

/// The ids of the panel's parts start with it.
const PREFIX: &str = "local-hub-instruction";
/// The text field is at least 180 high, as the form that makes one draws it (`Input` `sTxOp` in A10 `hGdGu`).
const TEXT_AREA: f32 = 180.0;

/// The editor's rows in bones, with the labels and the words under them it has.
const BONES: [BoneRow; 4] = [
  BoneRow {
    label: "local_hub.instruction.addedTo",
    sub: Some("local_hub.instruction.addedToSub"),
    bone: Bone::Words,
  },
  BoneRow {
    label: "local_hub.instruction.appliesTo",
    sub: Some("local_hub.instruction.appliesToSub"),
    bone: Bone::Field,
  },
  BoneRow {
    label: "local_hub.instruction.when",
    sub: None,
    bone: Bone::Field,
  },
  BoneRow {
    label: "local_hub.instruction.text",
    sub: Some("local_hub.instruction.textSub"),
    bone: Bone::Area(TEXT_AREA),
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

/// The panel of the instruction `props.id`. One reader serves every instruction of the hub's project; the editor under
/// it is keyed by the instruction, so what is typed never carries over to another.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<Loader>("local-hub-instruction-reader", props)
}

/// Reads the project's instructions with the profiles and pipelines they can name, and shows the editor over them.
struct Loader {
  /// The instruction whose editor shows: it keeps what it typed and saved itself while the instructions are read
  /// again.
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
    let query = ctx.query(hub::instructions(session, props.project.open.repo_root.clone()));
    let read = queries::reading(&query);
    let (pending, rejected) = (read.is_pending(), read.is_rejected());
    // An instruction opened while the read is under way again waits for it: the last answer may hold an older draft.
    let fresh = !pending || self.shown.get() == Some(props.id);
    let Some(InstructionsRead { data, published }) = read.data.filter(|_| fresh) else {
      return match read.error.filter(|_| rejected) {
        Some(error) => failed(ctx, &error),
        None => bones(
          ctx,
          (&props.project, &props.entry, props.id),
          PREFIX,
          ("local_hub.instruction.title", &BONES),
        ),
      };
    };
    let Some(entry) = data.entry(props.id).cloned() else {
      return Text::new(&ctx.t("local_hub.text.gone"))
        .id("local-hub-instruction-gone")
        .variant(HubText::Value.role())
        .into();
    };
    let published = published
      .into_iter()
      .find_map(|(id, body)| (id == props.id).then_some(body))
      .unwrap_or_else(|| entry.body.clone());
    self.shown.set(Some(props.id));
    let key = format!("local-hub-instruction-edit-{}", props.id);
    ctx.mount_keyed::<editor::Editor>(
      &key,
      EditorProps {
        props,
        data,
        entry,
        published,
      },
    )
  }
}

/// Why the instructions could not be read.
fn failed(ctx: &Ctx, error: &Message) -> Element {
  field_error("local-hub-instruction-unread", &message_text(ctx, error)).into()
}
