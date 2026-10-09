//! Menus: a list of commands that opens anchored to the control that opens it and closes on a choice,
//! a press outside or Esc. Its panel is `surface_raised` inside a `border_strong` stroke, radius 8, 4
//! inside and 2 between rows (`Pipeline menu` `ClKqz`, `Tag menu` `LLIjG`). A row (`Menu item` `mAHGP`)
//! is 28 high, radius 4, 8 at its sides: a 14 `text_secondary` icon, the label in 12 `text_primary` and
//! a shortcut in Geist Mono 11 `text_muted` at its end, 8 apart; a row with a detail (`Menu item/detail`
//! `j6ZUhL`) is 6 over and under its label and the detail in 11 `text_muted` 2 under it, and a chosen
//! row ends in a 14 check. Rows take the `hover` fill under the pointer. A row that cannot be chosen
//! is faded, ends in a lock and says why as its detail. Groups are split by a 1 `border` line with 4
//! over and under it (`Menu separator` `UoDdI`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{inert, pressable},
};
use crate::theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

/// Space between a control and the menu it opens (`FpAul`).
const MENU_GAP: f32 = 6.0;

/// The panel of a menu with the element id `id`; add its rows as children.
pub fn menu_panel(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .padding(SpacingSize::Xs)
    .spacing(ExtraSpacing::Tight.role())
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Md)
}

/// `panel` opened under `anchor` at its start while `open` holds; a press outside or Esc closes it.
pub fn menu(anchor: ElementRef, open: Signal<bool>, panel: impl Into<Element>) -> Popup {
  Popup::new(anchor, panel)
    .open(open)
    .placement(Placement::BottomStart)
    .offset(0.0, MENU_GAP)
}

/// The line between groups of rows.
pub fn separator(ctx: &Ctx) -> Element {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Xs))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(line))
        .background(PaletteColor::Border),
    )
    .into()
}

enum Choice {
  Action(Arc<dyn Fn() + Send + Sync>),
  /// It cannot be chosen, and why when the caller says.
  Unavailable(Option<Arc<str>>),
}

pub struct MenuItem {
  id: Arc<str>,
  label: Arc<str>,
  icon: Option<Icon>,
  shortcut: Option<Arc<str>>,
  detail: Option<Arc<str>>,
  chosen: bool,
  choice: Choice,
}

impl MenuItem {
  /// A row saying `label`; until [`Self::on_click`] it cannot be chosen and shows the lock.
  pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      icon: None,
      shortcut: None,
      detail: None,
      chosen: false,
      choice: Choice::Unavailable(None),
    }
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// The keys that do the same, at the row's end.
  pub fn shortcut(mut self, keys: impl Into<Arc<str>>) -> Self {
    self.shortcut = Some(keys.into());
    self
  }

  /// What the row is, under its label.
  pub fn detail(mut self, detail: impl Into<Arc<str>>) -> Self {
    self.detail = Some(detail.into());
    self
  }

  /// The row is the current choice: a check at its end.
  pub fn chosen(mut self, chosen: bool) -> Self {
    self.chosen = chosen;
    self
  }

  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.choice = Choice::Action(Arc::new(handler));
    self
  }

  /// The row cannot be chosen now, for `reason`, which it says in place of its detail.
  pub fn unavailable(mut self, reason: impl Into<Arc<str>>) -> Self {
    self.choice = Choice::Unavailable(Some(reason.into()));
    self
  }

  fn text(&self, detail: Option<&str>) -> Column {
    Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Tight.role())
      .child(
        Text::new(&self.label)
          .id(format!("{}-label", self.id))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      )
      .with_children(detail.map(|detail| Text::new(detail).variant(TypographyStyle::Caption)))
  }

  fn trailing(&self) -> Option<Element> {
    match (&self.choice, self.chosen, &self.shortcut) {
      (Choice::Unavailable(_), ..) => Some(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted).into()),
      (_, true, _) => Some(icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary).into()),
      (_, false, Some(keys)) => Some(
        Text::new(keys)
          .variant(ExtraTypography::MonoNote.role())
          .nowrap()
          .into(),
      ),
      (_, false, None) => None,
    }
  }
}

impl From<MenuItem> for Element {
  fn from(item: MenuItem) -> Self {
    let detail = match &item.choice {
      Choice::Unavailable(Some(reason)) => Some(reason.clone()),
      _ => item.detail.clone(),
    };
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(ControlHeight::Md.value()))
      .padding(match detail {
        Some(_) => Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()),
        None => Padding::horizontal(SpacingSize::Sm),
      })
      .spacing(SpacingSize::Sm)
      .align_items(if detail.is_some() {
        Alignment::Start
      } else {
        Alignment::Center
      })
      .with_children(
        item
          .icon
          .map(|glyph| icon(glyph, IconSize::Md, PaletteColor::TextSecondary).id(format!("{}-icon", item.id))),
      )
      .child(item.text(detail.as_deref()))
      .with_children(item.trailing());
    let surface = |button: Button| {
      button
        .id(item.id.clone())
        .width(Dimension::Pct(100.0))
        .rounded(RadiusSize::Sm)
        .describe("chosen", if item.chosen { "true" } else { "false" })
        .child(row)
    };
    match item.choice {
      Choice::Action(handler) => surface(pressable(move || handler()))
        .describe("state", "enabled")
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .into(),
      Choice::Unavailable(_) => surface(inert(Button::empty()))
        .describe("state", "unavailable")
        .opacity(DISABLED_OPACITY)
        .into(),
    }
  }
}
