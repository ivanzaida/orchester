//! The parts of an admin's edit that its kinds' panels share (`Agents · v2` A27 `uVUZ4`, A28 `qfrbn`): a section of
//! text an agent reads (an instruction's *Text*, a skill's *SKILL.md*), its label marked *· changed* while it differs
//! from the version the edit is made on, over a mono text area as wide as the panel.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::dimension::Dimension,
};

use crate::{theme::HubText, ui::field::FieldInput};

/// A section of text: its id, label and least height.
pub(in crate::screens::team_hub) struct TextSection {
  pub id: &'static str,
  pub label: Arc<str>,
  pub placeholder: Arc<str>,
  pub height: f32,
}

impl TextSection {
  /// The section, typing into `value`; `changed` marks its label; `error` says under the area why what is typed cannot
  /// be saved.
  pub(in crate::screens::team_hub) fn render(
    self,
    ctx: &mut Ctx,
    value: &Signal<String>,
    (changed, error): (bool, Option<Arc<str>>),
  ) -> Column {
    let mut input = FieldInput::new(format!("{}-input", self.id), self.label.clone(), value.clone())
      .placeholder(self.placeholder)
      .width(Dimension::Pct(100.0))
      .mono()
      .text_area()
      .area_height(self.height);
    if let Some(error) = error {
      input = input.invalid().error(error);
    }
    let head = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&self.label)
          .id(format!("{}-label", self.id))
          .variant(HubText::PanelLabel.role())
          .nowrap(),
      )
      .with_children(changed.then(|| changed_mark(ctx, self.id)));
    Column::new()
      .id(self.id.to_owned())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(head)
      .child(input.render(ctx))
  }
}

/// *· changed* after a section's label.
fn changed_mark(ctx: &Ctx, id: &str) -> Text {
  Text::new(&ctx.t("team_hub.edit.changedMark"))
    .id(format!("{id}-changed"))
    .variant(HubText::PanelHint.role())
    .color(PaletteColor::Warning)
    .nowrap()
}
