//! Form field (`UOlUX` with `ZGwlv`): a label over a control, with an inline error or hint.
//! Inputs use lurq's form input roles, filled from the design in `theme::form`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, FormTextRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text, TextInput, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, SelectPartStyle, SelectStyle, Style, border::Border, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::TAB_STOP,
};
use crate::theme::SELECT_CHEVRON_SIZE;

/// Below the control: nothing, or the error of the current value.
pub enum FieldNote {
  None,
  Error(Arc<str>),
}

pub fn field(label: &str, control: Element, note: FieldNote) -> Element {
  let column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(label).variant(TypographyStyle::Caption))
    .child(control);
  match note {
    FieldNote::None => column.into(),
    FieldNote::Error(error) => column.child(field_error(&error)).into(),
  }
}

/// Why a value or an action was not accepted, under the field or action it is about.
pub fn field_error(text: &str) -> Element {
  Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .describe("role", "field-error")
    .child(icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextPrimary))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}

/// A single-line text input bound to `value` in `typography`; an `invalid` input gets a
/// stronger border. Callers adjust it further, e.g. its height or placeholder.
pub fn text_input(
  ctx: &Ctx,
  id: &'static str,
  value: Signal<String>,
  invalid: bool,
  typography: TypographyStyle,
) -> TextInput {
  let input = ctx.theme().form().input.clone();
  styled_input(ctx, value, typography)
    .id(id)
    .single_line()
    .width(Dimension::Pct(100.0))
    .height(input.height)
    .padding_custom(input.padding)
    .background(input.background)
    .caret_color(input.caret)
    .border(Border::inside(
      BorderSize::Sm,
      if invalid {
        PaletteColor::BorderStrong
      } else {
        input.border
      },
    ))
    .rounded(input.radius)
    .focused_style(Style::new().border(Border::inside(BorderSize::Md, input.border_focus)))
}

/// A text input without a frame, in `typography`: entered text in the primary color, the
/// placeholder in the secondary one. Callers give it its frame.
pub fn styled_input(ctx: &Ctx, value: Signal<String>, typography: TypographyStyle) -> TextInput {
  let theme = ctx.theme();
  let resolve = |color| FormTextRole { typography, color }.resolve(&theme.typography(), &theme.palette());
  TextInput::styled(value, resolve(PaletteColor::TextPrimary))
    .placeholder_style(resolve(PaletteColor::TextSecondary))
    .caret_color(PaletteColor::TextPrimary)
}

/// A select drawn like a field (`ZGwlv`) at `height`, with its open menu. The design has no
/// open-menu state, so the menu follows the field: raised fill, field border and radius, options
/// padded like the trigger in the field text. The chosen option and the pointer's option take the
/// hover fill; the keyboard's option also gets the focused field stroke (`o8Oug`).
pub fn select_style(ctx: &Ctx, height: f32) -> SelectStyle {
  let theme = ctx.theme();
  let input = theme.form().input.clone();
  let hover = || SelectPartStyle::new().background(PaletteColor::AccentMuted);
  SelectStyle::new()
    .trigger(select_trigger(ctx, height))
    .trigger_focused(select_trigger_focused(ctx))
    .chevron_color(theme.palette_color(PaletteColor::TextSecondary))
    .chevron_size(SELECT_CHEVRON_SIZE)
    .menu(
      SelectPartStyle::new()
        .background(PaletteColor::SurfaceRaised)
        .border_inside(BorderSize::Sm, input.border.clone())
        .rounded(input.radius),
    )
    .option(
      SelectPartStyle::new()
        .padding(input.padding)
        .text(input.text.resolve(&theme.typography(), &theme.palette())),
    )
    .option_hovered(hover().border_inside(BorderSize::Md, input.border_focus))
    .option_selected(hover())
    .option_selected_hovered(hover())
}

/// Trigger of a select drawn like a field (`ZGwlv`) at `height`.
pub fn select_trigger(ctx: &Ctx, height: f32) -> SelectPartStyle {
  let theme = ctx.theme();
  let input = theme.form().input.clone();
  SelectPartStyle::new()
    .background(input.background)
    .border_inside(BorderSize::Sm, input.border)
    .rounded(input.radius)
    .padding(input.padding)
    .min_height(height)
    .text(input.text.resolve(&theme.typography(), &theme.palette()))
}

/// Focused trigger of a select: the focused field stroke (`o8Oug`).
fn select_trigger_focused(ctx: &Ctx) -> SelectPartStyle {
  SelectPartStyle::new().border_inside(BorderSize::Md, ctx.theme().form().input.border_focus.clone())
}

/// How a value box reads.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueTone {
  /// A real value, e.g. the chosen repository folder.
  Value,
  /// Nothing chosen yet.
  Placeholder,
  /// A value with an error shown under the field; drawn with a stronger border.
  Invalid,
  /// The control depends on something this version cannot do; drawn like a disabled field.
  Unavailable,
}

/// Select field (`ZGwlv`): a value with a trailing icon. With `on_click` it opens a chooser
/// (the trailing icon says which); an unavailable box shows a lock instead.
pub fn value_box(
  ctx: &Ctx,
  id: &'static str,
  text: &str,
  tone: ValueTone,
  choose: Option<(Icon, Box<dyn Fn() + Send + Sync>)>,
) -> Element {
  let height = ctx.theme().form().input.height;
  value_box_sized(ctx, id, text, tone, choose, height)
}

/// [`value_box`] at a height other than the standard field height, e.g. the 36px filters of
/// the task list (`d5tAIZ`). A box with a chooser is a button: Tab reaches it, in a form or
/// not, Enter or Space opens the chooser, and it shows the focused field stroke (`o8Oug`)
/// while focused.
pub fn value_box_sized(
  ctx: &Ctx,
  id: &'static str,
  text: &str,
  tone: ValueTone,
  choose: Option<(Icon, Box<dyn Fn() + Send + Sync>)>,
  height: Dimension,
) -> Element {
  let input = ctx.theme().form().input.clone();
  let (background, color) = match tone {
    ValueTone::Value => (input.background, PaletteColor::TextPrimary),
    ValueTone::Placeholder => (input.background, PaletteColor::TextSecondary),
    ValueTone::Invalid => (input.background, PaletteColor::TextPrimary),
    ValueTone::Unavailable => (PaletteColor::SurfacePanel, PaletteColor::TextMuted),
  };
  let border = if tone == ValueTone::Invalid {
    PaletteColor::BorderStrong
  } else {
    input.border.clone()
  };
  let value = Text::new(text)
    .variant(input.text.typography)
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex(1.0);
  match choose {
    Some((glyph, on_click)) if tone != ValueTone::Unavailable => Button::empty()
      .button()
      .id(id)
      .width(Dimension::Pct(100.0))
      .height(height)
      .padding_custom(input.padding)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(background)
      .border(Border::inside(BorderSize::Sm, border))
      .rounded(input.radius)
      .child(value)
      .child(icon(glyph, IconSize::Button, PaletteColor::TextSecondary))
      .cursor(CursorIcon::Pointer)
      .tab_index(TAB_STOP)
      .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, input.border_focus.clone())))
      .focused_style(Style::new().border(Border::inside(BorderSize::Md, input.border_focus)))
      .on_click(move |_| on_click())
      .into(),
    _ => {
      let row = Row::new()
        .id(id)
        .width(Dimension::Pct(100.0))
        .height(height)
        .padding_custom(input.padding)
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .background(background)
        .border(Border::inside(BorderSize::Sm, border))
        .rounded(input.radius)
        .child(value);
      if tone == ValueTone::Unavailable {
        row
          .describe("state", "unavailable")
          .cursor(CursorIcon::NotAllowed)
          .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
          .into()
      } else {
        row.into()
      }
    }
  }
}
