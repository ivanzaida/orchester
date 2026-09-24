//! Left column of the new task form (`Eaadf`): project, title, brief, acceptance criteria and
//! context. Attaching context is not available yet.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::task::MissingDetail;

use super::{FormState, NewTaskForm, error_note, missing_note};
use crate::{
  display,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{styled_input, text_input},
    icon::{Icon, IconSize, icon},
    message_text,
  },
  workspace::OpenProject,
};

/// Repository box, title input and context button heights (`QLSCb`, `HWxFG`, `vKz1m`), and
/// the rows the brief grows between.
const REPOSITORY_HEIGHT: f32 = 48.0;
const TITLE_HEIGHT: f32 = 44.0;
const CONTEXT_BUTTON_HEIGHT: f32 = 32.0;
const BRIEF_ROWS: (usize, usize) = (3, 12);
/// Longest repository path shown in full.
const PATH_CHARS: usize = 64;

/// One acceptance criterion row. `id` keeps the row's input when rows above it are removed.
#[derive(Clone, Debug)]
pub struct Criterion {
  id: u64,
  pub text: Signal<String>,
}

impl lurq::app::component::DevtoolsInspectable for Criterion {}

impl Criterion {
  pub fn new(id: u64) -> Self {
    Self {
      id,
      text: Signal::new(String::new()),
    }
  }
}

pub fn brief_form(ctx: &Ctx, form: &NewTaskForm, project: &OpenProject, state: &FormState) -> Element {
  let fields = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(labelled(ctx, "tasks.new.project", repository(project)))
    .child(title_field(ctx, form, state))
    .child(brief_field(ctx, form, state))
    .child(criteria_field(ctx, form, state))
    .child(labelled(
      ctx,
      "tasks.new.context",
      ActionButton::new(ctx.t("tasks.new.attachContext"))
        .icon(Icon::Plus)
        .id("attach-context")
        .height(Dimension::Px(CONTEXT_BUTTON_HEIGHT))
        .render(ctx),
    ));
  ScrollVertical::new(fields)
    .id("new-task-brief")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .into()
}

/// A form label over its control (`xpbV5`).
fn labelled(ctx: &Ctx, label: &str, control: impl Into<Element>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t(label)).variant(ExtraTypography::FormLabel.role()))
    .child(control)
}

/// The project's repository (`QLSCb`). There is no GitHub connection, so it is the local folder.
fn repository(project: &OpenProject) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(REPOSITORY_HEIGHT))
    .padding_horizontal(ExtraSpacing::Group.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(Icon::FolderGit, IconSize::Notice, PaletteColor::TextPrimary))
    .child(
      Text::new(project.project.name().as_str())
        .variant(ExtraTypography::FormLabel.role())
        .nowrap(),
    )
    .child(
      Text::new(&display::shorten_path(&project.repo_root, PATH_CHARS))
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .into()
}

fn title_field(ctx: &Ctx, form: &NewTaskForm, state: &FormState) -> Element {
  // A blank title only keeps Save disabled; a title the rules reject says why.
  let error = state
    .errors
    .title
    .as_ref()
    .filter(|_| !form.title.get_untracked().trim().is_empty())
    .map(|error| message_text(ctx, error));
  let input = text_input(
    ctx,
    "task-title",
    form.title.clone(),
    error.is_some(),
    TypographyStyle::Body,
  )
  .height(Dimension::Px(TITLE_HEIGHT))
  .placeholder(&ctx.t("tasks.new.titlePlaceholder"));
  labelled(ctx, "tasks.new.taskTitle", input)
    .with_children(error.map(|error| error_note(&error)))
    .into()
}

/// The brief (`bGakk`): a growing text area with a hint below it.
fn brief_field(ctx: &Ctx, form: &NewTaskForm, state: &FormState) -> Element {
  let input = styled_input(ctx, form.brief.clone(), ExtraTypography::Brief.role())
    .id("task-brief")
    .textarea()
    .rows(BRIEF_ROWS.0, BRIEF_ROWS.1)
    .width(Dimension::Pct(100.0))
    .placeholder(&ctx.t("tasks.new.briefPlaceholder"));
  let area = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::HeadingBottom.role())
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Notice.role())
    .child(input)
    .child(
      Text::new(&ctx.t("tasks.new.briefHint"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    );
  labelled(ctx, "tasks.new.brief", area)
    .with_children(missing_note(ctx, state, MissingDetail::Brief))
    .into()
}

/// Acceptance criteria (`Sc7Ua`): one input per criterion, rows can be added and removed.
fn criteria_field(ctx: &Ctx, form: &NewTaskForm, state: &FormState) -> Element {
  let criteria = form.criteria.get();
  let removable = criteria.len() > 1;
  let rows: Vec<Element> = criteria
    .iter()
    .map(|criterion| criterion_row(ctx, &form.criteria, criterion, removable))
    .collect();
  let signal = form.criteria.clone();
  let add = Row::new()
    .id("add-criterion")
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_click(move |_| {
      signal.update(|criteria| {
        let next = criteria
          .iter()
          .map(|criterion| criterion.id + 1)
          .max()
          .unwrap_or_default();
        criteria.push(Criterion::new(next));
      });
    })
    .child(
      Text::new(&ctx.t("tasks.new.addCriterion"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("tasks.new.criteria")).variant(ExtraTypography::FormLabel.role()))
    .with_children(rows)
    .child(add)
    .with_children(missing_note(ctx, state, MissingDetail::AcceptanceCriteria))
    .into()
}

fn criterion_row(ctx: &Ctx, criteria: &Signal<Vec<Criterion>>, criterion: &Criterion, removable: bool) -> Element {
  let input = styled_input(ctx, criterion.text.clone(), TypographyStyle::FieldLabel)
    .id(Arc::<str>::from(format!("criterion-{}", criterion.id)))
    .single_line()
    .flex(1.0)
    .caret_color(PaletteColor::TextPrimary)
    .placeholder(&ctx.t("tasks.new.criterionPlaceholder"));
  let row = Row::new()
    .key(Arc::<str>::from(format!("criterion-{}", criterion.id)))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Circle, IconSize::Marker, PaletteColor::TextMuted))
    .child(input);
  if !removable {
    return row.into();
  }
  let (criteria, id) = (criteria.clone(), criterion.id);
  row
    .child(
      Row::new()
        .id(Arc::<str>::from(format!("remove-criterion-{id}")))
        .cursor(CursorIcon::Pointer)
        .rounded(RadiusSize::Sm)
        .hovered_style(Style::new().background(PaletteColor::AccentMuted))
        .on_click(move |_| criteria.update(|rows| rows.retain(|row| row.id != id)))
        .child(icon(Icon::X, IconSize::Dismiss, PaletteColor::TextSecondary)),
    )
    .into()
}
