//! Form field (`UOlUX` with `ZGwlv`): a label over a control, with an inline error or hint.
//! Inputs use lurq's form input roles, filled from the design in `theme::form`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, FormTextRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, SelectPartStyle, Style, border::Border, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};

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
/// the task list (`d5tAIZ`).
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
  let row = Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .height(height)
    .padding_custom(input.padding)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(background)
    .border(Border::inside(
      BorderSize::Sm,
      if tone == ValueTone::Invalid {
        PaletteColor::BorderStrong
      } else {
        input.border.clone()
      },
    ))
    .rounded(input.radius)
    .child(
      Text::new(text)
        .variant(input.text.typography)
        .color(color)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    );
  match choose {
    Some((glyph, on_click)) if tone != ValueTone::Unavailable => row
      .child(icon(glyph, IconSize::Button, PaletteColor::TextSecondary))
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, input.border_focus)))
      .on_click(move |_| on_click())
      .into(),
    _ if tone == ValueTone::Unavailable => row
      .describe("state", "unavailable")
      .cursor(CursorIcon::NotAllowed)
      .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
      .into(),
    _ => row.into(),
  }
}
