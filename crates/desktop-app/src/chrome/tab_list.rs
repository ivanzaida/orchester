//! The tab list: while some tabs do not fit the strip (`tab_fit`), a chevron after the last tab that
//! shows opens a menu of every open tab in order, each with its icon, label and mark, the active one
//! filled. A click on a row switches to its tab, which then shows in the strip; its × or a middle click
//! closes it and the list stays open. The chevron is a Tab stop; with the list open, ArrowUp and
//! ArrowDown move a ring between rows (Home and End to the ends), Enter or Space switches to the ringed
//! tab, and Esc closes the list. A choice or Esc gives the focus back to the chevron.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::{KeyboardEvent, MouseButton, MouseEvent},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, ScrollVertical, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::layout_kind::Justify,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  tab_fit::CHEVRON_WIDTH,
  tab_label::{attention, close_button, label},
  tab_strip::TAB_HEIGHT,
};
use crate::{
  navigation::{
    Navigation,
    tabs::{Tab, TabId, TabSet},
  },
  project::Projects,
  theme::{ControlHeight, ExtraColor, ExtraShadow, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon, screen_icon},
    menu::menu_panel,
    pressable::{Focus, in_title_bar, pressable},
  },
};

/// The list's width.
const LIST_WIDTH: f32 = 320.0;
/// Eight 28 high rows and the 2 between them; more rows scroll.
const LIST_MAX_HEIGHT: f32 = 8.0 * 28.0 + 7.0 * 2.0;

pub struct TabList {
  open: Signal<bool>,
  /// The tab the keyboard ringed; none until an arrow key moves from the active tab.
  cursor: Signal<Option<TabId>>,
  chevron: ElementRef,
  /// The ringed row, or the active one, kept in view while the list scrolls.
  kept_row: ElementRef,
  /// The list closed by a choice or Esc, so the chevron takes the focus back.
  refocus: Arc<AtomicBool>,
}

impl Component for TabList {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      cursor: ctx.signal(None),
      chevron: ctx.element_ref(),
      kept_row: ctx.element_ref(),
      refocus: Arc::new(AtomicBool::new(false)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    if self.refocus.swap(false, Ordering::Relaxed) {
      ctx.focus(&self.chevron);
    }
    let navigation = Navigation::of(ctx);
    let open = self.open.get();
    let mut root = Row::new()
      .id("tab-list")
      .on_key_down(self.keys(navigation.clone()))
      .child(self.chevron(ctx, open));
    if open {
      ctx.scroll_into_view(&self.kept_row);
      let panel = self.panel(ctx, &navigation);
      root = root.child(
        Popup::new(self.chevron.clone(), panel)
          .open(self.open.clone())
          .placement(Placement::BottomEnd)
          .offset(0.0, ExtraSpacing::MenuOffset.value()),
      );
    }
    root
  }
}

impl TabList {
  /// The chevron (`chevron-down`, `chevron-up` while the list is open, filled like a hovered tab).
  fn chevron(&self, ctx: &mut Ctx, open: bool) -> Element {
    let (toggle, cursor) = (self.open.clone(), self.cursor.clone());
    let button = in_title_bar(Focus::Keyboard.apply(pressable(move || {
      cursor.set(None);
      toggle.update(|open| *open = !*open);
    })))
    .id("tab-list-button")
    .ref_element(self.chevron.clone())
    .describe("a11y_name", ctx.t("shell.tabs.list"))
    .describe("state", if open { "open" } else { "closed" })
    .width(Dimension::Px(CHEVRON_WIDTH))
    .height(Dimension::Px(TAB_HEIGHT))
    .flex_shrink(0.0)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(
      if open { Icon::ChevronUp } else { Icon::ChevronDown },
      IconSize::Md,
      PaletteColor::TextMuted,
    ));
    if open {
      button.background(ExtraColor::Hover.role()).into()
    } else {
      button.into()
    }
  }

  fn panel(&self, ctx: &mut Ctx, navigation: &Navigation) -> Column {
    let projects = Projects::of(ctx);
    let tabs = navigation.tabs();
    let ringed = self
      .cursor
      .get()
      .filter(|id| tabs.tabs().iter().any(|tab| tab.id() == *id));
    let ringed_any = ringed.is_some();
    let rows = tabs.tabs().iter().fold(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role()),
      |rows, tab| {
        let (active, ringed) = (tab.id() == tabs.active().id(), ringed == Some(tab.id()));
        let row = Entry {
          tab,
          active,
          ringed,
          closable: tabs.can_close(tab.id()),
          kept_in_view: if ringed_any { ringed } else { active },
        };
        rows.child(self.row(ctx, (navigation, &projects), &row))
      },
    );
    menu_panel("tab-list-menu")
      .width(Dimension::Px(LIST_WIDTH))
      .child(ScrollVertical::new(rows).max_height(Dimension::Px(LIST_MAX_HEIGHT)))
  }

  /// A tab's row (`Menu item` `mAHGP` with the tab's icon, label and mark, and its ×).
  fn row(&self, ctx: &mut Ctx, (navigation, projects): (&Navigation, &Projects), row: &Entry<'_>) -> Element {
    let id = row.tab.id();
    let text = label(ctx, projects, row.tab.route());
    let close = navigation.clone();
    let closable = row.closable;
    let glyph = if row.tab.route().is_home() {
      Icon::Folders
    } else {
      screen_icon(row.tab.route().screen())
    };
    let (style, color) = if row.active {
      (TypographyStyle::Label, PaletteColor::TextPrimary)
    } else {
      (TypographyStyle::Description, PaletteColor::TextSecondary)
    };
    let button = pressable(self.choose(navigation.clone(), id))
      .id(format!("tab-list-{id}"))
      .describe("a11y_name", text.clone())
      .describe("state", if row.active { "active" } else { "inactive" })
      .describe("ringed", if row.ringed { "true" } else { "false" })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .on_mouse_up(move |event: MouseEvent| {
        if event.button == MouseButton::Middle && closable {
          event.prevent_default();
          close.close(id);
        }
      })
      .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
      .child(
        Text::new(&text)
          .variant(style)
          .color(color)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .with_children(attention(ctx, projects, row.tab, format!("tab-list-{id}-attention")))
      .with_children(closable.then(|| close_button(ctx, navigation.clone(), id, format!("tab-list-{id}-close"))));
    let button = if row.kept_in_view {
      button.ref_element(self.kept_row.clone())
    } else {
      button
    };
    let button = if row.active {
      button.class("active").background(ExtraColor::Hover.role())
    } else {
      button
    };
    if row.ringed {
      button.class("ringed").box_shadow(ExtraShadow::FocusRing.role()).into()
    } else {
      button.into()
    }
  }

  /// Switches to the tab `id` and closes the list, the focus back on the chevron.
  fn choose(&self, navigation: Navigation, id: TabId) -> impl Fn() + Send + Sync + 'static + use<> {
    let (open, refocus) = (self.open.clone(), self.refocus.clone());
    move || {
      navigation.select(id);
      refocus.store(true, Ordering::Relaxed);
      open.set(false);
    }
  }

  /// The keys while the list is open; plain keys only, and none a handler before took.
  fn keys(&self, navigation: Navigation) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
    let (open, cursor, refocus) = (self.open.clone(), self.cursor.clone(), self.refocus.clone());
    move |event: KeyboardEvent| {
      let plain = !(event.ctrl || event.alt || event.meta || event.shift);
      if !open.get_untracked() || event.default_prevented() || !plain {
        return;
      }
      let tabs = navigation.tabs_now();
      let from = cursor.get_untracked().unwrap_or_else(|| tabs.active().id());
      let moved = match event.key.as_str() {
        "ArrowDown" => Some(Move::Step(1)),
        "ArrowUp" => Some(Move::Step(-1)),
        "Home" => Some(Move::First),
        "End" => Some(Move::Last),
        _ => None,
      };
      match (moved, event.key.as_str()) {
        (Some(moved), _) => cursor.set(Some(moved.target(&tabs, from))),
        (None, "Enter" | " ") => {
          navigation.select(from);
          refocus.store(true, Ordering::Relaxed);
          open.set(false);
        }
        (None, "Escape") => {
          refocus.store(true, Ordering::Relaxed);
          open.set(false);
        }
        _ => return,
      }
      event.prevent_default();
    }
  }
}

/// A row as the list shows it.
struct Entry<'a> {
  tab: &'a Tab,
  active: bool,
  ringed: bool,
  closable: bool,
  /// The row the list keeps in view: the ringed one, or the active one before a key moved the ring.
  kept_in_view: bool,
}

/// Where an arrow key moves the ring.
#[derive(Clone, Copy)]
enum Move {
  /// To the next row (1) or the one before (-1), stopping at the ends.
  Step(isize),
  First,
  Last,
}

impl Move {
  /// The tab the ring moves to from `from`.
  fn target(self, tabs: &TabSet, from: TabId) -> TabId {
    let ids: Vec<TabId> = tabs.tabs().iter().map(Tab::id).collect();
    let last = ids.len().saturating_sub(1);
    let at = ids.iter().position(|id| *id == from).unwrap_or(0);
    let to = match self {
      Self::Step(step) => at.saturating_add_signed(step).min(last),
      Self::First => 0,
      Self::Last => last,
    };
    ids.get(to).copied().unwrap_or(from)
  }
}
