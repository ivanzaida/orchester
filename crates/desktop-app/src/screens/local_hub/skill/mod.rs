//! The panel of a local skill (`Detail` in L1, `docs/design/team-configuration.md` rule 11), edited in place: the head
//! with *Versions* and the … menu, the chips, *Skill* (its name, where it is added, its description, its SKILL.md and
//! its files), *Ready?* and the pinned *Save* and *Cancel* ([`super::save_bar`]). What is typed is only the panel's
//! until *Save*, which publishes it as the next version at once through the Skills editor's checks
//! ([`orchester_workspace::skills::editor`]); *Cancel* puts back what is stored. Its reference files are listed but not
//! changed here, and where it is added was fixed when it was made.
//!
//! What the Skills editor checks a write against is read once for every skill the panel shows, through its query
//! (`queries::hub::skills`), kept for the session, and again when the skills, the profiles or the pipelines change (the
//! panel's own *Save* and *Cancel* record such a change): going from one skill to another, or coming back to the hub,
//! shows it at once. Only the first read of one shows the panel in bones. A draft stored before (by the Skills page)
//! opens as the panel's changes over the published version: *Save* publishes it, *Cancel* throws it away.

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
    hub::{self, SkillsRead},
  },
  theme::HubText,
  ui::{field::field_error, message::message_text},
};

/// The ids of the panel's parts start with it.
const PREFIX: &str = "local-hub-skill";
/// The description field is at least 64 high, as the form that makes one draws it (`Input` in A11 `p4aSf`).
const DESCRIPTION_AREA: f32 = 64.0;
/// SKILL.md's field is at least 180 high, as an instruction's text is (`Input` `sTxOp` in A10 `hGdGu`).
const BODY_AREA: f32 = 180.0;

/// The editor's rows in bones, with the labels and the words under them it has.
const BONES: [BoneRow; 5] = [
  BoneRow {
    label: "local_hub.skill.name",
    sub: Some("local_hub.skill.nameSub"),
    bone: Bone::Field,
  },
  BoneRow {
    label: "local_hub.skill.addedTo",
    sub: Some("local_hub.skill.addedToSub"),
    bone: Bone::Words,
  },
  BoneRow {
    label: "local_hub.skill.description",
    sub: Some("local_hub.skill.descriptionSub"),
    bone: Bone::Area(DESCRIPTION_AREA),
  },
  BoneRow {
    label: "local_hub.skill.body",
    sub: Some("local_hub.skill.bodySub"),
    bone: Bone::Area(BODY_AREA),
  },
  BoneRow {
    label: "local_hub.skill.files",
    sub: Some("local_hub.skill.filesSub"),
    bone: Bone::Words,
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

/// The panel of the skill `props.id`. One reader serves every skill of the hub's project; the editor under it is keyed
/// by the skill, so what is typed never carries over to another.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<Loader>("local-hub-skill-reader", props)
}

/// Reads what the Skills editor checks a write against (the project's skills, the scopes they can take) and shows the
/// editor over it.
struct Loader {
  /// The skill whose editor shows: it keeps what it typed and saved itself while the skills are read again.
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
    let query = ctx.query(hub::skills(session, props.project.open.repo_root.clone()));
    let read = queries::reading(&query);
    let (pending, rejected) = (read.is_pending(), read.is_rejected());
    // A skill opened while the read is under way again waits for it: the last answer may hold an older draft.
    let fresh = !pending || self.shown.get() == Some(props.id);
    let Some(SkillsRead { data, published }) = read.data.filter(|_| fresh) else {
      return match read.error.filter(|_| rejected) {
        Some(error) => field_error("local-hub-skill-unread", &message_text(ctx, &error)).into(),
        None => bones(
          ctx,
          (&props.project, &props.entry, props.id),
          PREFIX,
          ("local_hub.skill.title", &BONES),
        ),
      };
    };
    let Some(entry) = data.listed.entry(props.id).cloned() else {
      return Text::new(&ctx.t("local_hub.text.gone"))
        .id("local-hub-skill-gone")
        .variant(HubText::Value.role())
        .into();
    };
    let published = published
      .into_iter()
      .find_map(|(id, body)| (id == props.id).then_some(body))
      .unwrap_or_else(|| entry.body.clone());
    self.shown.set(Some(props.id));
    let key = format!("local-hub-skill-edit-{}", props.id);
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
