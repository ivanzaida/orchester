//! Tab (`SOW6S`): a label with an optional count; the selected tab is underlined (`Iefue`).
//! A tab without a data source yet is unavailable: muted, locked and without a count.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::ExtraSpacing;

type Handler = Arc<dyn Fn() + Send + Sync>;

enum State {
  Selected,
  Available(Handler),
  Unavailable,
}

pub struct Tab {
  id: &'static str,
  label: Arc<str>,
  count: Option<usize>,
  state: State,
  height: Dimension,
}

impl Tab {
  /// An unavailable tab unless marked `selected` or given `on_click`.
  pub fn new(id: &'static str, label: Arc<str>, height: Dimension) -> Self {
    Self {
      id,
      label,
      count: None,
      state: State::Unavailable,
      height,
    }
  }

  pub fn count(mut self, count: Option<usize>) -> Self {
    self.count = count;
    self
  }

  pub fn selected(mut self, selected: bool) -> Self {
    if selected {
      self.state = State::Selected;
    }
    self
  }

  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    if !matches!(self.state, State::Selected) {
      self.state = State::Available(Arc::new(handler));
    }
    self
  }
}

impl From<Tab> for Element {
  fn from(tab: Tab) -> Self {
    let color = match tab.state {
      State::Selected => PaletteColor::TextPrimary,
      State::Available(_) => PaletteColor::TextSecondary,
      State::Unavailable => PaletteColor::TextMuted,
    };
    let row = Row::new()
      .id(tab.id)
      .height(tab.height)
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&tab.label)
          .variant(TypographyStyle::FieldLabel)
          .color(color)
          .nowrap(),
      )
      .with_children(
        tab
          .count
          .map(|count| Text::new(&count.to_string()).variant(TypographyStyle::Mono)),
      );
    match tab.state {
      State::Selected => row
        .describe("state", "selected")
        .border_bottom(Border::inside(BorderSize::Lg, PaletteColor::TextPrimary))
        .into(),
      State::Available(handler) => row
        .describe("state", "enabled")
        .cursor(CursorIcon::Pointer)
        .hovered_style(Style::new().border_bottom(Border::inside(BorderSize::Lg, PaletteColor::Border)))
        .on_click(move |_| handler())
        .into(),
      State::Unavailable => row
        .describe("state", "unavailable")
        .cursor(CursorIcon::NotAllowed)
        .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
        .into(),
    }
  }
}
