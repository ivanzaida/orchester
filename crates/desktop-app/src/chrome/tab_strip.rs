//! The tabs in the title bar (`Tabs` `mkCf8`): lurq's window tabs (`ui::WindowTabs`, its look in
//! `theme::kit::window_tabs`), one per open tab, then *New tab*. A tab at the window's home is the folders icon alone
//! (`Tab/Home` `qqzuJ`), unless it is the active one at All projects; that one, and every other tab, shows its
//! screen's icon and name (`Tab/Active`, `Tab/Inactive`) — All projects with the layout grid (`Active tab` `jMVvA` in
//! 09 `i5G19A`), a task by its key — after its project's name once the project is known to be in the list (`Kontur ·
//! Overview`); a project's id is never shown. A tab of a project where something waits for the person ends in an
//! amber hand and how many wait (`Tab/attention` `iJNwr`, the count as `ruG7r`), one with an unresolved failure
//! (runner offline, tasks unread) in a red alert; the mark's tooltip says which. The active tab is filled with
//! `hover` and has a close button, unless it cannot be closed (the only tab, at All projects); a middle click closes
//! any tab that can be closed. The OS window title is the active tab's label and the app's name. A tab whose task
//! form holds edits that are not saved shows a dot after its label (`Dirty` `TfKtF` in `Fd1ai`), beside its close
//! button.
//!
//! Each tab is as wide as what it shows (`tab_fit`, from the widths `tab_measure` learns); the tabs that do not fit
//! go into the menu of the button after the last tab that shows, which counts them. The tabs, *New tab* and that
//! button are for the pointer only, never a Tab stop: the Tab that comes with Alt+Tab back into the window must not
//! land on them, where Space would open a tab. The window's own shortcuts (`navigation::shortcuts`) switch, open and
//! close tabs.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::TypographyStyle,
  },
  components::Row,
  core::ElementRef,
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension},
  ui::{WindowTab, WindowTabs, WindowTabsDirtyMark, WindowTabsOverflow},
};

use super::{
  tab_fit::{self, DOT_WIDTH, ICON_TAB_WIDTH, LABEL_MAX_WIDTH, LABEL_MIN_WIDTH, MARK_GAP, Slot},
  tab_label::{attention, label, tab_icon, unsaved},
  tab_measure::TabWidths,
};
use crate::{
  navigation::{
    Navigation,
    tabs::{Tab, TabId, TabSet},
  },
  project::Projects,
  routes::Route,
  shell::needs_you,
  theme::CLOSING_TABS_OPACITY,
  ui::icon::Icon,
};

/// The strip's id: its tabs are `tab-<id>`, *New tab* `tab-strip-add`, the button of the tabs that do not fit
/// `tab-strip-more`.
const STRIP_ID: &str = "tab-strip";

pub struct TabStrip {
  widths: TabWidths,
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

/// What one tab shows, as the strip sizes and draws it.
struct Shown<'a> {
  tab: &'a Tab,
  label: String,
  active: bool,
  closable: bool,
  icon_only: bool,
  unsaved: bool,
  mark: Option<Element>,
}

impl Shown<'_> {
  /// The room its unsaved dot and its mark take, `mark` wide.
  fn marks(&self, mark: f32) -> f32 {
    let dot = if self.unsaved { DOT_WIDTH } else { 0.0 };
    dot + if self.mark.is_some() { MARK_GAP + mark } else { 0.0 }
  }
}

impl Component for TabStrip {
  type Props = TabStripProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      widths: TabWidths::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let projects = Projects::of(ctx);
    let tabs = navigation.tabs();
    let window = ctx.window();
    let strip = tab_fit::strip_room(window.info().logical_width(), pill_width(ctx, &projects, &tabs));
    let shown: Vec<Shown> = tabs
      .tabs()
      .iter()
      .map(|tab| shown(ctx, &projects, &tabs, tab))
      .collect();
    let cap = self.label_cap(&shown, strip);
    let active = tabs.active();
    let title = label(ctx, &projects, active.route());
    window.set_title(ctx.t_args("shell.app.windowTitle", [("tab", title)]).to_string());
    let unseen = self.widths.unseen(
      &shown
        .iter()
        .filter(|shown| !shown.icon_only)
        .map(|shown| (shown.tab.id(), shown.label.clone(), typography(shown.active)))
        .collect::<Vec<_>>(),
    );
    let strip_tabs: Vec<WindowTab<TabId>> = shown
      .into_iter()
      .map(|shown| {
        let width = self.width(&shown, cap);
        self.tab(shown, width)
      })
      .collect();
    let tabs = window_tabs(ctx, &navigation, active.id(), strip_tabs);
    let closing = ctx.props::<TabStripProps>().closing;
    Row::new()
      .max_width(Dimension::Px(strip.max(0.0)))
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .align_items(Alignment::Center)
      .opacity(if closing { CLOSING_TABS_OPACITY } else { 1.0 })
      .hit_test(if closing {
        HitTestBehavior::None
      } else {
        HitTestBehavior::Auto
      })
      .child(unseen)
      .child(tabs)
  }
}

impl TabStrip {
  /// How wide the inactive labels get beside the others in a strip `strip` wide; `None` once the tabs do not fit.
  fn label_cap(&self, shown: &[Shown], strip: f32) -> Option<f32> {
    let slots: Vec<Slot> = shown
      .iter()
      .map(|shown| {
        let marks = shown.marks(self.widths.mark(shown.tab.id()));
        match (shown.icon_only, shown.active, self.widths.label(shown.tab.id())) {
          (true, ..) => Slot::Icon,
          (false, true, Some(label)) => {
            Slot::Measured(tab_fit::tab_width(Some(label), LABEL_MAX_WIDTH, marks, shown.closable))
          }
          (false, ..) => Slot::Labelled(marks),
        }
      })
      .collect();
    let active = shown
      .iter()
      .position(|shown| shown.active)
      .expect("the active tab is one of the tabs");
    tab_fit::label_cap(tab_fit::room(strip), &slots, active)
  }

  /// How wide `shown`'s tab is: while it is inactive its label capped at `cap`, or `TAB_MIN_WIDTH` and its marks
  /// once the tabs do not fit.
  fn width(&self, shown: &Shown, cap: Option<f32>) -> f32 {
    if shown.icon_only {
      return ICON_TAB_WIDTH;
    }
    let id = shown.tab.id();
    let marks = shown.marks(self.widths.mark(id));
    let (label, cap) = match (shown.active, cap) {
      (true, _) => (self.widths.label(id), LABEL_MAX_WIDTH),
      (false, Some(cap)) => (self.widths.label(id), cap),
      (false, None) => (None, LABEL_MIN_WIDTH),
    };
    tab_fit::tab_width(label, cap, marks, shown.active && shown.closable)
  }

  /// The window tab of `shown`, `width` wide.
  fn tab(&self, shown: Shown, width: f32) -> WindowTab<TabId> {
    let id = shown.tab.id();
    let route = shown.tab.route();
    let tab = WindowTab::new(id, &shown.label)
      .id(format!("tab-{id}"))
      .describe("state", if shown.active { "active" } else { "inactive" })
      .describe("route", route.screen().name())
      .closable(shown.closable)
      .close_button(shown.active)
      .dirty(shown.unsaved);
    if shown.icon_only {
      return tab.icon(Icon::Folders.name()).icon_only(true);
    }
    let tab = tab.icon(tab_icon(route).name()).width(width);
    match shown.mark {
      Some(mark) => tab.trailing(self.widths.watch_mark(id, mark)),
      None => tab,
    }
  }
}

/// The strip of `tabs`, the one `active` selected.
fn window_tabs(ctx: &mut Ctx, navigation: &Navigation, active: TabId, tabs: Vec<WindowTab<TabId>>) -> Element {
  let (select, close, open) = (navigation.clone(), navigation.clone(), navigation.clone());
  WindowTabs::controlled(active, tabs, move |id| select.select(*id))
    .id(STRIP_ID)
    .label(&ctx.t("shell.tabs.strip"))
    .add_label(&ctx.t("common.newTab"))
    .close_label(&ctx.t("shell.tabs.close"))
    .more_label(&ctx.t("shell.tabs.hidden"))
    .overflow(WindowTabsOverflow::List)
    .dirty_mark(WindowTabsDirtyMark::AfterLabel)
    .tab_stop(false)
    // The window's own shortcuts switch and close tabs, by the app's rules.
    .shortcuts(false)
    .on_close(move |id| close.close(*id))
    .on_add(move || open.open_tab())
    .mount(ctx)
}

/// What `tab` shows.
fn shown<'a>(ctx: &mut Ctx, projects: &Projects, tabs: &TabSet, tab: &'a Tab) -> Shown<'a> {
  let active = tab.id() == tabs.active().id();
  Shown {
    tab,
    label: label(ctx, projects, tab.route()),
    active,
    closable: tabs.can_close(tab.id()),
    icon_only: icon_only(tab, active),
    unsaved: unsaved(projects, tab.route()),
    mark: attention(ctx, projects, tab, format!("tab-{}-attention", tab.id())),
  }
}

/// The typography of a tab's label: the active one's, or another's.
const fn typography(active: bool) -> TypographyStyle {
  if active {
    TypographyStyle::Label
  } else {
    TypographyStyle::Description
  }
}

/// Whether `tab` is the folders icon alone: a tab at the window's home, unless it is the `active` one at All projects
/// (`Active tab` `jMVvA` in 09 `i5G19A`); Add project and the start page's steps keep the icon (`W5PIR`, `kkGWj`).
fn icon_only(tab: &Tab, active: bool) -> bool {
  let route = tab.route();
  route.is_home() && !(active && matches!(route, Route::AllProjects | Route::Usage(_)))
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
