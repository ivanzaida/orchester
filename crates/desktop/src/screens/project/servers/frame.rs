//! The frame every Settings › Servers dialog shares (10.02 `BPzuT`, 10.04, 10.05, 10.08): a 600
//! wide panel with the title and its explanation over a hairline, the body, and the actions at the
//! right of a 76 high bar, with what went wrong at their left. Fields are a 12/500 label over a 40
//! high input, with a hint or an error under it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::{ActionButton, SKIPPED_BY_TAB},
    field::{text_input, wrapping_field_error},
    form::{Submit, form},
    icon::Icon,
  },
};

/// Dialog width, action bar, field and button heights (`BPzuT`, `AK2aQ`, `aHu5O`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const FIELD_HEIGHT: f32 = 40.0;
pub(super) const BUTTON_HEIGHT: f32 = 36.0;

/// What a dialog shows.
pub(super) struct Dialog {
  pub id: &'static str,
  pub title: Arc<str>,
  pub explanation: Option<Arc<str>>,
  pub body: Option<Element>,
  /// What went wrong, at the left of the actions.
  pub problem: Option<Arc<str>>,
  /// At the left of the bar instead of a problem, e.g. *Sign out* while choosing a password.
  pub leading: Option<Element>,
  pub actions: Vec<Element>,
  pub submit: Submit,
}

/// The dialog over its backdrop, as a form that submits with `dialog.submit`. `frame` is focused when
/// the dialog opens, so Tab starts at its first control.
pub(super) fn dialog(ctx: &mut Ctx, frame: &ElementRef, dialog: Dialog) -> Element {
  let header = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&dialog.title).variant(ExtraTypography::DialogTitle.role()))
    .with_children(dialog.explanation.map(|explanation| {
      Text::new(&explanation)
        .variant(ExtraTypography::Explanation.role())
        .width(Dimension::Pct(100.0))
    }));
  let body = dialog.body.map(|body| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Step.role())
      .padding_horizontal(ExtraSpacing::Rail.role())
      .spacing(SpacingSize::Md)
      .overflow_visible()
      .child(body)
  });
  let start: Element = match (dialog.problem, dialog.leading) {
    (Some(problem), _) => Column::new()
      .id(format!("{}-problem", dialog.id))
      .flex(1.0)
      .child(wrapping_field_error(&problem))
      .into(),
    (None, Some(leading)) => Row::new().flex(1.0).child(leading).into(),
    (None, None) => Spacer::new().flex(1.0).into(),
  };
  let actions = Row::new()
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(start)
    .with_children(dialog.actions);
  let panel = Column::new()
    .id(dialog.id)
    .ref_element(frame.clone())
    .focusable(true)
    .tab_index(SKIPPED_BY_TAB)
    .width(Dimension::Px(DIALOG_WIDTH))
    .background(ExtraColor::Surface.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .child(header)
    .with_children(body)
    .child(actions);
  backdrop(ctx, form(dialog.submit, panel))
}

/// A button of the action bar; without `run` it is unavailable.
pub(super) fn button(
  id: &'static str,
  label: Arc<str>,
  icon: Option<Icon>,
  run: Option<Arc<dyn Fn() + Send + Sync>>,
) -> ActionButton {
  let mut button = ActionButton::new(label)
    .id(id)
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  if let Some(icon) = icon {
    button = button.icon(icon);
  }
  match run {
    Some(run) => button.on_click(move || run()),
    None => button.on_click(|| {}).enabled(false),
  }
}

/// Cancel, which closes the dialog.
pub(super) fn cancel(ctx: &Ctx, id: &'static str, close: Arc<dyn Fn() + Send + Sync>) -> Element {
  button(id, ctx.t("common.cancel"), None, Some(close)).render(ctx)
}

/// A field: its 12/500 label over `control`, then `notes` (a hint, an error).
pub(super) fn field(label: &str, control: impl Into<Element>, notes: impl IntoIterator<Item = Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .overflow_visible()
    .child(Text::new(label).variant(ExtraTypography::FormLabel.role()))
    .child(control)
    .with_children(notes)
    .into()
}

/// A hint under a field (`IUyCh`): 10 muted at 1.5, wrapping.
pub(super) fn hint(text: &str) -> Element {
  Text::new(text)
    .variant(ExtraTypography::Hint.role())
    .width(Dimension::Pct(100.0))
    .into()
}

/// What is wrong with a field's value, under it.
pub(super) fn error(id: impl Into<Arc<str>>, text: &str) -> Element {
  Column::new()
    .id(id.into())
    .width(Dimension::Pct(100.0))
    .child(wrapping_field_error(text))
    .into()
}

/// A 40 high single-line input; `mono` for an address or a username.
pub(super) fn input(ctx: &Ctx, id: &'static str, value: &Signal<String>, mono: bool) -> TextInput {
  let typography = if mono {
    ExtraTypography::MonoField.role()
  } else {
    TypographyStyle::FieldLabel
  };
  text_input(ctx, id, value.clone(), false, typography).height(Dimension::Px(FIELD_HEIGHT))
}

/// A password input: masked on screen and in the MCP tree.
pub(super) fn password(ctx: &Ctx, id: &'static str, value: &Signal<String>) -> TextInput {
  input(ctx, id, value, false).mask()
}
