//! The title and the outcome, typed on the page without boxes (`Title field` `klxTy`, `Outcome field` `KNpPG`): the
//! title in 24/600 with *Task title* muted while it is empty, which has the focus on a new task; the outcome in 14 at
//! 1.6 `text_secondary` with its placeholder, growing with its lines. A title the workspace refuses says why under it;
//! while the form is locked neither takes input.

use lurq::{
  app::{
    ctx::Ctx,
    events::TextInputEvent,
    theme::{FormTextRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, TextInput},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::state::View;
use crate::{
  theme::{ExtraSpacing, TaskFormText},
  ui::{field::field_error, message::message_text},
};

/// The most lines the outcome grows to before its lines scroll.
const OUTCOME_MAX_LINES: usize = 40;

/// The inputs the composer types into, and where the title is, to give it the focus.
pub struct BriefFields<'a> {
  pub title: &'a Signal<String>,
  pub brief: &'a Signal<String>,
  pub title_input: &'a ElementRef,
}

/// `typography` resolved as a text input draws it: its text in `color`, then its placeholder muted. New module and
/// Edit module type their name and description so too.
pub(in crate::screens) fn text_styles(
  ctx: &Ctx,
  typography: TypographyStyle,
  color: PaletteColor,
) -> [lurq::layout::text_style::TextStyle; 2] {
  let theme = ctx.theme();
  [color, PaletteColor::TextMuted]
    .map(|color| FormTextRole { typography, color }.resolve(&theme.typography(), &theme.palette()))
}

/// `input` that takes no input while the form is `locked`.
fn locking(input: TextInput, locked: bool) -> TextInput {
  if locked {
    input
      .on_input(|event: TextInputEvent| event.prevent_default())
      .focusable(false)
  } else {
    input
  }
}

pub fn title(ctx: &mut Ctx, view: &View, fields: &BriefFields<'_>) -> Element {
  let [text, placeholder] = text_styles(ctx, TaskFormText::Title.role(), PaletteColor::TextPrimary);
  let name = ctx.t("task_form.fields.title.label");
  let input = TextInput::styled(fields.title.clone(), text)
    .placeholder_style(placeholder)
    .placeholder(&ctx.t("task_form.fields.title.placeholder"))
    .id("task-form-title-input")
    .describe("a11y_name", name)
    .width(Dimension::Pct(100.0))
    .caret_color(PaletteColor::TextPrimary)
    .ref_element(fields.title_input.clone());
  // An empty title is said in the action bar; a title the workspace refuses, here.
  let error = view
    .title_error
    .as_ref()
    .filter(|_| !view.values.title.trim().is_empty())
    .map(|error| field_error("task-form-title-error", &message_text(ctx, error)));
  Column::new()
    .id("task-form-title-field")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .child(locking(input, view.locked()))
    .with_children(error)
    .into()
}

pub fn outcome(ctx: &mut Ctx, view: &View, fields: &BriefFields<'_>) -> Element {
  let [text, placeholder] = text_styles(ctx, TaskFormText::Outcome.role(), PaletteColor::TextSecondary);
  let name = ctx.t("task_form.fields.brief.label");
  let input = TextInput::styled(fields.brief.clone(), text)
    .placeholder_style(placeholder)
    .placeholder(&ctx.t("task_form.fields.brief.placeholder"))
    .id("task-form-brief-input")
    .describe("a11y_name", name)
    .textarea()
    .rows(1, OUTCOME_MAX_LINES)
    .width(Dimension::Pct(100.0))
    .caret_color(PaletteColor::TextPrimary);
  Column::new()
    .id("task-form-brief-field")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().bottom(SpacingSize::Sm))
    .child(locking(input, view.locked()))
    .into()
}
