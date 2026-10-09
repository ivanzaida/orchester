//! The tabs in the title bar (`Tabs` `mkCf8`): one per open tab, then *New tab*. A tab at All projects
//! is the folders icon alone (`All projects tab` `TvmmP`, `Tab/home`); every other tab shows its screen's
//! icon and name (`Tab/active`, `Tab/inactive`) — a task by its key. The design's `<project> · <screen>`
//! needs the project's name, which comes with reading projects; a project's id is never shown. The
//! active tab is filled with `hover` and has a close button; a middle click closes any tab that can be
//! closed. The OS window title is the active tab's label and the app's name. The design's
//! unsaved-changes dot comes with editing.
//!
//! The strip sits in the title bar's flexible middle and shares its room as `tab_fit` says; once it
//! scrolls, the active tab is brought into view whenever it changes, the mouse wheel scrolls it
//! sideways, and it shows no scrollbar.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::{MouseButton, MouseEvent, ScrollEvent},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, ScrollHorizontal, Text, TextOverflow},
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
    scrollbar::ScrollBarStyle,
  },
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};
use parking_lot::Mutex;

use super::tab_fit::{self, Fit, ICON_TAB_WIDTH, MIN_DRAG_WIDTH, TAB_MIN_WIDTH};
use crate::{
  navigation::{
    Navigation,
    tabs::{Tab, TabId, TabSet},
  },
  routes::{ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon, screen_icon},
    pressable::pressable,
  },
};

/// Tabs and the new tab button (`Tab/*`, `New tab` `w204x`).
const TAB_HEIGHT: f32 = 26.0;
const NEW_TAB_WIDTH: f32 = 26.0;

pub struct TabStrip {
  scroll: ScrollState,
  /// What the strip last brought into view.
  revealed: Mutex<Option<Revealed>>,
}

/// The active tab as the strip last brought it into view: which tab, at which route (which decides its
/// width), and whether the strip scrolled.
#[derive(PartialEq)]
struct Revealed {
  tab: TabId,
  route: Route,
  scrolling: bool,
}

impl Component for TabStrip {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      scroll: ScrollState::new(),
      revealed: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let tabs = navigation.tabs();
    let window = ctx.window();
    let fit = tab_fit::fit(window.info().logical_width(), &tabs);
    let active = label(ctx, tabs.active().route());
    window.set_title(ctx.t_args("shell.app.windowTitle", [("tab", active)]).to_string());
    self.reveal(&tabs, fit);
    let strip = tabs.tabs().iter().fold(
      Row::new()
        .id("tab-strip")
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center),
      |strip, tab| strip.child(tab_button(ctx, &navigation, &tabs, tab, fit)),
    );
    let wheel = self.scroll.clone();
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(SpacingSize::Sm))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(
        ScrollHorizontal::new(strip)
          .id("tab-scroll")
          .with_scroll_state(self.scroll.clone())
          .scrollbar(ScrollBarStyle::hidden())
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0))
          .on_scroll(move |event: ScrollEvent| {
            if event.delta_x == 0.0 && event.delta_y != 0.0 {
              wheel.scroll_by(-event.delta_y, 0.0);
              event.prevent_default();
            }
          }),
      )
      .child(new_tab(ctx, navigation))
      // The drag area: the 48 that always stay, then whatever the tabs leave.
      .child(
        Row::new()
          .id("title-bar-drag")
          .width(Dimension::Px(MIN_DRAG_WIDTH))
          .height(Dimension::Px(TAB_HEIGHT)),
      )
      .child(Row::new().flex(1.0))
  }
}

impl TabStrip {
  /// Brings the active tab into view when it, its route or the strip's fit changed since last time:
  /// a new tab, another place in the same tab (its width may change), or a strip that starts to scroll.
  /// The offset is queued even when it is the current one, so it holds against lurq keeping a strip
  /// that was scrolled to its end at the end while a tab is inserted.
  fn reveal(&self, tabs: &TabSet, fit: Fit) {
    let active = tabs.active();
    let now = Revealed {
      tab: active.id(),
      route: active.route().clone(),
      scrolling: matches!(fit, Fit::Scrolling(_)),
    };
    let mut revealed = self.revealed.lock();
    if revealed.as_ref() == Some(&now) {
      return;
    }
    *revealed = Some(now);
    let active = active.id();
    let Fit::Scrolling(room) = fit else {
      self.scroll.scroll_to_left_pending();
      return;
    };
    let Some(index) = tabs.tabs().iter().position(|tab| tab.id() == active) else {
      return;
    };
    let viewport = Some(self.scroll.viewport_width())
      .filter(|width| *width > 0.0)
      .unwrap_or(room);
    let target = tab_fit::reveal(self.scroll.scroll_x(), viewport, tab_fit::span(tabs, index));
    self.scroll.set_scroll_pending(target, 0.0);
  }
}

fn tab_button(ctx: &mut Ctx, navigation: &Navigation, tabs: &TabSet, tab: &Tab, fit: Fit) -> Element {
  let active = tab.id() == tabs.active().id();
  let id = tab.id();
  let (select, close) = (navigation.clone(), navigation.clone());
  let closable = tabs.can_close(id);
  let label = label(ctx, tab.route());
  let button = pressable(move || select.select(id))
    .id(format!("tab-{id}"))
    .describe("state", if active { "active" } else { "inactive" })
    .describe("route", tab.route().screen().name())
    .describe("a11y_name", label.clone())
    .height(Dimension::Px(TAB_HEIGHT))
    .rounded(RadiusSize::Md)
    .on_mouse_up(move |event: MouseEvent| {
      if event.button == MouseButton::Middle && closable {
        event.prevent_default();
        close.close(id);
      }
    });
  let button = if active {
    button.background(ExtraColor::Hover.role())
  } else {
    button.hovered_style(Style::new().background(ExtraColor::Hover.role()))
  };
  if tab_fit::is_icon(tab) {
    return all_projects_tab(button, active);
  }
  let close = active.then(|| close_button(ctx, navigation.clone(), tab));
  labelled_tab(button, tab, (&label, fit), close)
}

/// A tab at All projects: the folders icon.
fn all_projects_tab(button: Button, active: bool) -> Element {
  let color = if active {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  button
    .width(Dimension::Px(ICON_TAB_WIDTH))
    .justify(Justify::Center)
    .child(icon(Icon::Folders, IconSize::Md, color))
    .into()
}

fn labelled_tab(button: Button, tab: &Tab, (label, fit): (&str, Fit), close: Option<Element>) -> Element {
  let (glyph, text, style) = if close.is_some() {
    (
      PaletteColor::TextSecondary,
      PaletteColor::TextPrimary,
      TypographyStyle::Label,
    )
  } else {
    (
      PaletteColor::TextMuted,
      PaletteColor::TextMuted,
      TypographyStyle::Description,
    )
  };
  let padding = Padding::horizontal(ExtraSpacing::Loose.role());
  let padding = if close.is_some() {
    padding.right(ExtraSpacing::Snug.role())
  } else {
    padding
  };
  let text = Text::new(label)
    .variant(style)
    .color(text)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis);
  let (button, text) = match fit {
    Fit::Shrunk(label_width) => (button, text.max_width(Dimension::Px(label_width))),
    Fit::Scrolling(_) => (
      button.width(Dimension::Px(TAB_MIN_WIDTH)),
      text.flex(1.0).min_width(Dimension::Px(0.0)),
    ),
  };
  button
    .padding(padding)
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(screen_icon(tab.route().screen()), IconSize::Base, glyph))
    .child(text)
    .with_children(close)
    .into()
}

/// The active tab's close button (`Close` `hfcCG`). A tab with a label is never the only tab at All
/// projects, so it can always be closed.
fn close_button(ctx: &mut Ctx, navigation: Navigation, tab: &Tab) -> Element {
  let id = tab.id();
  Button::empty()
    .id(format!("tab-{id}-close"))
    .describe("a11y_name", ctx.t("shell.tabs.close"))
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .align_items(Alignment::Center)
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      navigation.close(id);
    })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .rounded(RadiusSize::Sm)
    .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

/// The tab's label: its screen's name, a task by its key and its Edit task as `Edit <key>` (`Qp1B0`).
fn label(ctx: &mut Ctx, route: &Route) -> String {
  match route {
    Route::Project(_, ProjectPage::Task(key, _)) => key.to_string(),
    Route::Project(_, ProjectPage::EditTask(key)) => ctx
      .t_args("shell.screens.editTaskOf", [("key", key.to_string())])
      .to_string(),
    route => ctx.t(route.screen().title_key()).to_string(),
  }
}

/// *New tab* (`New tab` `w204x`): a tab at All projects, right of the active one.
fn new_tab(ctx: &mut Ctx, navigation: Navigation) -> Element {
  pressable(move || navigation.open_tab())
    .id("new-tab")
    .describe("a11y_name", ctx.t("common.newTab"))
    .width(Dimension::Px(NEW_TAB_WIDTH))
    .height(Dimension::Px(TAB_HEIGHT))
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::Plus, IconSize::Md, PaletteColor::TextMuted))
    .into()
}
