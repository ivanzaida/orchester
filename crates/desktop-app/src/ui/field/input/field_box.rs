//! The field's box and its input: the strokes, the parts around the value, and how typing and leaving
//! the field change it.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::{TextInputEvent, mouse_event::MouseEvent},
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text, TextInput, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::tokens::regroup;

use super::{COMPACT_AREA_HEIGHT, FieldInput, Lines, TEXT_AREA_MAX_LINES};
use crate::{
  theme::{ExtraSpacing, IconSize, SecretsText},
  ui::{
    field::field_error,
    icon::{Icon, icon},
    kbd::key_cap,
  },
};

pub(super) struct FieldBox {
  focused: Signal<bool>,
}

impl Component for FieldBox {
  type Props = FieldInput;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      focused: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FieldInput>().clone();
    let input: Element = if props.cut && props.lines == Lines::One {
      cut_value(&props).into()
    } else {
      self.input(ctx, &props).into()
    };
    let refused = props.error.is_some() || props.invalid;
    let stroke = if refused {
      PaletteColor::Danger
    } else if self.focused.get() {
      PaletteColor::BorderFocus
    } else if props.strong {
      PaletteColor::BorderStrong
    } else {
      PaletteColor::Border
    };
    let dress = |frame: Row| {
      frame
        .width(props.width)
        .background(if props.locked {
          PaletteColor::SurfacePanel
        } else {
          PaletteColor::SurfaceInput
        })
        .border(Border::inside(BorderSize::Sm, stroke.clone()))
        .rounded(RadiusSize::Md)
        .describe("state", if refused { "invalid" } else { "valid" })
        .describe("focused", if self.focused.get() { "true" } else { "false" })
        .describe("locked", if props.locked { "true" } else { "false" })
    };
    let frame = match props.lines {
      Lines::One => dress(
        Row::new()
          .id(format!("{}-box", props.id))
          .height(Dimension::Px(props.box_height))
          .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center),
      )
      .with_children(
        props
          .leading
          .map(|glyph| icon(glyph, IconSize::Base, PaletteColor::TextMuted).id(format!("{}-icon", props.id))),
      )
      .child(input)
      .with_children(clear_button(&props))
      .with_children(
        props
          .keys
          .as_deref()
          .filter(|_| props.value.get().is_empty())
          .map(|keys| key_cap(&format!("{}-keys", props.id), keys)),
      )
      .with_children(props.unit.as_deref().map(|unit| {
        Text::new(unit)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap()
      }))
      .with_children(props.trailing.map(|glyph| {
        icon(
          glyph,
          if props.masked { IconSize::Base } else { IconSize::Md },
          PaletteColor::TextMuted,
        )
        .id(format!("{}-trailing", props.id))
      })),
      Lines::Many | Lines::Compact => {
        dress(Row::new().id(format!("{}-box", props.id)).align_items(Alignment::Start)).child(input)
      }
    };
    let Some(error) = &props.error else {
      return Column::new().width(props.width).child(frame);
    };
    Column::new()
      .spacing(ExtraSpacing::Snug.role())
      .width(props.width)
      .child(frame)
      .child(field_error(&format!("{}-note", props.id), error))
  }
}

impl FieldBox {
  fn input(&self, ctx: &Ctx, props: &FieldInput) -> TextInput {
    let theme = ctx.theme();
    let resolve = |typography, color| FormTextRole { typography, color }.resolve(&theme.typography(), &theme.palette());
    // A masked value takes the credential's type unless its design names another (a recovery code's, `JQkW2`).
    let value = props.text_style.unwrap_or(if props.masked {
      SecretsText::Credential.role()
    } else if props.mono {
      TypographyStyle::Mono
    } else {
      TypographyStyle::Description
    });
    let (gained, lost) = (self.focused.clone(), self.focused.clone());
    let color = if props.locked {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::TextPrimary
    };
    let shown = resolve(value, color);
    let tracking = shown.letter_spacing;
    let mut input = TextInput::styled(props.value.clone(), shown)
      // A mono field's placeholder reads in its own type (`The worktree` in `M20DGB`).
      .placeholder_style(resolve(value, PaletteColor::TextMuted))
      // The input takes the role's tracking only on its own, after the styles (a two-factor code's 2, `JQkW2`).
      .letter_spacing(tracking)
      .caret_color(PaletteColor::TextPrimary)
      .id(props.id.to_string())
      .describe("a11y_name", props.name.clone())
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .on_focus(move || gained.set(true))
      .on_blur(leave_field(lost, props));
    if props.masked {
      input = input.mask();
    }
    if let Some(element) = &props.element {
      input = input.ref_element(element.clone());
    }
    if let Some(position) = props.tab_first {
      input = input.tab_index(position);
    }
    if let Some(placeholder) = &props.placeholder {
      input = input.placeholder(placeholder);
    }
    if props.locked {
      input = input
        .on_input(|event: TextInputEvent| event.prevent_default())
        .focusable(false);
    } else if props.group_separator.is_some() || props.on_typed.is_some() {
      let group = props
        .group_separator
        .clone()
        .map(|separator| group_while_typing(props.value.clone(), separator));
      let typed = props.on_typed.clone();
      input = input.on_input(move |event: TextInputEvent| {
        if let Some(typed) = &typed {
          typed(&event);
        }
        if let Some(group) = &group {
          group(event);
        }
      });
    }
    match props.lines {
      Lines::One => input.single_line(),
      Lines::Many | Lines::Compact => input
        .multiline()
        .rows(1, TEXT_AREA_MAX_LINES)
        // The input owns the whole hit area, including its empty body and text insets.
        .min_height(Dimension::Px(if props.lines == Lines::Compact {
          COMPACT_AREA_HEIGHT
        } else {
          props.area_height
        }))
        .padding(Padding::symmetric(
          ExtraSpacing::Loose.role(),
          ExtraSpacing::TextArea.role(),
        )),
    }
  }
}

/// A locked value as a text that ends in an ellipsis where the box is narrower, with the input's element id; the
/// placeholder while it is empty.
fn cut_value(props: &FieldInput) -> Text {
  let value = props.value.get();
  let style = props.text_style.unwrap_or(if props.mono {
    TypographyStyle::Mono
  } else {
    TypographyStyle::Description
  });
  let (text, color) = match props.placeholder.as_deref().filter(|_| value.is_empty()) {
    Some(placeholder) => (placeholder.to_owned(), PaletteColor::TextMuted),
    None => (value, PaletteColor::TextSecondary),
  };
  Text::new(&text)
    .id(props.id.to_string())
    .describe("a11y_name", props.name.clone())
    .variant(style)
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .width(Dimension::Px(0.0))
    .flex(1.0)
}

/// The cross that clears `props`' value, while it holds text and the field is clearable. A press clears the
/// value only: it does not also press a row around the field.
fn clear_button(props: &FieldInput) -> Option<Button> {
  let label = props.clear.clone()?;
  if props.value.get().is_empty() {
    return None;
  }
  let value = props.value.clone();
  Some(
    Button::empty()
      .id(format!("{}-clear", props.id))
      .align_items(Alignment::Center)
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .describe("a11y_name", label)
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        value.set(String::new());
      })
      .child(icon(Icon::X, IconSize::Base, PaletteColor::TextMuted)),
  )
}

/// Leaving the field ends its focus, for a whole number groups its digits once more, so that a value set
/// without typing (pasted all at once, or by a tool) reads like a typed one, and then commits an
/// autosaving field.
fn leave_field(focused: Signal<bool>, props: &FieldInput) -> impl Fn() + Send + Sync + 'static {
  let (value, separator, commit) = (
    props.value.clone(),
    props.group_separator.clone(),
    props.on_leave.clone(),
  );
  move || {
    focused.set(false);
    if let Some(separator) = &separator {
      let text = value.get_untracked();
      if let Some(grouped) = regroup(&text, separator).filter(|grouped| *grouped != text) {
        value.set(grouped);
      }
    }
    if let Some(commit) = &commit {
      commit();
    }
  }
}

/// Regroups a whole number as it is typed (`1400000` → `1,400,000`). Text that is not a whole number
/// stays as typed, so its error can name it.
fn group_while_typing(value: Signal<String>, separator: Arc<str>) -> impl Fn(TextInputEvent) + Send + Sync + 'static {
  move |event: TextInputEvent| {
    let Some(grouped) = regroup(event.new_value(), &separator) else {
      return;
    };
    if grouped != event.new_value() {
      event.prevent_default();
      value.set(grouped);
    }
  }
}
