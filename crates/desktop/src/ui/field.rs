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
  node::{
    CursorIcon, Element, SelectCheckmarkPosition, SelectPartStyle, SelectStyle, Style, border::Border,
    dimension::Dimension, padding::Padding,
  },
};

use super::{
  icon::{Icon, IconSize, icon, select_icon},
  pressable::TAB_STOP,
};
use crate::theme::{ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography, SELECT_CHECK_SLOT, select_menu_scrollbar};

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

/// The slot that keeps a wrapping error's icon on its first line: one 11/1.5 line (`TGURL` in
/// 01.06 F `glurU`, `OG5pW` in 02.Q states `XIEn8`).
const ERROR_ICON_SLOT: f32 = 16.5;

/// [`field_error`] in a narrow column: the reason wraps within the column's width, and its icon
/// stays with the first line.
pub fn wrapping_field_error(text: &str) -> Element {
  let slot = Row::new()
    .height(Dimension::Px(ERROR_ICON_SLOT))
    .align_items(Alignment::Center)
    .child(icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextPrimary));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Start)
    .describe("role", "field-error")
    .child(slot)
    .child(Text::new(text).variant(ExtraTypography::WrappingError.role()).flex(1.0))
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

/// Height of an option in an open select menu (`L20XvS`).
pub(crate) const SELECT_OPTION_HEIGHT: f32 = 32.0;
/// Most an open select menu grows to, eight options and the border, before it scrolls (`g3poI`).
const SELECT_MENU_MAX_HEIGHT: f32 = 266.0;
/// Space between a select and its open menu (`g3poI`).
const SELECT_MENU_OFFSET: f32 = 4.0;

/// A select drawn like a field (`ZGwlv`) at `height`, with its open menu (`u0ByM`, options
/// `L20XvS`, states `goMne`). A fill means the pointer is on an option, a check marks the value,
/// and the keyboard highlight is the inset focus ring (`xURvJ`); they compose.
pub fn select_style(ctx: &Ctx, height: f32) -> SelectStyle {
  let theme = ctx.theme();
  let input = theme.form().input.clone();
  let hover = || SelectPartStyle::new().background(PaletteColor::AccentMuted);
  let ring = || SelectPartStyle::new().box_shadow(ExtraShadow::FocusRingInset.role());
  SelectStyle::unstyled()
    .trigger(select_trigger(ctx, height))
    .trigger_hovered(SelectPartStyle::new().border_inside(BorderSize::Sm, input.border_focus.clone()))
    .trigger_focused(select_trigger_focused(ctx))
    .trigger_open(select_trigger_focused(ctx))
    .chevron(select_icon(Icon::ChevronDown))
    .chevron_open(select_icon(Icon::ChevronUp))
    .chevron_color(PaletteColor::TextSecondary)
    .menu(
      SelectPartStyle::new()
        .background(PaletteColor::SurfaceRaised)
        .border_inside(BorderSize::Sm, input.border)
        .rounded(ExtraRadius::SelectMenu.role())
        .padding(ExtraSpacing::SelectMenu.role())
        .box_shadow(ExtraShadow::Popover.role()),
    )
    .menu_scrollbar(select_menu_scrollbar())
    .max_menu_height(SELECT_MENU_MAX_HEIGHT)
    .menu_gap(SELECT_MENU_OFFSET)
    .option(
      SelectPartStyle::new()
        .min_height(SELECT_OPTION_HEIGHT)
        .padding(Padding::symmetric(SpacingSize::Sm, 0.0))
        .rounded(input.radius)
        .text(input.text.resolve(&theme.typography(), &theme.palette())),
    )
    .option_hovered(hover())
    .option_selected_hovered(hover())
    .option_highlighted(ring())
    .option_selected_highlighted(ring())
    .option_disabled(SelectPartStyle::new().text_color(PaletteColor::TextMuted))
    .option_detail(
      SelectPartStyle::new()
        .typography(TypographyStyle::Caption)
        .text_color(PaletteColor::TextMuted),
    )
    .single_checkmark(true)
    .checkmark(select_icon(Icon::Check))
    .checkmark_color(PaletteColor::TextPrimary)
    .checkmark_size(SELECT_CHECK_SLOT)
    .checkmark_position(SelectCheckmarkPosition::Trailing)
    .checkmark_gap(SpacingSize::Sm)
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
  let trailing = choose.map_or(Trailing::None, |(glyph, on_click)| Trailing::Choose(glyph, on_click));
  let typography = ctx.theme().form().input.text.typography;
  value_box_parts(ctx, id, (text, typography), tone, trailing, height)
}

/// What a value box shows after its value.
enum Trailing {
  None,
  /// Opens a chooser; the icon says which.
  Choose(Icon, Box<dyn Fn() + Send + Sync>),
  /// Says what the value is.
  Marker(Icon),
}

/// `text` is drawn in `typography`, in the tone's color.
fn value_box_parts(
  ctx: &Ctx,
  id: &'static str,
  (text, typography): (&str, TypographyStyle),
  tone: ValueTone,
  trailing: Trailing,
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
    .variant(typography)
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex(1.0);
  match trailing {
    Trailing::Choose(glyph, on_click) if tone != ValueTone::Unavailable => Button::empty()
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
    trailing => {
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
      match (tone, trailing) {
        (ValueTone::Unavailable, _) => row
          .describe("state", "unavailable")
          .cursor(CursorIcon::NotAllowed)
          .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
          .into(),
        (_, Trailing::Marker(glyph)) => row
          .child(icon(glyph, IconSize::Dismiss, PaletteColor::TextSecondary))
          .into(),
        (_, Trailing::None | Trailing::Choose(..)) => row.into(),
      }
    }
  }
}

/// A read-only identifier in mono at `height`, e.g. the name of the skill being edited (`un4ZX`
/// in `ecJtg`).
pub fn mono_value_box(ctx: &Ctx, id: &'static str, text: &str, height: Dimension) -> Element {
  value_box_parts(
    ctx,
    id,
    (text, ExtraTypography::MonoValue.role()),
    ValueTone::Value,
    Trailing::None,
    height,
  )
}

/// A read-only path at `height` with an icon that says what the path is, e.g. the project
/// folder beside its own *Choose folder…* button (`xZAZc`, icon `eASPF`). A path is in mono
/// (`ObbY7` in `xZAZc`, `F5KwDR`); the placeholder is not (`bnN26`).
pub fn marked_value_box(
  ctx: &Ctx,
  id: &'static str,
  text: &str,
  (tone, glyph): (ValueTone, Icon),
  height: Dimension,
) -> Element {
  let typography = match tone {
    ValueTone::Value | ValueTone::Invalid => ExtraTypography::MonoField.role(),
    ValueTone::Placeholder | ValueTone::Unavailable => ctx.theme().form().input.text.typography,
  };
  value_box_parts(ctx, id, (text, typography), tone, Trailing::Marker(glyph), height)
}

/// `input` as a locked form shows it: its value stays as it is and it takes no focus.
pub fn locked_input(input: TextInput, locked: bool) -> TextInput {
  if locked {
    input
      .on_input(|event: lurq::app::events::TextInputEvent| event.prevent_default())
      .focusable(false)
  } else {
    input
  }
}
