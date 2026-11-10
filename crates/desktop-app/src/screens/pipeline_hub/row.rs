//! A row of the hub's list (`Item · Feature delivery` `UbEH3` in `Pipelines · v2` P1): the pipeline's `workflow` mark
//! in the color of how it stands, the name over its steps and tasks (or what the person has to do, in `warning`) and at
//! its end a *Default* tag on the pipeline New task offers first, then its version or *draft*. The chosen row stands
//! inert on `hover` with a 2 `accent` start; the others open the hub on them.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_workspace::readiness::Group;

use super::{list::Shown, model::Entry, words};
use crate::{
  routes::{PipelinesView, ProjectPage, Route},
  screens::hub::agent_row::{AgentRow, agent_row, tag_chip},
  theme::IconSize,
  ui::{
    icon::{Icon, icon},
    pressable::Press,
  },
};

pub(super) fn row(ctx: &Ctx, entry: &Entry, shown: &Shown<'_>) -> Element {
  let id = format!("pipeline-hub-row-{}", entry.id);
  let route = Route::Project(
    shown.project,
    ProjectPage::pipelines(PipelinesView::new(shown.filter, Some(entry.id))),
  );
  let said = words::line(ctx, shown.now, entry);
  agent_row(AgentRow {
    id: &id,
    chosen: shown.selected == Some(entry.id),
    open: Press::Open(shown.navigation.to(route)).button(),
    group: entry.group,
    kind: "pipeline".to_owned(),
    mark: icon(Icon::Workflow, IconSize::Lg, mark_color(entry)).into(),
    name: &entry.name,
    line: (&said.text, said.color),
    tag: (shown.default == Some(entry.id) && entry.archived.is_none())
      .then(|| tag_chip(&format!("{id}-tag"), &ctx.t("pipeline_hub.row.default"))),
    version: Some(words::version(ctx, entry).to_string()),
  })
}

/// The mark's color: `warning` for one that needs the person, `text_muted` for an archived one.
const fn mark_color(entry: &Entry) -> PaletteColor {
  if entry.archived.is_some() {
    return PaletteColor::TextMuted;
  }
  match entry.group {
    Group::NeedsYouHere => PaletteColor::Warning,
    Group::Ready | Group::OffHere => PaletteColor::TextSecondary,
  }
}
