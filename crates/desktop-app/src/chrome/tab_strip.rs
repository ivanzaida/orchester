//! The tabs in the title bar (`Tabs` `mkCf8`): one per open tab, then *New tab*. A tab at All projects
//! is the folders icon alone (`All projects tab` `TvmmP`, `Tab/home`); every other tab shows its screen's
//! icon and `<project> · <screen>` (`Tab · Kontur · Overview` `P6KocQ`, `Tab/active`, `Tab/inactive`). The
//! active tab is filled with `hover` and has a close button; a middle click closes any tab that can be
//! closed. The OS window title is the active tab's label and the app's name. The design's
//! unsaved-changes dot comes with editing.
//!
//! The strip sits in the title bar's flexible middle. As more tabs open their labels shrink, down to
//! [`LABEL_MIN_WIDTH`]; past that the tabs scroll sideways, and *New tab* and at least
//! [`MIN_DRAG_WIDTH`] of drag area stay visible before the window controls (`docs/design/
//! orchester-workspace.md`, "Tabs" › "Overflow").

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::{MouseButton, MouseEvent},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, ScrollHorizontal, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::{
    Navigation,
    tabs::{Tab, TabSet},
  },
  routes::{ProjectPage, Route, Screen},
  theme::{ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon, screen_icon},
    pressable::pressable,
  },
};

/// Tabs and the new tab button (`Tab/*`, `New tab` `w204x`).
const TAB_HEIGHT: f32 = 26.0;
/// An All projects tab (`All projects tab`) and the new tab button are square-ish icons.
const ICON_TAB_WIDTH: f32 = 28.0;
const NEW_TAB_WIDTH: f32 = 26.0;
/// The drag area that always stays between *New tab* and the window controls.
pub const MIN_DRAG_WIDTH: f32 = 48.0;
/// The widest and the narrowest a tab's label gets as tabs open.
const LABEL_MAX_WIDTH: f32 = 160.0;
pub const LABEL_MIN_WIDTH: f32 = 56.0;
/// What a labelled tab adds around its label: padding, icon, gaps and close button.
const TAB_CHROME_WIDTH: f32 = 48.0;
/// What the title bar takes besides the tabs: the app menu and divider (61), the strip's start (8),
/// *New tab* with its gaps (30), the drag area and the Windows controls (138).
const TITLE_BAR_RESERVE: f32 = 61.0 + 8.0 + 30.0 + MIN_DRAG_WIDTH + 138.0;

pub struct TabStrip;

impl Component for TabStrip {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let tabs = navigation.tabs();
    let window = ctx.window();
    let label_width = label_width(window.info().logical_width(), tabs.tabs().len());
    let active =
      label(ctx, tabs.active().route()).unwrap_or_else(|| ctx.t(Screen::AllProjects.title_key()).to_string());
    window.set_title(ctx.t_args("app.windowTitle", [("tab", active)]).to_string());
    let strip = tabs.tabs().iter().fold(
      Row::new()
        .id("tab-strip")
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center),
      |strip, tab| strip.child(tab_button(ctx, &navigation, &tabs, tab, label_width)),
    );
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(SpacingSize::Sm))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(
        ScrollHorizontal::new(strip)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0)),
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

/// The widest a tab's label may be: the room the window leaves for tabs shared among `count` of them,
/// between [`LABEL_MIN_WIDTH`] and [`LABEL_MAX_WIDTH`].
fn label_width(window_width: f32, count: usize) -> f32 {
  let share = (window_width - TITLE_BAR_RESERVE) / count.max(1) as f32;
  (share - TAB_CHROME_WIDTH).clamp(LABEL_MIN_WIDTH, LABEL_MAX_WIDTH)
}

fn tab_button(ctx: &mut Ctx, navigation: &Navigation, tabs: &TabSet, tab: &Tab, label_width: f32) -> Element {
  let active = tab.id() == tabs.active().id();
  let id = tab.id();
  let (select, close) = (navigation.clone(), navigation.clone());
  let closable = tabs.can_close(id);
  let button = pressable(move || select.select(id))
    .id(format!("tab-{id}"))
    .describe("state", if active { "active" } else { "inactive" })
    .describe("route", tab.route().screen().name())
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
  match label(ctx, tab.route()) {
    None => all_projects_tab(ctx, button, active),
    Some(label) => labelled_tab(
      button,
      tab,
      (&label, label_width),
      active.then(|| close_button(navigation.clone(), tab)),
    ),
  }
}

/// A tab at All projects: the folders icon, named for agents.
fn all_projects_tab(ctx: &mut Ctx, button: Button, active: bool) -> Element {
  let color = if active {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  button
    .describe("a11y_name", ctx.t(Screen::AllProjects.title_key()))
    .width(Dimension::Px(ICON_TAB_WIDTH))
    .justify(Justify::Center)
    .child(icon(Icon::Folders, IconSize::Md, color))
    .into()
}

fn labelled_tab(button: Button, tab: &Tab, (label, width): (&str, f32), close: Option<Element>) -> Element {
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
  button
    .padding(padding)
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(screen_icon(tab.route().screen()), IconSize::Base, glyph))
    .child(
      Text::new(label)
        .variant(style)
        .color(text)
        .nowrap()
        .max_width(Dimension::Px(width))
        .text_overflow(TextOverflow::Elipsis),
    )
    .with_children(close)
    .into()
}

/// The active tab's close button (`Close` `hfcCG`). A tab with a label is never the only tab at All
/// projects, so it can always be closed.
fn close_button(navigation: Navigation, tab: &Tab) -> Element {
  let id = tab.id();
  Button::empty()
    .id(format!("tab-{id}-close"))
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

/// The tab's label: `<project> · <screen>`, a task by its key, the app's settings by name; `None` for
/// All projects, which shows its icon alone.
fn label(ctx: &mut Ctx, route: &Route) -> Option<String> {
  let (project, page) = match route {
    Route::AllProjects => return None,
    Route::Settings => return Some(ctx.t(route.screen().title_key()).to_string()),
    Route::Project(project, page) => (project, page),
  };
  let page = match page {
    ProjectPage::Task(key, _) => key.as_str().to_owned(),
    page => ctx.t(page.screen().title_key()).to_string(),
  };
  Some(
    ctx
      .t_args("tabs.projectScreen", [("project", project.as_str()), ("screen", &page)])
      .to_string(),
  )
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
