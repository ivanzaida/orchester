//! What a record list says in place of records, centered in the list's room: on the task's tab a bare glyph over the
//! title, the explanation and the actions, in the middle of the list (`n3lxin` in `fQ33n`, `qLxGh` in `e8oWP4`);
//! on the project's page the empty state's glyph tile and a smaller title, under the filters (`X0l2I6` in `z4t51u`).

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, IconSize, MemorySpacing, MemoryText, SecretsSpacing},
  ui::{
    button::ActionButton,
    icon::{Icon, icon},
    state::glyph_tile,
  },
};

/// The widest the task tab's explanation runs (`M22vg`, `QtSG5`).
const TASK_TEXT_WIDTH: f32 = 460.0;

pub struct Centered {
  id: Arc<str>,
  glyph: Icon,
  title: Arc<str>,
  text: Arc<str>,
  note: Option<Arc<str>>,
  actions: Vec<ActionButton>,
  /// The project's page: the glyph tile and a smaller title; else the task's tab.
  page: bool,
}

impl Centered {
  pub fn new(id: impl Into<Arc<str>>, glyph: Icon, title: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      glyph,
      title: title.into(),
      text: text.into(),
      note: None,
      actions: Vec::new(),
      page: false,
    }
  }

  /// As the project's page draws it (`X0l2I6`).
  pub fn page(mut self, page: bool) -> Self {
    self.page = page;
    self
  }

  /// A line under the explanation (*1 match among deprecated records*, `rYmDP`).
  pub fn note(mut self, note: impl Into<Arc<str>>) -> Self {
    self.note = Some(note.into());
    self
  }

  pub fn action(mut self, action: ActionButton) -> Self {
    self.actions.push(action);
    self
  }
}

impl From<Centered> for Element {
  fn from(state: Centered) -> Self {
    let id = state.id.clone();
    let frame = Column::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .describe("state", "empty");
    let (frame, glyph, title, text) = if state.page {
      (
        frame
          .padding(
            Padding::new()
              .top(MemorySpacing::StateTop.role())
              .bottom(ExtraSpacing::Roomy.role()),
          )
          .spacing(SpacingSize::Sm),
        Element::from(glyph_tile(&id, state.glyph)),
        TypographyStyle::Label,
        TypographyStyle::Description,
      )
    } else {
      (
        frame
          .flex(1.0)
          .justify(Justify::Center)
          .padding(Padding::horizontal(SecretsSpacing::Page.role()))
          .spacing(ExtraSpacing::Loose.role()),
        Element::from(icon(state.glyph, IconSize::Xl, PaletteColor::TextMuted).id(format!("{id}-glyph"))),
        MemoryText::PanelTitle.role(),
        MemoryText::StateText.role(),
      )
    };
    let text_width = if state.page {
      Dimension::Auto
    } else {
      Dimension::Px(TASK_TEXT_WIDTH)
    };
    let line = |words: &str, variant| {
      Text::new(words)
        .max_width(text_width)
        .variant(variant)
        .text_align(TextAlign::Center)
    };
    let actions = (!state.actions.is_empty()).then(|| {
      let row = Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(state.actions);
      if state.page {
        row
      } else {
        row.padding(Padding::new().top(ExtraSpacing::Snug.role()))
      }
    });
    frame
      .child(glyph)
      .child(line(&state.title, title).id(format!("{id}-title")))
      .child(line(&state.text, text))
      .with_children(state.note.map(|note| line(&note, text)))
      .with_children(actions)
      .into()
  }
}
