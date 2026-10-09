//! The tabs in the title bar (`Tabs` `mkCf8`): one per open tab, then *New tab*. A tab at All projects
//! is the folders icon alone (`All projects tab` `TvmmP`, `Tab/home`); every other tab shows its screen's
//! icon and `<project> · <screen>` (`Tab · Kontur · Overview` `P6KocQ`, `Tab/active`, `Tab/inactive`). The
//! active tab is filled with `hover` and has a close button; a middle click closes any tab that can be
//! closed. The design's unsaved-changes dot comes with editing.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::{MouseButton, MouseEvent},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::{
    Navigation,
    tabs::{Tab, TabSet},
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
/// An All projects tab (`All projects tab`) and the new tab button are square-ish icons.
const ICON_TAB_WIDTH: f32 = 28.0;
const NEW_TAB_WIDTH: f32 = 26.0;

pub struct TabStrip;

impl Component for TabStrip {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let tabs = navigation.tabs();
    let strip = Row::new()
      .id("tab-strip")
      .padding(Padding::new().left(SpacingSize::Xs))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center);
    tabs
      .tabs()
      .iter()
      .fold(strip, |strip, tab| {
        strip.child(tab_button(ctx, &navigation, &tabs, tab))
      })
      .child(new_tab(ctx, navigation))
  }
}

fn tab_button(ctx: &mut Ctx, navigation: &Navigation, tabs: &TabSet, tab: &Tab) -> Element {
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
      &label,
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
    .describe("a11y_name", ctx.t("screens.allProjects"))
    .width(Dimension::Px(ICON_TAB_WIDTH))
    .justify(Justify::Center)
    .child(icon(Icon::Folders, IconSize::Md, color))
    .into()
}

fn labelled_tab(button: Button, tab: &Tab, label: &str, close: Option<Element>) -> Element {
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
    .child(Text::new(label).variant(style).color(text).nowrap())
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
    .describe("a11y_name", ctx.t("tabs.newTab"))
    .width(Dimension::Px(NEW_TAB_WIDTH))
    .height(Dimension::Px(TAB_HEIGHT))
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::Plus, IconSize::Md, PaletteColor::TextMuted))
    .into()
}
