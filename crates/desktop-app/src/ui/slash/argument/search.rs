//! A picker's own search field (`Search` `pVdUg` in `P1` `QXPDY`, ORC-889): at the panel's top, 32 high, the UI kit's
//! search field with its 13 `search` glyph, the words in 12.5, and what the picker counts at its end in `text_muted`.
//! It holds the focus while the picker is open; its words filter the picker. It also keeps the focused row in view as
//! the keys move through a list that scrolls, and lets a press on a row pick it as Enter picks the focused one.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, events::mouse_event::MouseEvent},
  components::Row,
  core::{ElementRef, Signal},
  node::{CursorIcon, Element},
};

use crate::{
  theme::SlashText,
  ui::{
    field::{FieldInput, UnitLook},
    slash::SlashField,
  },
};

/// The field's height (`pVdUg`).
const SEARCH_HEIGHT: f32 = 32.0;

/// The words typed in the picker's search, the field that takes the focus, the row the picker keeps in view and the
/// composer's slash menu a pressed row is picked in.
#[derive(Clone)]
pub struct PickerSearch {
  words: Signal<String>,
  field: ElementRef,
  row: ElementRef,
  slash: SlashField,
}

impl PickerSearch {
  pub(super) fn new(slash: &SlashField, (field, row): (ElementRef, ElementRef)) -> Self {
    Self {
      words: slash.search.clone(),
      field,
      row,
      slash: slash.clone(),
    }
  }

  /// The field named `id`, saying `placeholder` while it is empty and `count` at its end.
  pub fn field(&self, ctx: &mut Ctx, id: &str, (placeholder, count): (Arc<str>, Option<Arc<str>>)) -> Element {
    let field = FieldInput::new(id, placeholder.clone(), self.words.clone())
      .search()
      .placeholder(placeholder)
      .text_style(SlashText::Search.role())
      .box_height(SEARCH_HEIGHT)
      .element(self.field.clone());
    match count {
      Some(count) => field.unit(count).unit_look(UnitLook::Count).render(ctx),
      None => field.render(ctx),
    }
  }

  /// `row`, the focused one: the picker scrolls it into view when the focus moves.
  pub fn keep_in_view(&self, row: Row) -> Row {
    row.ref_element(self.row.clone())
  }

  /// `row`, row `at` of the rows the picker offers: a press on it picks it and closes the picker, as Enter does.
  pub fn pickable(&self, row: Row, at: usize) -> Row {
    let slash = self.slash.clone();
    row
      .cursor(CursorIcon::Pointer)
      .on_click(move |_: MouseEvent| slash.pick(at))
  }
}
