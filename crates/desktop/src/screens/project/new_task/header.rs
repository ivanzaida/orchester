//! The form's topbar and heading: where the form is, what it holds and what it does.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::task::Task;

use super::{FormState, ProjectNav, edit};
use crate::{
  navigation::route::Page,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// Topbar height (`dWjfr`).
const TOPBAR_HEIGHT: f32 = 64.0;

/// Back to the task list, or to the task being edited, the breadcrumb and the persisted task state
/// (`dWjfr`, `kTm0n`): a draft being edited reads *Unsaved changes* once it differs from the
/// saved one.
pub(super) fn topbar(
  ctx: &Ctx,
  project: &str,
  nav: &ProjectNav,
  (form, editing, recovered): (&FormState, Option<&Task>, Option<&Task>),
) -> Element {
  let nav = nav.clone();
  let back_to = editing.or(recovered).map_or(Page::TASKS, |task| Page::task(task.id()));
  let back = pressable(surface().id("new-task-back"), move || nav.leave(back_to))
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .child(icon(Icon::ArrowLeft, IconSize::Option, PaletteColor::TextSecondary));
  let state = if form.starting {
    ctx.t("tasks.new.starting")
  } else if form.saving {
    ctx.t(if form.editing {
      "tasks.edit.savingChanges"
    } else {
      "tasks.new.saving"
    })
  } else if let Some(queued) = form.queued_task.as_ref() {
    ctx.t(queued.outcome.status_key())
  } else if form.saved {
    ctx.t(if form.started {
      "tasks.edit.saved"
    } else {
      "tasks.new.saved"
    })
  } else if form.editing {
    ctx.t("tasks.edit.unsaved")
  } else {
    ctx.t("tasks.new.notSaved")
  };
  let state = Text::new(&state).variant(TypographyStyle::Caption).id("new-task-state");
  // Changes to a saved draft that are not saved yet read in `$text` (`ctHXE`).
  let state = if form.editing && !form.saved && !form.saving && !form.starting && !form.queued {
    state.color(PaletteColor::TextPrimary)
  } else {
    state
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(back)
    .child(Text::new(&edit::breadcrumb(ctx, project, editing, recovered)).variant(TypographyStyle::Description))
    .child(Spacer::new().flex(1.0))
    .child(state)
    .into()
}

/// Heading (`hkWbp`; `h79DDd` while editing).
pub(super) fn heading(ctx: &Ctx, editing: Option<&Task>, state: &FormState) -> Element {
  let (title, description) = edit::heading_texts(ctx, editing);
  let description = if state.starting && state.queued_task.is_some() && !state.retrying {
    ctx.t("tasks.new.firstStartQueuedDescription")
  } else if state.recovered && state.run_problems.is_none() {
    ctx.t("tasks.new.recoveredDescription")
  } else if let Some(queued) = state.queued_task.as_ref() {
    ctx.t(queued.outcome.description_key())
  } else {
    description
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Section)
    .padding_horizontal(ExtraSpacing::Indent.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&title).variant(ExtraTypography::ScreenTitle.role()))
    .child(
      Text::new(&description)
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    )
    .into()
}
