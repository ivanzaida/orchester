//! The tabs in the title bar (`Tabs` `mkCf8`): one per open tab, then *New tab*. A tab at the window's home is the
//! folders icon alone (`Tab/Home` `qqzuJ`), unless it is the active one at All projects; that one, and every other
//! tab, shows its screen's icon and name (`Tab/active`, `Tab/inactive`) — All projects with the layout grid (`Active
//! tab` `jMVvA` in 09 `i5G19A`), a task by its key — after its project's name once the
//! project is known to be in the list (`Kontur · Overview`); a project's id is never shown. A tab of a
//! project where something waits for the person ends in an amber hand and how many wait (`Tab/attention`
//! `iJNwr`, the count as `ruG7r`), one with an unresolved failure (runner offline, tasks unread) in a red alert;
//! the mark's tooltip says which. The
//! active tab is filled with `hover` and has a close button, unless it cannot be closed (the only tab, at All
//! projects); a middle click closes any tab that can be closed.
//! The OS window title is the active tab's label and the app's name. A tab whose task form holds edits
//! that are not saved ends its label in a 6 `text_secondary` dot (`Dirty` `TfKtF` in `Fd1ai`).
//!
//! The strip sits in the title bar's flexible middle and shares its room as `tab_fit` says; the tabs that
//! do not fit are only in the tab list (`tab_list`), whose button follows the last tab that shows. The
//! active tab takes the room it was laid out at (`tab_measure`). *New tab* is for the pointer only, never a Tab
//! stop: the Tab that comes with Alt+Tab back into the window must not land on it, where Space would open a tab.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{MouseButton, MouseEvent},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, Text, TextOverflow},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, HitTestBehavior, Style, dimension::Dimension, padding::Padding},
};

use super::{
  tab_fit::{self, DOT_WIDTH, Fit, ICON_TAB_WIDTH, MIN_DRAG_WIDTH, Slot},
  tab_label::{attention, close_button, label, mark_width, tab_icon, unsaved},
  tab_list::{TabList, TabListProps},
  tab_measure::{ActiveKey, ActiveWidth, ListHold},
};
use crate::{
  navigation::{
    Navigation,
    tabs::{Tab, TabSet},
  },
  project::Projects,
  routes::Route,
  shell::needs_you,
  theme::{CLOSING_TABS_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::{Focus, in_title_bar, pressable},
  },
};

/// Tabs, the tab list's chevron and the new tab button (`Tab/*`, `New tab` `w204x`).
pub const TAB_HEIGHT: f32 = 26.0;
/// The dot of a tab whose task form holds unsaved edits (`Dirty` `TfKtF` in `Fd1ai`).
const UNSAVED_DOT: f32 = 6.0;
const NEW_TAB_WIDTH: f32 = 26.0;

pub struct TabStrip {
  active_width: ActiveWidth,
  list: ListHold,
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

/// The strip's props: the pill's room, and whether the closing panel holds the window, when the tabs stay
/// in place, dimmed, and do not react (`Tabs` `L7gRf` in `u3Fw36`).
#[derive(Clone, PartialEq)]
pub struct TabStripProps {
  pub pill: PillRef,
  pub closing: bool,
}

impl DevtoolsInspectable for TabStripProps {}

impl Component for TabStrip {
  type Props = TabStripProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      active_width: ActiveWidth::new(ctx),
      list: ListHold::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let projects = Projects::of(ctx);
    let tabs = navigation.tabs();
    let window = ctx.window();
    let room = tab_fit::room(window.info().logical_width(), pill_width(ctx, &projects, &tabs));
    let active = label(ctx, &projects, tabs.active().route());
    let measured = self.active_width.width(&ActiveKey {
      tab: tabs.active().id(),
      label: active.clone(),
      marks: marks(&projects, tabs.active()),
    });
    let fit = fit(&projects, &tabs, (room, measured));
    window.set_title(ctx.t_args("shell.app.windowTitle", [("tab", active)]).to_string());
    let strip = tabs
      .tabs()
      .iter()
      .enumerate()
      .filter(|(index, _)| fit.shows(*index))
      .fold(
        Row::new()
          .id("tab-strip")
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0))
          .clip()
          .spacing(ExtraSpacing::Tight.role())
          .align_items(Alignment::Center),
        |strip, (_, tab)| {
          let button = tab_button(ctx, (&navigation, &projects), &tabs, tab, (&fit, &self.active_width));
          strip.child(button)
        },
      );
    let hidden: Vec<_> = fit
      .hidden()
      .iter()
      .filter_map(|index| tabs.tabs().get(*index))
      .map(Tab::id)
      .collect();
    let list = fit.overflows().then(|| {
      let list = self.list.clone();
      ctx.mount_keyed::<TabList>("tab-list", TabListProps { hidden, list })
    });
    let closing = ctx.props::<TabStripProps>().closing;
    let tabs = Row::new()
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .opacity(if closing { CLOSING_TABS_OPACITY } else { 1.0 })
      .hit_test(if closing {
        HitTestBehavior::None
      } else {
        HitTestBehavior::Auto
      })
      .child(strip)
      .with_children(list)
      .child(new_tab(ctx, navigation));
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(SpacingSize::Sm))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(tabs)
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

/// The room a tab's unsaved dot and its mark take.
fn marks(projects: &Projects, tab: &Tab) -> f32 {
  let dot = if unsaved(projects, tab.route()) { DOT_WIDTH } else { 0.0 };
  dot + mark_width(projects, tab)
}

/// Whether `tab` is the folders icon alone: a tab at the window's home, unless it is the `active` one at All projects
/// (`Active tab` `jMVvA` in 09 `i5G19A`); Add project and the start page's steps keep the icon (`W5PIR`, `kkGWj`).
fn icon_only(tab: &Tab, active: bool) -> bool {
  let route = tab.route();
  route.is_home() && !(active && matches!(route, Route::AllProjects | Route::Usage(_)))
}

/// How `tabs` share `room`, each as wide as what it shows; the active tab as wide as it was `measured`
/// once it has been laid out.
fn fit(projects: &Projects, tabs: &TabSet, (room, measured): (f32, Option<f32>)) -> Fit {
  let active = tabs.active().id();
  let slots: Vec<Slot> = tabs
    .tabs()
    .iter()
    .map(|tab| match (icon_only(tab, tab.id() == active), measured) {
      (true, _) => Slot::Icon,
      (false, Some(width)) if tab.id() == active => Slot::Measured(width),
      (false, _) => Slot::Labelled(marks(projects, tab)),
    })
    .collect();
  let index = tabs
    .tabs()
    .iter()
    .position(|tab| tab.id() == active)
    .expect("the active tab is one of the tabs");
  tab_fit::fit(room, &slots, index)
}

/// The room the *N need you* pill takes: none while it does not show, what it took when it was laid out
/// last, or the design's until it has been.
fn pill_width(ctx: &mut Ctx, projects: &Projects, tabs: &TabSet) -> f32 {
  if needs_you::waiting(projects, tabs).is_empty() {
    return 0.0;
  }
  let pill = &ctx.props::<TabStripProps>().pill.0;
  if pill.is_attached() && pill.width() > 0.0 {
    pill.width()
  } else {
    tab_fit::NEEDS_YOU_WIDTH
  }
}

fn tab_button(
  ctx: &mut Ctx,
  (navigation, projects): (&Navigation, &Projects),
  tabs: &TabSet,
  tab: &Tab,
  (fit, active_width): (&Fit, &ActiveWidth),
) -> Element {
  let active = tab.id() == tabs.active().id();
  let id = tab.id();
  let (select, close) = (navigation.clone(), navigation.clone());
  let closable = tabs.can_close(id);
  let label = label(ctx, projects, tab.route());
  let button = in_title_bar(pressable(move || select.select(id)))
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
  if icon_only(tab, active) {
    return home_tab(button, active);
  }
  let attention = attention(ctx, projects, tab, format!("tab-{id}-attention"));
  let close = (active && closable).then(|| close_button(ctx, navigation.clone(), id, format!("tab-{id}-close")));
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
  let marks = marks(projects, tab);
  let widest = Slot::Labelled(marks).width(fit.label_width(), active);
  // The active tab keeps its width, which the strip measures; while tabs hide, the inactive ones are
  // exactly as wide as `tab_fit` counts them (120 and their marks, `Tab 2 · Kontur · Overview` in `oHEGQ`).
  let button = if active {
    button.ref_element(active_width.element()).flex_shrink(0.0)
  } else if fit.overflows() {
    button.width(Dimension::Px(widest)).flex_shrink(0.0)
  } else {
    button
  };
  labelled_tab(button, tab, (&label, widest, active), (dirty, attention, close))
}

/// A tab at the window's home that shows the folders icon alone (`Tab/Home` `qqzuJ`).
fn home_tab(button: Button, active: bool) -> Element {
  let color = if active {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  button
    .width(Dimension::Px(ICON_TAB_WIDTH))
    .flex_shrink(0.0)
    .justify(Justify::Center)
    .child(icon(Icon::Folders, IconSize::Md, color))
    .into()
}

fn labelled_tab(
  button: Button,
  tab: &Tab,
  (label, widest, active): (&str, f32, bool),
  (dirty, attention, close): (Option<Element>, Option<Element>, Option<Element>),
) -> Element {
  let (glyph, text, style) = if active {
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
  // The tab is at most as wide as `tab_fit` counts it, and its label gives way.
  button
    .max_width(Dimension::Px(widest))
    .padding(padding)
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(tab_icon(tab.route()), IconSize::Base, glyph))
    .child(
      Text::new(label)
        .variant(style)
        .color(text)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(dirty)
    .with_children(attention)
    .with_children(close)
    .into()
}

/// *New tab* (`New tab` `w204x`): a tab at All projects, right of the active one; Ctrl+T does it from the keyboard.
fn new_tab(ctx: &mut Ctx, navigation: Navigation) -> Element {
  in_title_bar(Focus::Pointer.apply(pressable(move || navigation.open_tab())))
    .id("new-tab")
    .describe("a11y_name", ctx.t("common.newTab"))
    .width(Dimension::Px(NEW_TAB_WIDTH))
    .height(Dimension::Px(TAB_HEIGHT))
    .flex_shrink(0.0)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::Plus, IconSize::Md, PaletteColor::TextMuted))
    .into()
}
