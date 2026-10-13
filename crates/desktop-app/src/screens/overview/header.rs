//! The Overview's heading (`Header` `PCFyu`): the project's key over its name and what the page shows,
//! with Import resources, not built yet and so locked with why under it, and New task. A project without
//! tasks says what a task is, with the same actions and what is ready to run under them (`Empty project`
//! `h2DTY7`, its list in [`super::ready`]).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    state::EmptyState,
    unavailable::with_reason,
  },
};

pub(super) fn header(ctx: &mut Ctx, project: &ShownProject) -> Row {
  let id = project.open.project.id();
  let name = project.open.project.name().as_str();
  let eyebrow = ctx.t_args("overview.eyebrow", [("key", project.open.project.key().as_str())]);
  let description = ctx.t_args("overview.description", [("project", name)]);
  Row::new()
    .id("overview-header")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(
          Text::new(&eyebrow)
            .id("overview-eyebrow")
            .variant(ExtraTypography::MonoNote.role()),
        )
        .child(
          Text::new(name)
            .id("overview-title")
            .variant(ExtraTypography::Display.role()),
        )
        .child(
          Text::new(&description)
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(actions(ctx, id, "overview", true))
}

/// The explanation's measure in the empty project's card (`F7LdL`).
const EMPTY_MEASURE: f32 = 520.0;

/// No task yet: what a task is, the heading's actions and what is ready to run.
pub(super) fn empty_project(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let actions = actions(ctx, project.open.project.id(), "overview-empty", false);
  EmptyState::new(
    "overview-empty",
    Icon::ListPlus,
    ctx.t("overview.empty.title"),
    ctx.t("overview.empty.detail"),
  )
  .measure(EMPTY_MEASURE)
  .action(actions)
  .child(super::ready::ready(ctx, project))
  .into()
}

/// Import resources, locked with why under it, and New task, under the element ids `<id>-*`; New task
/// last in the heading, first in the empty project's card.
fn actions(ctx: &mut Ctx, project: ProjectId, id: &str, heading: bool) -> Column {
  let navigation = Navigation::of(ctx);
  let reason = ctx.t("overview.importReason");
  let import = ActionButton::new(
    format!("{id}-import"),
    ButtonKind::Secondary,
    ctx.t("overview.importResources"),
  )
  .icon(Icon::Download)
  .unavailable(reason.clone());
  let new_task = ActionButton::new(format!("{id}-new-task"), ButtonKind::Primary, ctx.t("overview.newTask"))
    .icon(Icon::Plus)
    .on_click(move || navigation.go(Route::Project(project, ProjectPage::NewTask)));
  let row = Row::new().spacing(SpacingSize::Sm).align_items(Alignment::Center);
  let row = if heading {
    row.child(import).child(new_task)
  } else {
    row.child(new_task).child(import)
  };
  with_reason(row, &format!("{id}-import-reason"), &reason)
}
