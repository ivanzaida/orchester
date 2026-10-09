//! The tabs in the title bar (`Tabs` `mkCf8`): one per open tab, then *New tab*. A tab at All projects
//! is the folders icon alone (`All projects tab` `TvmmP`, `Tab/home`); every other tab shows its screen's
//! icon and name (`Tab/active`, `Tab/inactive`) — a task by its key — after its project's name once the
//! project is known to be in the list (`Kontur · Overview`); a project's id is never shown. A tab of a
//! project where something waits for the person ends in an amber hand (`Tab/attention` `iJNwr`), one with
//! an unresolved failure (runner offline, tasks unread) in a red alert; the mark's tooltip says which. The
//! active tab is filled with `hover` and has a close button; a middle click
//! closes any tab that can be closed.
//! The OS window title is the active tab's label and the app's name. A tab whose task form holds edits
//! that are not saved ends its label in a 6 `text_secondary` dot (`Dirty` `TfKtF` in `Fd1ai`).
//!
//! The strip sits in the title bar's flexible middle and shares its room as `tab_fit` says; once it
//! scrolls, the active tab is brought into view whenever it changes, the mouse wheel scrolls it
//! sideways, and it shows no scrollbar.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{MouseButton, MouseEvent, ScrollEvent},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, ScrollHorizontal, Text, TextOverflow},
  core::ElementRef,
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
  project::Projects,
  routes::{ProjectPage, Route},
  shell::needs_you::{self, Attention, count_words},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon, screen_icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// Tabs and the new tab button (`Tab/*`, `New tab` `w204x`).
const TAB_HEIGHT: f32 = 26.0;
/// The dot of a tab whose task form holds unsaved edits (`Dirty` `TfKtF` in `Fd1ai`).
const UNSAVED_DOT: f32 = 6.0;
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

/// The title bar's *N need you* pill, as it was laid out last.
#[derive(Clone)]
pub struct PillRef(pub ElementRef);

/// One pill for the window's life.
impl PartialEq for PillRef {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl DevtoolsInspectable for PillRef {}

impl Component for TabStrip {
  type Props = PillRef;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      scroll: ScrollState::new(),
      revealed: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let projects = Projects::of(ctx);
    let tabs = navigation.tabs();
    let window = ctx.window();
    let fit = tab_fit::fit(window.info().logical_width(), pill_width(ctx, &projects, &tabs), &tabs);
    let active = label(ctx, &projects, tabs.active().route());
    window.set_title(ctx.t_args("shell.app.windowTitle", [("tab", active)]).to_string());
    self.reveal(&tabs, fit);
    let strip = tabs.tabs().iter().fold(
      Row::new()
        .id("tab-strip")
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center),
      |strip, tab| strip.child(tab_button(ctx, &navigation, &projects, &tabs, tab, fit)),
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

/// The room the *N need you* pill takes: none while it does not show, what it took when it was laid out
/// last, or the design's until it has been.
fn pill_width(ctx: &mut Ctx, projects: &Projects, tabs: &TabSet) -> f32 {
  if needs_you::waiting(projects, tabs).is_empty() {
    return 0.0;
  }
  let pill = &ctx.props::<PillRef>().0;
  if pill.is_attached() && pill.width() > 0.0 {
    pill.width()
  } else {
    tab_fit::NEEDS_YOU_WIDTH
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

fn tab_button(
  ctx: &mut Ctx,
  navigation: &Navigation,
  projects: &Projects,
  tabs: &TabSet,
  tab: &Tab,
  fit: Fit,
) -> Element {
  let active = tab.id() == tabs.active().id();
  let id = tab.id();
  let (select, close) = (navigation.clone(), navigation.clone());
  let closable = tabs.can_close(id);
  let label = label(ctx, projects, tab.route());
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
  let attention = attention(ctx, projects, tab);
  let close = active.then(|| close_button(ctx, navigation.clone(), tab));
  let dirty = unsaved(projects, tab.route()).then(|| {
    Row::new()
      .id(format!("tab-{id}-unsaved"))
      .width(Dimension::Px(UNSAVED_DOT))
      .height(Dimension::Px(UNSAVED_DOT))
      .flex_shrink(0.0)
      .rounded(ExtraRadius::Full.role())
      .background(PaletteColor::TextSecondary)
      .into()
  });
  labelled_tab(button, tab, (&label, fit), (dirty, attention, close))
}

/// The mark of a tab whose project needs the person (an amber hand, *Kontur · 2 need you*) or has an
/// unresolved failure (a red alert, *Kontur · runner offline*), with what it means in its tooltip.
fn attention(ctx: &mut Ctx, projects: &Projects, tab: &Tab) -> Option<Element> {
  let project = tab.route().project()?;
  let mark = needs_you::attention(projects, project)?;
  let name = projects.name(project)?;
  let (glyph, color, what, state) = match mark {
    Attention::Waiting(count) => (Icon::Hand, PaletteColor::Warning, count_words(ctx, count), "needs-you"),
    Attention::RunnerOffline => (
      Icon::CircleAlert,
      PaletteColor::Danger,
      ctx.t("shell.tabs.runnerOffline").to_string(),
      "runner-offline",
    ),
    Attention::TasksUnread => (
      Icon::CircleAlert,
      PaletteColor::Danger,
      ctx.t("shell.tabs.tasksUnread").to_string(),
      "tasks-unread",
    ),
  };
  let text = ctx.t_args(
    "shell.tabs.attention",
    [("project", name.as_str()), ("waiting", what.as_str())],
  );
  let id = format!("tab-{}-attention", tab.id());
  let marker = icon(glyph, IconSize::Xs, color).id(id.clone()).describe("state", state);
  Some(Tooltip::new(id, text).wrap(ctx, marker))
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

/// Whether the task form `route` shows holds edits that are not saved.
fn unsaved(projects: &Projects, route: &Route) -> bool {
  let (project, task) = match route {
    Route::Project(project, ProjectPage::NewTask) => (*project, None),
    Route::Project(project, ProjectPage::EditTask(key)) => (*project, Some(key.to_string())),
    _ => return false,
  };
  projects
    .feed_now(project)
    .is_some_and(|feed| feed.unsaved.get().contains(task.as_deref()))
}

fn labelled_tab(
  button: Button,
  tab: &Tab,
  (label, fit): (&str, Fit),
  (dirty, attention, close): (Option<Element>, Option<Element>, Option<Element>),
) -> Element {
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
    .with_children(dirty)
    .with_children(attention)
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

/// The tab's label: its screen's name, a task by its key and its Edit task as `Edit <key>` (`Qp1B0`),
/// after the project's name once it is known (`Kontur · KON-12`).
fn label(ctx: &mut Ctx, projects: &Projects, route: &Route) -> String {
  let screen = match route {
    Route::Project(_, ProjectPage::Task(key, _)) => key.to_string(),
    Route::Project(_, ProjectPage::EditTask(key)) => ctx
      .t_args("shell.screens.editTaskOf", [("key", key.to_string())])
      .to_string(),
    route => ctx.t(route.screen().title_key()).to_string(),
  };
  match route.project().and_then(|project| projects.name(project)) {
    Some(name) => ctx
      .t_args(
        "shell.tabs.inProject",
        [("project", name.as_str()), ("label", screen.as_str())],
      )
      .to_string(),
    None => screen,
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
