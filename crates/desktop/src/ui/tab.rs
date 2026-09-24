//! Tab (`SOW6S`): a label with an optional count; the selected tab is underlined (`Iefue`).
//! A tab without a data source yet is unavailable: muted, locked and without a count.
//! [`tab_row`] lays tabs out over the hairline that ends their row. Only an available tab
//! takes focus; Tab reaches it and Enter or Space selects it.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Stack, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{pressable, surface},
};
use crate::theme::{ExtraColor, ExtraSpacing};

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
  min_width: Option<Dimension>,
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
      min_width: None,
    }
  }

  /// The design's width for this tab; a longer label still widens it.
  pub fn min_width(mut self, width: Dimension) -> Self {
    self.min_width = Some(width);
    self
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
    let mut row = surface().id(tab.id).height(tab.height);
    if let Some(width) = tab.min_width {
      row = row.min_width(width);
    }
    let row = row
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
      State::Available(handler) => pressable(row, move || handler())
        .describe("state", "enabled")
        .hovered_style(Style::new().border_bottom(Border::inside(BorderSize::Lg, PaletteColor::Border)))
        .into(),
      State::Unavailable => row
        .describe("state", "unavailable")
        .cursor(CursorIcon::NotAllowed)
        .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
        .into(),
    }
  }
}

/// A row of tabs at `height`, `gap` apart and inset by `inset`, ended by a hairline (`XxplC`,
/// `i2uFe`). The hairline is drawn below the tabs, so a selected tab's 2px underline covers
/// it as in the design instead of being cut by it.
pub fn tab_row(
  height: Dimension,
  gap: SpacingSize,
  inset: Option<SpacingSize>,
  tabs: impl IntoIterator<Item = Tab>,
) -> Element {
  let hairline = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()));
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .spacing(gap)
    .with_children(tabs);
  if let Some(inset) = inset {
    row = row.padding_horizontal(inset);
  }
  Stack::new()
    .width(Dimension::Pct(100.0))
    .height(height)
    .child(hairline)
    .child(row)
    .into()
}
