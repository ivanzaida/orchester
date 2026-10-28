//! The Configuration tab's sections (`Command` `dnWzu`, `Run` `SuKYS`, `Environment` `thyO1` in `04b` `nuoSy`): a
//! group's head over setting rows, each with its label and hint at the start and its control at the end — 320 for
//! text, 120 for a number — and a field's error under its control.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::messages::Message;

use crate::{
  screens::hub::parts::{boxed, field, group, head},
  theme::HubText,
  ui::{field::FieldInput, message::message_text},
};

/// A text control's width (`wFyZk`, `TJ4R0`, `kMWO4` in `VKr9s`).
const TEXT_WIDTH: f32 = 320.0;
/// A number's (`cHcdN` in `JZ9i9`).
const NUMBER_WIDTH: f32 = 120.0;

/// How a setting's control looks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Control {
  /// One line of text in plain type.
  Text,
  /// A path or a word in Geist Mono.
  Mono,
  /// Lines in Geist Mono, e.g. one argument per line.
  Lines,
  /// A number in Geist Mono, narrow.
  Number,
}

/// One setting: `id` names its input (`tool-<id>`) and its row (`tool-<id>-row`).
pub(super) struct Setting<'a> {
  pub id: &'a str,
  pub label: &'a str,
  pub hint: Option<&'a str>,
  pub control: Control,
  pub error: &'a Option<Message>,
  pub locked: bool,
}

/// A row with a value that is only read, e.g. a saved tool's name or the working directory.
pub(super) fn read_row(id: &str, label: &str, hint: Option<&str>, value: &str) -> Element {
  field(
    &format!("tool-{id}"),
    label,
    hint,
    Text::new(value)
      .id(format!("tool-{id}"))
      .variant(HubText::Value.role())
      .color(PaletteColor::TextSecondary)
      .nowrap(),
  )
  .into()
}

pub(super) fn setting(ctx: &mut Ctx, setting: &Setting<'_>, value: lurq::core::Signal<String>) -> Element {
  let id = format!("tool-{}", setting.id);
  let width = if setting.control == Control::Number {
    NUMBER_WIDTH
  } else {
    TEXT_WIDTH
  };
  let mut input = FieldInput::new(id.clone(), setting.label, value).width(Dimension::Px(width));
  input = match setting.control {
    Control::Text => input,
    Control::Mono | Control::Number => input.mono(),
    Control::Lines => input.compact_text_area().mono(),
  };
  if setting.locked {
    input = input.locked();
  }
  if setting.error.is_some() {
    input = input.invalid();
  }
  let control = Column::new()
    .width(Dimension::Px(width))
    .flex_shrink(0.0)
    .spacing(SpacingSize::Xs)
    .child(input.render(ctx))
    .with_children(setting.error.as_ref().map(|error| {
      Text::new(&message_text(ctx, error))
        .id(format!("{id}-note"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::Danger)
    }));
  field(&id, setting.label, setting.hint, control).into()
}

/// A section: its head in 12/600 `text_secondary` over its rows in one box.
pub(super) fn section(id: &str, title: &str, rows: Vec<Element>) -> Element {
  group(id, head(id, title, None, None), boxed(&format!("{id}-rows"), rows)).into()
}
