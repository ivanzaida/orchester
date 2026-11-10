//! A text area in a row of a hub panel that edits in place (a local skill's *Description* and *SKILL.md*, a local
//! instruction's *Text*, a team skill's *Description*): its label at the top of the row's key, the words under it (or
//! why what is typed cannot be saved, in `danger`), and the area beside them, at least `height` high.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  core::Signal,
  layout::Alignment,
  node::dimension::Dimension,
};

use crate::{
  screens::team_hub::rows::{self, Sub},
  ui::field::FieldInput,
};

/// One area's row: its id, words and least height.
pub(in crate::screens) struct Area {
  /// The row's id; the area's is it with `-input` after it.
  pub id: String,
  pub label: Arc<str>,
  /// What the key says under the label while what is typed can be saved.
  pub sub: Arc<str>,
  pub placeholder: Arc<str>,
  pub height: f32,
}

impl Area {
  /// The row, typing into `value`; `error` says under the label why what is typed cannot be saved.
  pub(in crate::screens) fn row(self, ctx: &mut Ctx, value: &Signal<String>, error: Option<Arc<str>>) -> Row {
    let mut input = FieldInput::new(format!("{}-input", self.id), self.label.clone(), value.clone())
      .placeholder(self.placeholder)
      .width(Dimension::Pct(100.0))
      .text_area()
      .area_height(self.height);
    let sub = match error {
      Some(words) => {
        input = input.invalid();
        Sub {
          words,
          color: PaletteColor::Danger,
        }
      }
      None => Sub::quiet(self.sub),
    };
    rows::row(&self.id, &self.label, Some(sub), input.render(ctx)).align_items(Alignment::Start)
  }
}
