//! Number field with its unit and read-back (`Limit field` `UQFos`): a 200 wide box with the value in
//! Geist Mono and the unit after it, and under it, at its end, what the value means in words in 11
//! `text_muted`, 6 apart (`= 1.4M tokens · input + output`, `No limit · the run takes the time it
//! needs`). An empty value shows its placeholder (`No limit`) in the field's own type.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
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
  refused: bool,
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
      refused: false,
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

  /// Why the value is not accepted, said in place of the read-back.
  pub fn error(mut self, error: impl Into<Arc<str>>) -> Self {
    self.refused = true;
    self.input = self.input.error(error);
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let read_back = (!self.refused).then(|| {
      Text::new(&self.read_back)
        .id(format!("{}-read-back", self.id))
        .variant(TypographyStyle::Caption)
    });
    Column::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::End)
      .child(self.input.render(ctx))
      .with_children(read_back)
      .into()
  }
}
