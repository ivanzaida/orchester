//! Left column of the new task form (`Eaadf`): project, title, brief, acceptance criteria
//! ([`super::criteria`]) and context. Attaching context is not available yet.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{repository::RepositoryLink, task::MissingDetail};

use super::{FOCUS_ROOM, FormState, NewTaskForm, criteria::criteria_field, missing_note};
use crate::{
  display,
  theme::{DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{locked_input, styled_input, text_input, wrapping_field_error},
    github_mark::github_mark,
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

pub fn brief_form(
  ctx: &mut Ctx,
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
    .child(organization_fields(ctx, form, project, state.queued || state.starting))
    .child(labelled(
      ctx,
      "tasks.new.context",
      ActionButton::new(ctx.t("tasks.new.attachContext"))
        .icon(Icon::Plus)
        .id("attach-context")
        .in_form()
        .height(Dimension::Px(CONTEXT_BUTTON_HEIGHT))
        .render(ctx),
    ));
  // The footer stays at the column's foot under the fields, which scroll (`MoDET` after the
  // flexible space `i2EEy`).
  Column::new()
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(
      ScrollVertical::new(fields)
        .id("new-task-brief")
        .width(Dimension::Pct(100.0))
        .flex(1.0),
    )
    .child(preview_footer(ctx))
    .into()
}

/// What the task's prompt also includes, with *Preview* unavailable at the right (`MoDET`).
fn preview_footer(ctx: &Ctx) -> Element {
  let preview = Button::empty()
    .button()
    .id("task-preview")
    .describe("state", "unavailable")
    .focusable(false)
    .cursor(CursorIcon::NotAllowed)
    .opacity(DISABLED_OPACITY)
    .padding_vertical(SpacingSize::Xs)
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
    // In line with the fields, which keep this room for focus rings (see `brief_form`).
    .padding_left(FOCUS_ROOM)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("tasks.new.previewFooter"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .child(Spacer::new().flex(1.0))
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
  let input = locked_input(input, state.queued || state.starting);
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
  let input = locked_input(input, state.queued || state.starting);
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

/// Status, priority, tags and Blocked; locked with the rest of the form while the task is queued
/// or starting, since Retry run starts the task as stored.
fn organization_fields(ctx: &mut Ctx, form: &NewTaskForm, project: &OpenProject, locked: bool) -> Element {
  let task = ctx
    .props::<super::NewTaskProps>()
    .editing
    .as_ref()
    .map(|editing| editing.task.id());
  ctx.mount_keyed::<super::super::organization_fields::OrganizationFields>(
    "task-organization-fields",
    super::super::organization_fields::OrganizationFieldsProps {
      draft: form.organization.clone(),
      root: project.repo_root.clone(),
      task,
      locked,
    },
  )
}
