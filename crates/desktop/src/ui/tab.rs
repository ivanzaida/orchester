//! Tab (`SOW6S`): a label with an optional count; the selected tab is underlined (`Iefue`).
//! A tab without a data source yet is unavailable: muted, locked and without a count.
//! [`tab_row`] lays tabs out over the hairline that ends their row. Tabs outside a form never take
//! focus: they are pointer-only (see `ui::pressable`). Tabs in a dialog are form controls
//! ([`Tab::in_form`]): Tab reaches the selected and the available ones, with the focus ring.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Stack, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{Ring, keyboard_stop, pressable, surface},
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
  in_form: bool,
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
      in_form: false,
    }
  }

  /// A form control of a dialog: a Tab stop unless unavailable, the selected tab included, so
  /// the focus stays on a tab that Enter or Space has just selected.
  pub fn in_form(mut self) -> Self {
    self.in_form = true;
    self
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

  /// Selects the tab. A selected tab selects nothing.
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
    let mut row = Row::new().height(Dimension::Pct(100.0));
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
    if tab.in_form && !matches!(tab.state, State::Unavailable) {
      return form_tab(tab.id, tab.height, row, tab.state);
    }
    match tab.state {
      State::Selected => row
        .id(tab.id)
        .height(tab.height)
        .border_bottom(Border::inside(BorderSize::Lg, PaletteColor::TextPrimary))
        .describe("state", "selected")
        .into(),
      // A button around the row, so a press on it keeps the focus where it is.
      State::Available(handler) => pressable(
        surface()
          .id(tab.id)
          .height(tab.height)
          .child(row.hovered_style(Style::new().border_bottom(Border::inside(BorderSize::Lg, PaletteColor::Border)))),
        move || handler(),
      )
      .describe("state", "enabled")
      .into(),
      State::Unavailable => row
        .id(tab.id)
        .height(tab.height)
        .describe("state", "unavailable")
        .cursor(CursorIcon::NotAllowed)
        .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
        .into(),
    }
  }
}

/// A tab of a dialog (see [`Tab::in_form`]): the same button whether selected or not, so the focus
/// stays on it when it becomes selected.
fn form_tab(id: &'static str, height: Dimension, row: Row, state: State) -> Element {
  let (row, described) = match &state {
    State::Selected => (
      row.border_bottom(Border::inside(BorderSize::Lg, PaletteColor::TextPrimary)),
      "selected",
    ),
    _ => (
      row.hovered_style(Style::new().border_bottom(Border::inside(BorderSize::Lg, PaletteColor::Border))),
      "enabled",
    ),
  };
  let button = keyboard_stop(surface().id(id).height(height).child(row), Ring::Outer).describe("state", described);
  match state {
    State::Available(handler) => button.cursor(CursorIcon::Pointer).on_click(move |_| handler()).into(),
    State::Selected | State::Unavailable => button.into(),
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
