//! Menus: a list of commands that opens anchored to the control that opens it and closes on a choice, a press outside,
//! Esc or Tab: lurq's `ui::Menu` in the design's look (`theme::kit::menu`), its items the kit's rows ([`MenuItem`]) as
//! `ui::MenuItem::content`. Its panel is `surface_raised` inside a `border_strong` stroke, radius 8, 4 inside and 2
//! between rows (`Pipeline menu` `ClKqz`, `Tag menu` `LLIjG`). Groups are split by a 1 `border` line with 4 over and
//! under it (`Menu separator` `UoDdI`). A foot, a heading or a field is an element of the app's among the entries.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
  ui::{self, MenuKeys, Recipe},
};

pub use self::item::MenuItem;
use crate::theme::{self, ExtraSpacing};

mod item;

/// What a menu holds, in order: an item, a line between groups, or an element of the app's (a foot, a heading, a
/// field), which is no item and closes nothing.
pub struct Entry(ui::MenuEntry);

impl From<ui::MenuItem> for Entry {
  fn from(item: ui::MenuItem) -> Self {
    Self(ui::MenuEntry::Item(item))
  }
}

impl From<Element> for Entry {
  fn from(element: Element) -> Self {
    Self(ui::MenuEntry::Element(element))
  }
}

impl From<Row> for Entry {
  fn from(row: Row) -> Self {
    Element::from(row).into()
  }
}

impl From<Column> for Entry {
  fn from(column: Column) -> Self {
    Element::from(column).into()
  }
}

impl From<ScrollVertical> for Entry {
  fn from(scroll: ScrollVertical) -> Self {
    Element::from(scroll).into()
  }
}

/// The line between groups of items (`Menu separator` `UoDdI`).
pub const fn separator() -> Entry {
  Entry(ui::MenuEntry::Separator)
}

/// What a menu's panel holds and how it looks, before [`menu`] hangs it from its control.
pub struct MenuPanel {
  id: Arc<str>,
  width: Option<f32>,
  max_height: Option<f32>,
  rules: Option<Recipe>,
  entries: Vec<ui::MenuEntry>,
  keys: Option<MenuKeys>,
}

impl MenuPanel {
  /// The panel is `width` wide, its rows as wide.
  #[must_use]
  pub const fn width(mut self, width: f32) -> Self {
    self.width = Some(width);
    self
  }

  /// The panel is at most `height` high: what it holds gives way, a section of its own that scrolls first.
  #[must_use]
  pub const fn max_height(mut self, height: f32) -> Self {
    self.max_height = Some(height);
    self
  }

  #[must_use]
  pub fn child(mut self, entry: impl Into<Entry>) -> Self {
    self.entries.push(entry.into().0);
    self
  }

  #[must_use]
  pub fn with_children(mut self, entries: impl IntoIterator<Item = impl Into<Entry>>) -> Self {
    self.entries.extend(entries.into_iter().map(|entry| entry.into().0));
    self
  }

  /// A section of the panel that scrolls past `max_height` (lurq's `Menu::scrolling`): its items are still the menu's,
  /// reached by the arrows and by typing, the one the keys reach scrolled into view. Its rows stand `gap` apart (the
  /// panel's 2, a list's own, or none): lurq lays a section's content out with no gap of its own, so a spacer stands
  /// between them.
  #[must_use]
  pub fn scrolling(
    mut self,
    entries: impl IntoIterator<Item = impl Into<Entry>>,
    (max_height, gap): (f32, Option<ExtraSpacing>),
  ) -> Self {
    let mut spaced = Vec::new();
    for entry in entries {
      if let Some(gap) = gap
        && !spaced.is_empty()
      {
        spaced.push(ui::MenuEntry::Element(row_gap(gap)));
      }
      spaced.push(entry.into().0);
    }
    self.entries.push(ui::MenuEntry::Scroll {
      entries: spaced,
      max_height,
    });
    self
  }

  /// The menu leaves the focus in the field `keys` was made with (lurq's `Menu::keeps_focus`): while it has the focus,
  /// Up and Down move the active item, Enter chooses it and Esc closes the menu; every other key is the field's.
  #[must_use]
  pub fn keeps_focus(mut self, keys: &MenuKeys) -> Self {
    self.keys = Some(keys.clone());
    self
  }

  /// lurq's menu of this panel, shown at `anchor` while `open` holds.
  fn menu(self, anchor: ElementRef, open: Signal<bool>) -> ui::Menu {
    self.fill(ui::Menu::new(open, anchor))
  }

  /// `menu` holding this panel's entries in its look.
  fn fill(self, menu: ui::Menu) -> ui::Menu {
    let mut menu = menu.id(self.id);
    for entry in self.entries {
      menu = menu.entry(entry);
    }
    if let Some(rules) = &self.rules {
      menu = menu.rules(rules);
    }
    if let Some(keys) = &self.keys {
      menu = menu.keeps_focus(keys);
    }
    if let Some(width) = self.width {
      menu = menu.style(ui::MenuPart::Panel, |style| style.width(width));
    }
    match self.max_height {
      Some(height) => menu.style(ui::MenuPart::Panel, |style| style.max_height(height)),
      None => menu,
    }
  }
}

/// The panel of a menu with the element id `id`; add its entries.
pub fn menu_panel(id: &str) -> MenuPanel {
  MenuPanel {
    id: Arc::from(id),
    width: None,
    max_height: None,
    rules: None,
    entries: Vec::new(),
    keys: None,
  }
}

/// The room between two rows of a section that scrolls ([`MenuPanel::scrolling`]).
fn row_gap(gap: ExtraSpacing) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(gap.value()))
    .into()
}

/// The panel of a filter's menu, `width` wide, with the element id `id` (`CLI menu` `tEymU`).
pub fn filter_menu_panel(id: &str, width: f32) -> MenuPanel {
  MenuPanel {
    rules: Some(theme::kit::menu::filter_panel()),
    ..menu_panel(id).width(width)
  }
}

/// `panel` opened 6 under `anchor` at its start while `open` holds (`FpAul`); a choice, a press outside, Esc or Tab
/// closes it.
pub fn menu(anchor: ElementRef, open: Signal<bool>, panel: MenuPanel) -> ui::Menu {
  panel
    .menu(anchor, open)
    .placement(Placement::BottomStart)
    .offset(ExtraSpacing::Snug.role())
}

/// `panel` opened 6 under `anchor` at its end while `open` holds, for a control at the end of a row (`Version
/// menu` `DqtY6` under its picker in `kadDh`); it closes as [`menu`] does.
pub fn menu_at_end(anchor: ElementRef, open: Signal<bool>, panel: MenuPanel) -> ui::Menu {
  panel
    .menu(anchor, open)
    .placement(Placement::BottomEnd)
    .offset(ExtraSpacing::Snug.role())
}

/// `panel` as the submenu of a row ([`MenuItem::opening`]), open while `open` holds, `gap` past the row's end with its
/// first row level with it: a choice in it closes it and the menus it hangs from.
pub fn submenu(anchor: ElementRef, open: Signal<bool>, gap: f32, panel: MenuPanel) -> ui::Menu {
  panel.menu(anchor, open).offset(gap)
}

/// `panel` with its corner at `point` of the window while `open` holds, as a context menu stands (a row's menu at the
/// pointer); `anchor` (the row) is where the focus goes back, and a press of it only closes the menu. It closes as
/// [`menu`] does.
pub fn menu_at(anchor: ElementRef, open: Signal<bool>, point: (f32, f32), panel: MenuPanel) -> ui::Menu {
  panel
    .menu(anchor, open)
    .at(Some(point))
    .offset(0.0)
    .switch_on_anchor_press(false)
}

/// `panel` drawn in place, for a popup of the kit's that places it (a property chip's popover, a popup placed by a
/// frame around its control): its items are still lurq's menu's, the arrows move between them, and a choice, or Tab,
/// sets `open` to `false`, which closes that popup.
pub fn inline_menu(open: Signal<bool>, panel: MenuPanel) -> Element {
  panel.menu(ElementRef::default(), open).inline(true).into()
}

/// `item` where it is no entry of a lurq menu: in a section of a menu that scrolls (lurq's menu does not scroll its
/// entries), or among the rows of a panel of the kit's own. A choice closes what `open` shows by itself.
pub fn standalone(item: ui::MenuItem, open: &Signal<bool>) -> Element {
  let open = open.clone();
  item.on_select(move || open.set(false)).into()
}

/// `panel` drawn in place and held open, for a popup of the kit's that shows it and whose rows keep their own keys and
/// focus in the composer's field (the slash and @ menus): lurq's menu cannot reach rows in a section of it that
/// scrolls.
pub fn held_panel(panel: MenuPanel) -> Element {
  panel
    .fill(ui::Menu::controlled(true, ElementRef::default(), |_| {}))
    .inline(true)
    .into()
}

/// The foot of a menu: a `border` line over words in 11 `text_muted`, 8 over, 4 under and 8 at their sides, which puts
/// them 12 from the menu's edge inside its padding (`Foot` `cJqb6` in T7 `gyG3z`, `Foot` in T4 `V26A3`).
pub fn foot(ctx: &Ctx, id: &str, words: &str) -> Element {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(line))
        .background(PaletteColor::Border),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(SpacingSize::Sm)
            .bottom(SpacingSize::Xs)
            .left(SpacingSize::Sm)
            .right(SpacingSize::Sm),
        )
        .child(
          Text::new(words)
            .id(id.to_owned())
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::MutedForeground)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// The line between groups of rows as an element, where it is no entry of a lurq menu: in a section of a menu that
/// scrolls, which holds elements and no entries (lurq's menu does not scroll its entries), or in a panel of the kit's
/// own. As [`separator`] draws it.
pub fn separator_line(ctx: &Ctx) -> Element {
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
