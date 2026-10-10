//! The Brief section (`Section · Brief` `uuAN4`): the project the task goes to, its title, the outcome it
//! describes and its acceptance criteria (`criteria`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::workspace::OpenProject;

use super::{
  criteria::{Criterion, criteria_row},
  state::View,
};
use crate::{
  theme::{ExtraTypography, TaskFormRadius, TaskFormText},
  ui::{
    avatar::initials,
    field::FieldInput,
    form_section::{Label, Place, section, side_row, stacked_row},
    message::message_text,
    tooltip::Truncated,
  },
};

/// The project's tile (`Avatar` `cBsT5`), and the most room its name and folder take beside the row's
/// label: a longer folder is cut, whole in its tooltip.
const PROJECT_TILE: f32 = 20.0;
const PROJECT_MAX_WIDTH: f32 = 420.0;

/// The fields the section types into.
pub struct BriefFields<'a> {
  pub title: &'a Signal<String>,
  pub brief: &'a Signal<String>,
  pub criteria: &'a Signal<Vec<Criterion>>,
}

pub fn brief(ctx: &mut Ctx, view: &View, project: &OpenProject, fields: &BriefFields<'_>) -> Element {
  let note = view.queued().then(|| ctx.t("task_form.queued.lockedMeta"));
  let rows = vec![
    project_row(ctx, project),
    title_row(ctx, view, fields.title),
    outcome_row(ctx, view, fields.brief),
    criteria_row(ctx, view, fields.criteria),
  ];
  section(
    "task-form-brief",
    &ctx.t("task_form.sections.brief"),
    note.as_deref(),
    rows,
  )
  .into()
}

/// The project new tasks go to: its initial, name and folder.
fn project_row(ctx: &mut Ctx, project: &OpenProject) -> Element {
  let name = project.project.name().as_str();
  let tile = Row::new()
    .width(Dimension::Px(PROJECT_TILE))
    .height(Dimension::Px(PROJECT_TILE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(TaskFormRadius::ProjectTile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&initials(name)).variant(TaskFormText::ProjectInitial.role()));
  let folder = Truncated::new(
    "task-form-project-folder",
    project.repo_root.display().to_string(),
    ExtraTypography::MonoNote.role(),
  )
  .render(ctx);
  let control = Row::new()
    .id("task-form-project")
    .max_width(Dimension::Px(PROJECT_MAX_WIDTH))
    .flex_shrink(1.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(tile.flex_shrink(0.0))
    .child(Text::new(name).variant(TypographyStyle::Label).nowrap())
    .child(Row::new().flex(1.0).flex_shrink(1.0).child(folder));
  let label = Label::new("task-form-project", ctx.t("task_form.project.label"))
    .description(ctx.t("task_form.project.description"));
  side_row(label, control, Place::Within, false)
}

fn title_row(ctx: &mut Ctx, view: &View, title: &Signal<String>) -> Element {
  let name = ctx.t("task_form.fields.title.label");
  let mut input = FieldInput::new("task-form-title-input", name.clone(), title.clone())
    .placeholder(ctx.t("task_form.fields.title.placeholder"));
  // An empty title is said under the actions it holds back; a title the field refuses, here.
  if let Some(error) = view
    .title_error
    .as_ref()
    .filter(|_| !view.values.title.trim().is_empty())
  {
    input = input.error(message_text(ctx, error));
  }
  if view.locked() {
    input = input.locked();
  }
  stacked_row(
    Label::new("task-form-title-field", name),
    input.render(ctx),
    Place::Within,
  )
}

fn outcome_row(ctx: &mut Ctx, view: &View, brief: &Signal<String>) -> Element {
  let name = ctx.t("task_form.fields.brief.label");
  let mut input = FieldInput::new("task-form-brief-input", name.clone(), brief.clone())
    .text_area()
    .placeholder(ctx.t("task_form.fields.brief.placeholder"));
  if view.locked() {
    input = input.locked();
  }
  stacked_row(
    Label::new("task-form-brief-field", name),
    input.render(ctx),
    Place::Within,
  )
}
