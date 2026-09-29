//! Left column of the new task form (`Eaadf`): project, title, brief, acceptance criteria and
//! context. Attaching context is not available yet.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{repository::RepositoryLink, task::MissingDetail};

use super::{FOCUS_ROOM, FormState, NewTaskForm, missing_note};
use crate::{
  display,
  theme::{DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{styled_input, text_input, wrapping_field_error},
    github_mark::github_mark,
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, keyboard_stop},
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
  hovered: Signal<bool>,
  remove_focused: Signal<bool>,
}

impl lurq::app::component::DevtoolsInspectable for Criterion {}

impl Criterion {
  pub fn new(id: u64) -> Self {
    Self {
      id,
      text: Signal::new(String::new()),
      hovered: Signal::new(false),
      remove_focused: Signal::new(false),
    }
  }
}

pub fn brief_form(
  ctx: &Ctx,
  form: &NewTaskForm,
  project: &OpenProject,
  link: Option<&RepositoryLink>,
  state: &FormState,
) -> Element {
  // `FOCUS_ROOM` on the left, where the viewport starts that much earlier (see `mod.rs`), so
  // focus rings of controls on the content's left edge are not cut by the viewport.
  let fields = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_left(FOCUS_ROOM)
    // Attach context is last; the room keeps its ring when the brief scrolls to the end.
    .padding_bottom(FOCUS_ROOM)
    .spacing(SpacingSize::Lg)
    .child(labelled(ctx, "tasks.new.project", repository(ctx, project, link)))
    .child(title_field(ctx, form, state))
    .child(brief_field(ctx, form, state))
    .child(criteria_field(ctx, form, state))
    .child(labelled(
      ctx,
      "tasks.new.context",
      ActionButton::new(ctx.t("tasks.new.attachContext"))
        .icon(Icon::Plus)
        .id("attach-context")
        .in_form()
        .height(Dimension::Px(CONTEXT_BUTTON_HEIGHT))
        .render(ctx),
    ))
    .child(preview_footer(ctx));
  ScrollVertical::new(fields)
    .id("new-task-brief")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .into()
}

fn preview_footer(ctx: &Ctx) -> Element {
  let preview = Button::empty()
    .button()
    .id("task-preview")
    .describe("state", "unavailable")
    .focusable(false)
    .cursor(CursorIcon::NotAllowed)
    .opacity(DISABLED_OPACITY)
    .child(
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("tasks.new.preview")).variant(TypographyStyle::Link))
        .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted)),
    );
  Row::new()
    .id("new-task-preview-footer")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("tasks.new.previewFooter"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .child(preview)
    .into()
}

/// A form label over its control (`xpbV5`).
fn labelled(ctx: &Ctx, label: &str, control: impl Into<Element>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    // A button control (Attach context) starts at the column's edge; its ring reaches past it.
    .overflow_visible()
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t(label)).variant(ExtraTypography::FormLabel.role()))
    .child(control)
}

/// The project's repository (`QLSCb`): the linked GitHub repository with its base branch
/// (`OORjS`, `F7okLt`), or the local folder while none is linked.
fn repository(ctx: &Ctx, project: &OpenProject, link: Option<&RepositoryLink>) -> Element {
  let row = Row::new()
    .id("new-task-repository")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(REPOSITORY_HEIGHT))
    .padding_horizontal(ExtraSpacing::Group.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md);
  let name = Text::new(project.project.name().as_str())
    .variant(ExtraTypography::FormLabel.role())
    .nowrap();
  let Some(link) = link else {
    return row
      .child(icon(Icon::FolderGit, IconSize::Notice, PaletteColor::TextPrimary))
      .child(name)
      .child(
        Text::new(&display::shorten_path(&project.repo_root, PATH_CHARS))
          .variant(TypographyStyle::Caption)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex(1.0),
      )
      .into();
  };
  let path = ctx.t_args(
    "tasks.new.repositoryPath",
    [("owner", link.repo().owner()), ("name", link.repo().name())],
  );
  row
    .child(github_mark(ctx, PaletteColor::TextPrimary))
    .child(name)
    .child(
      Text::new(&path)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .child(
      Text::new(link.default_branch().as_str())
        .id("new-task-base-branch")
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
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
    .with_children(error.map(|error| wrapping_field_error(&error)))
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
  // The link's padding keeps its focus ring off the text, and an equal shift back keeps the text
  // in line with the criterion markers (`Sc8e9` in `l2YA4`). The ring reaches into the room the
  // form's scroll content keeps on its left.
  let padding = ctx.theme().spacing_value(SpacingSize::Xs).resolve(0.0);
  let add = keyboard_stop(Button::empty().button(), Ring::Outer)
    .id("add-criterion")
    .padding_horizontal(SpacingSize::Xs)
    .offset(-padding, 0.0)
    .rounded(RadiusSize::Sm)
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
    // Add criterion may be last; its focus ring reaches past the column.
    .overflow_visible()
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
    .on_mouse_enter({
      let hovered = criterion.hovered.clone();
      move || hovered.set(true)
    })
    .on_mouse_leave({
      let hovered = criterion.hovered.clone();
      move || hovered.set(false)
    })
    .child(icon(Icon::Circle, IconSize::Marker, PaletteColor::TextMuted))
    .child(input);
  if !removable {
    return row.into();
  }
  let (criteria, id) = (criteria.clone(), criterion.id);
  let shown = criterion.hovered.get() || criterion.remove_focused.get();
  let focused = criterion.remove_focused.clone();
  let blurred = criterion.remove_focused.clone();
  row
    .child(
      keyboard_stop(Button::empty().button(), Ring::Inset)
        .id(Arc::<str>::from(format!("remove-criterion-{id}")))
        .align_items(Alignment::Start)
        .opacity(if shown { 1.0 } else { 0.0 })
        .cursor(CursorIcon::Pointer)
        .rounded(RadiusSize::Sm)
        .hovered_style(Style::new().background(PaletteColor::AccentMuted))
        .on_focus(move || focused.set(true))
        .on_blur(move || blurred.set(false))
        .on_click(move |_| criteria.update(|rows| rows.retain(|row| row.id != id)))
        .child(icon(Icon::X, IconSize::Dismiss, PaletteColor::TextSecondary)),
    )
    .into()
}
