//! The tab list (state F `FtUhB`): once the tabs overflow, a button after the strip shows how many
//! tabs are open, and opens a menu in the select menu's style (`R4rtN1`, `u0ByM`) naming every tab
//! with its full location and a check at the active one. At most eight rows show, then it
//! scrolls. A click switches, a middle click closes; Escape or a click outside closes the list.

use lurq::{
  app::{
    ctx::Placement,
    events::{MouseButton, MouseEvent},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, ScrollVertical, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::super::{
  Tabs,
  label::{TabLabel, TabMark},
  model::TabId,
};
use crate::{
  theme::{ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography, select_menu_scrollbar},
  ui::icon::{Icon, IconSize, icon},
};

/// The button (`OZSnN`, 24 high) and the room its slot takes after the strip, its padding
/// included; the menu's width, row height, rows before it scrolls, and gap under the button.
const BUTTON_HEIGHT: f32 = 24.0;
pub const LIST_SLOT_WIDTH: f32 = 50.0;
const MENU_WIDTH: f32 = 320.0;
const ROW_HEIGHT: f32 = 32.0;
const VISIBLE_ROWS: f32 = 8.0;
const MENU_GAP: f32 = 6.0;

/// One row of the list.
pub struct ListRow {
  pub id: TabId,
  pub label: TabLabel,
  pub active: bool,
  pub attention: bool,
}

/// The button with the tab count; `open` while the list shows.
pub fn list_button(count: usize, open: &Signal<bool>, anchor: &ElementRef, enabled: bool) -> Element {
  let shown = open.get();
  let chevron = if shown { Icon::ChevronUp } else { Icon::ChevronDown };
  let mut button = Row::new()
    .id("window-tab-list")
    .height(Dimension::Px(BUTTON_HEIGHT))
    .padding_horizontal(ExtraSpacing::Tight.role())
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .ref_element(anchor.clone())
    .child(
      Text::new(&count.to_string())
        .variant(ExtraTypography::NavCount.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(icon(chevron, IconSize::Small, PaletteColor::TextPrimary));
  if shown {
    button = button.background(PaletteColor::AccentMuted);
  }
  if enabled {
    let toggle = open.clone();
    button = button
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_mouse_down(|event: MouseEvent| {
        event.prevent_default();
        event.stop_propagation();
      })
      .on_click(move |_: MouseEvent| toggle.update(|open| *open = !*open));
  }
  Row::new()
    .height(Dimension::Px(super::tab::TAB_HEIGHT))
    .padding_horizontal(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(button)
    .into()
}

/// The open list under its button, right-aligned with it.
pub fn list_menu(tabs: &Tabs, rows: &[ListRow], open: &Signal<bool>, anchor: &ElementRef) -> Element {
  let height = ROW_HEIGHT * (rows.len() as f32).min(VISIBLE_ROWS);
  let list = Column::new()
    .width(Dimension::Pct(100.0))
    .with_children(rows.iter().map(|row| list_row(tabs, row, open)));
  let menu = Column::new()
    .id("window-tab-list-menu")
    .width(Dimension::Px(MENU_WIDTH))
    .padding(ExtraSpacing::SelectMenu.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::SelectMenu.role())
    .box_shadow(ExtraShadow::Popover.role())
    .child(
      ScrollVertical::new(list)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(height))
        .scrollbar(select_menu_scrollbar()),
    );
  Popup::new(anchor.clone(), menu)
    .open(open.clone())
    .placement(Placement::BottomEnd)
    .offset(0.0, MENU_GAP)
    .into()
}

fn list_row(tabs: &Tabs, row: &ListRow, open: &Signal<bool>) -> Element {
  let (switch, close, tab) = (tabs.clone(), tabs.clone(), row.id);
  let closed = open.clone();
  let check =
    icon(Icon::Check, IconSize::Dismiss, PaletteColor::TextPrimary).opacity(if row.active { 1.0 } else { 0.0 });
  Row::new()
    .id(format!("window-tab-list-{tab}"))
    .describe("tab", row.label.full.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_click(move |_: MouseEvent| {
      closed.set(false);
      switch.activate(tab);
    })
    .on_mouse_click(MouseButton::Middle, move |_: MouseEvent| close.request_close(tab))
    .child(avatar(row))
    .child(
      Text::new(&row.label.full)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .child(check)
    .into()
}

/// The tab's avatar, as the tab shows it, with the attention dot ringed in the menu's fill.
fn avatar(row: &ListRow) -> Element {
  let TabMark::Project { initial } = &row.label.mark else {
    return icon(Icon::Folders, IconSize::Dismiss, PaletteColor::TextSecondary).into();
  };
  let dot_id = format!("window-tab-list-attention-{}", row.id);
  super::tab::avatar(
    initial,
    row.attention.then_some((dot_id.as_str(), PaletteColor::SurfaceRaised)),
  )
}

/// The rows of the tabs the strip labelled, in the strip's order.
pub fn rows(labels: &[super::Labeled], active: TabId) -> Vec<ListRow> {
  labels
    .iter()
    .map(|labeled| ListRow {
      id: labeled.id,
      label: labeled.label.clone(),
      active: labeled.id == active,
      attention: labeled.attention.is_some(),
    })
    .collect()
}
