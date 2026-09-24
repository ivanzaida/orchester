//! Navigation item (`TSJpn`): icon, label and an optional count. An item is the selected
//! screen, a screen that can be opened, or visibly unavailable when the screen does not exist yet.
//! The selected item and the items that open a screen are Tab stops; Enter or Space opens one.

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{Ring, keyboard_stop, pressable, surface},
};
use crate::theme::{ExtraSpacing, ExtraTypography};

/// Item heights: primary navigation (`TSJpn`), setup steps (`aaeeO`), footer entries (`wqkkE`).
const ITEM_HEIGHT: f32 = 38.0;
const STEP_HEIGHT: f32 = 44.0;
const COMPACT_HEIGHT: f32 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Density {
  Primary,
  /// Setup steps: taller, smaller label.
  Step,
  /// Secondary entries such as All projects: smaller icon and label.
  Secondary,
  /// Footer entries such as Project settings.
  Compact,
}

type Handler = Arc<dyn Fn() + Send + Sync>;

enum State {
  /// `navigable` when the item also has a click handler: it stays a Tab stop while selected.
  Selected {
    navigable: bool,
  },
  Available(Handler),
  Unavailable,
}

pub struct NavItem {
  icon: Icon,
  label: Arc<str>,
  count: Option<usize>,
  state: State,
  density: Density,
  id: &'static str,
}

impl NavItem {
  /// An unavailable item unless marked `selected` or given `on_click`.
  pub fn new(id: &'static str, icon: Icon, label: Arc<str>) -> Self {
    Self {
      icon,
      label,
      count: None,
      state: State::Unavailable,
      density: Density::Primary,
      id,
    }
  }

  /// Opens the item's screen. A selected item opens nothing but stays a Tab stop.
  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.state = match self.state {
      State::Selected { .. } => State::Selected { navigable: true },
      _ => State::Available(Arc::new(handler)),
    };
    self
  }

  pub fn selected(mut self, selected: bool) -> Self {
    if selected {
      let navigable = matches!(self.state, State::Available(_));
      self.state = State::Selected { navigable };
    }
    self
  }

  /// Number of things in the screen, e.g. tasks (`LTXft`).
  pub fn count(mut self, count: Option<usize>) -> Self {
    self.count = count;
    self
  }

  pub fn step(mut self) -> Self {
    self.density = Density::Step;
    self
  }

  pub fn secondary(mut self) -> Self {
    self.density = Density::Secondary;
    self
  }

  pub fn compact(mut self) -> Self {
    self.density = Density::Compact;
    self
  }
}

impl From<NavItem> for Element {
  fn from(item: NavItem) -> Self {
    let color = match item.state {
      State::Selected { .. } => PaletteColor::TextPrimary,
      State::Available(_) => PaletteColor::TextSecondary,
      State::Unavailable => PaletteColor::TextMuted,
    };
    let (height, icon_size, label_style) = match item.density {
      Density::Primary => (ITEM_HEIGHT, IconSize::Nav, TypographyStyle::Body),
      Density::Step => (STEP_HEIGHT, IconSize::Nav, TypographyStyle::FieldLabel),
      Density::Secondary => (ITEM_HEIGHT, IconSize::Button, TypographyStyle::FieldLabel),
      Density::Compact => (COMPACT_HEIGHT, IconSize::Button, TypographyStyle::FieldLabel),
    };
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(height))
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .child(icon(item.icon, icon_size, color.clone()))
      .child(
        Text::new(&item.label)
          .variant(label_style)
          .color(color)
          .nowrap()
          .flex(1.0),
      )
      .with_children(
        item
          .count
          .map(|count| Text::new(&count.to_string()).variant(ExtraTypography::NavCount.role())),
      );
    // The selected and the available item are buttons around the row, with the row's shape so
    // the focus ring follows it; an unavailable item stays a plain row, so a press on it
    // clears the focus instead of keeping it elsewhere.
    let button = |row: Row| {
      surface()
        .id(item.id)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(height))
        .rounded(RadiusSize::Sm)
        .child(row)
    };
    match item.state {
      State::Selected { navigable: true } => {
        keyboard_stop(button(row.background(PaletteColor::AccentMuted)), Ring::Outer)
          .describe("state", "selected")
          .into()
      }
      // A selected setup step only shows progress; nothing opens from it.
      State::Selected { navigable: false } => row
        .id(item.id)
        .background(PaletteColor::AccentMuted)
        .describe("state", "selected")
        .into(),
      State::Available(handler) => pressable(
        button(row.hovered_style(Style::new().background(PaletteColor::AccentMuted))),
        Ring::Outer,
        move || handler(),
      )
      .describe("state", "enabled")
      .into(),
      State::Unavailable => row
        .id(item.id)
        .describe("state", "unavailable")
        .cursor(CursorIcon::NotAllowed)
        .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
        .into(),
    }
  }
}
