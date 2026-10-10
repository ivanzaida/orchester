//! Number field with its unit and read-back (`Limit field` `UQFos`): a 200 wide box with the value in
//! Geist Mono and the unit after it, and under it, at its end, what the value means in words in 11
//! `text_muted`, 6 apart (`= 1.4M tokens · input + output`, `No limit · the run takes the time it
//! needs`). An empty value shows its placeholder (`No limit`) in the field's own type.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::input::FieldInput;
use crate::theme::ExtraSpacing;

/// The box's width (`eesXH` in `UQFos`).
const NUMBER_FIELD_WIDTH: f32 = 200.0;

pub struct NumberField {
  input: FieldInput,
  id: Arc<str>,
  read_back: Arc<str>,
  /// Why the value is not accepted, said in 11 `danger` in the read-back's place (`Aqupm` in `i4Y9hL`).
  error: Option<Arc<str>>,
  read_back_alignment: Alignment,
}

impl NumberField {
  /// A field typing a number of `unit` into `value`, named `name` for tools, saying `read_back` under
  /// it; its input's element id is `id`, the read-back's `<id>-read-back`.
  pub fn new(
    id: impl Into<Arc<str>>,
    name: impl Into<Arc<str>>,
    value: Signal<String>,
    unit: impl Into<Arc<str>>,
    read_back: impl Into<Arc<str>>,
  ) -> Self {
    let id = id.into();
    Self {
      input: FieldInput::new(id.clone(), name, value)
        .unit(unit)
        .mono()
        .width(Dimension::Px(NUMBER_FIELD_WIDTH)),
      id,
      read_back: read_back.into(),
      error: None,
      read_back_alignment: Alignment::End,
    }
  }

  /// What an empty field means, e.g. that there is no limit.
  pub fn placeholder(mut self, placeholder: impl Into<Arc<str>>) -> Self {
    self.input = self.input.placeholder(placeholder);
    self
  }

  /// A count of tokens: its digits are grouped with `separator` while they are typed.
  pub fn grouped(mut self, separator: impl Into<Arc<str>>) -> Self {
    self.input = self.input.grouped(separator);
    self
  }

  /// The value cannot be changed now ([`FieldInput::locked`]).
  pub fn locked(mut self) -> Self {
    self.input = self.input.locked();
    self
  }

  /// Why the value is not accepted, said in place of the read-back.
  pub fn error(mut self, error: impl Into<Arc<str>>) -> Self {
    self.error = Some(error.into());
    self.input = self.input.invalid();
    self
  }

  /// Use the containing form's control width.
  pub fn width(mut self, width: Dimension) -> Self {
    self.input = self.input.width(width);
    self
  }

  /// Align the read-back with the containing form's controls.
  pub fn read_back_alignment(mut self, alignment: Alignment) -> Self {
    self.read_back_alignment = alignment;
    self
  }

  pub fn on_leave(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.input = self.input.on_leave(handler);
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let read_back = match &self.error {
      Some(error) => Text::new(error)
        .id(format!("{}-note", self.id))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::Danger)
        .describe("role", "field-error"),
      None => Text::new(&self.read_back)
        .id(format!("{}-read-back", self.id))
        .variant(TypographyStyle::Caption),
    };
    Column::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(self.read_back_alignment)
      .child(self.input.render(ctx))
      .child(read_back)
      .into()
  }
}
