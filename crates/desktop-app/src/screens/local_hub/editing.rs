//! The panel of an instruction, skill or tool, or of a profile that has only a draft, in a local project (`Detail` in
//! L1; `docs/design/team-configuration.md` rule 11): the head with its … menu, the chips (the version, how it stands),
//! *Ready?* and, pinned at the bottom, *Edit*, which opens the editor the kind has: the local draft, *Publish vN*,
//! *Versions* and the rest of its fields are the editor's. The team hub's panel of a resource without a sheet is the
//! same frame ([`crate::screens::team_hub::panel`]).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{id::ResourceId, resource::ResourceKind};
use orchester_workspace::readiness::Group;

use super::{
  archive,
  model::Entry,
  panel::{chips_of, frame, readiness},
  words::{kind_where_key, needs},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{AgentTab, InstructionsPanel, InstructionsTab, InstructionsView, ProjectPage, Route, ToolTab},
  screens::team_hub::{
    bar::bar,
    panel::{body, head_named},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    icon::Icon,
  },
};

/// The panel of `entry`.
pub(super) fn panel(ctx: &mut Ctx, project: &ShownProject, (entry, id): (&Entry, ResourceId)) -> Element {
  let navigation = Navigation::of(ctx);
  let more = archive::menu(
    ctx,
    archive::Target {
      root: project.open.repo_root.clone(),
      project: project.open.project.id(),
      what: entry.what,
      kind: entry.kind,
      name: Arc::from(entry.name.as_str()),
    },
  );
  let version = entry.number.map(|number| number.to_string());
  let body = body()
    .child(head_named(
      &entry.name,
      (ctx.t(kind_where_key(entry.kind)).to_string(), version),
      Some(more),
    ))
    .child(chips_of(chips(ctx, entry), None))
    .with_children(readiness(ctx, entry));
  let edit = ActionButton::new("local-hub-edit", ButtonKind::Primary, ctx.t("local_hub.panel.edit"))
    .icon(Icon::Pencil)
    .pinned()
    .opens(navigation.to(Route::Project(project.open.project.id(), editor(entry.kind, id))));
  frame(
    entry.what,
    body,
    Some(bar(
      vec![edit.into()],
      Some(ctx.t("local_hub.panel.editHint").to_string()),
    )),
  )
}

/// How the entry stands, as the chip after its version.
fn chips(ctx: &Ctx, entry: &Entry) -> Vec<(Arc<str>, BadgeTone)> {
  let mut chips = Vec::new();
  if let Some(number) = entry.number {
    chips.push((Arc::from(number.to_string()), BadgeTone::Neutral));
  }
  chips.push(match (entry.number, entry.group, entry.standing.as_ref()) {
    (None, ..) => (ctx.t("local_hub.chip.draft"), BadgeTone::Info),
    (Some(_), Group::NeedsYouHere, Some(standing)) => (needs(ctx, entry, standing), BadgeTone::Warning),
    _ => (ctx.t("local_hub.chip.ready"), BadgeTone::Success),
  });
  chips
}

/// The editor the kind has.
fn editor(kind: ResourceKind, id: ResourceId) -> ProjectPage {
  match kind {
    ResourceKind::AgentProfile => ProjectPage::Agent(id, AgentTab::default()),
    ResourceKind::Tool => ProjectPage::Tool(id, ToolTab::default()),
    ResourceKind::Instruction => ProjectPage::Instructions(InstructionsView::Rule(
      InstructionsTab::default(),
      Some(id),
      InstructionsPanel::Hidden,
    )),
    _ => ProjectPage::Skill(id),
  }
}
