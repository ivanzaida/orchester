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
//! Each tab is as wide as what it shows up to its cap (`tab_fit`); the tabs that do not fit go into the menu of the
//! button after the last tab that shows, which counts them and opens the list of every open tab (`Tab list` `pB0VB`:
//! search above eight tabs, a row's page with its project quiet after it and the project's mark, a close button on each
//! row, Ctrl+Shift+A, Cmd+Shift+A on macOS, to open and close it). The tabs, *New tab* and that button are for the
//! pointer only, never a Tab stop: the Tab that comes with Alt+Tab back into the window must not land on them, where
//! Space would open a tab. The window's own shortcuts (`navigation::shortcuts`) switch, open and close tabs.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  core::ElementRef,
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension},
  ui::{WindowTab, WindowTabs, WindowTabsDirtyMark, WindowTabsMenu, WindowTabsOverflow},
};

use super::{
  tab_fit::{self, DOT_WIDTH, LABEL_MAX_WIDTH, LABEL_MIN_WIDTH, Slot},
  tab_label::{attention, in_project, label, mark_width, page, project_name, tab_icon, unsaved},
};
use crate::{
  chrome::TITLE_BAR_HEIGHT,
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
/// `tab-strip-more`, its list `tab-strip-panel` with `tab-strip-search` and a row `tab-<id>-item` for each tab.
const STRIP_ID: &str = "tab-strip";
/// How far the list of every open tab stays inside the window's edges: 8 inside its right edge, and its rows end 8
/// above its bottom, so it is at most the window less 48 under the bar (`pB0VB`).
const LIST_INSET: f32 = 8.0;

pub struct TabStrip;

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

/// What one tab shows, as the strip sizes and draws it, and what its row in the list of every tab says.
struct Shown<'a> {
  tab: &'a Tab,
  label: String,
  /// Its page, which its row says in place of the label, and its project's name after it.
  page: String,
  project: Option<String>,
  active: bool,
  closable: bool,
  icon_only: bool,
  unsaved: bool,
  mark: Option<Element>,
  /// The same mark in its row, an element of its own.
  row_mark: Option<Element>,
  /// The room its unsaved dot and its mark take.
  marks: f32,
}

impl Component for TabStrip {
  type Props = TabStripProps;

  fn create(_: &mut Ctx) -> Self {
    Self
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
    let cap = label_cap(&shown, strip);
    let active = tabs.active();
    let title = label(ctx, &projects, active.route());
    window.set_title(ctx.t_args("shell.app.windowTitle", [("tab", title)]).to_string());
    let strip_tabs: Vec<WindowTab<TabId>> = shown.into_iter().map(|shown| tab(shown, cap)).collect();
    // The strip's row is as high as the title bar: the list of every tab stands 4 under it.
    let bar = ctx.element_ref();
    let tabs = window_tabs(ctx, &navigation, (active.id(), bar.clone()), strip_tabs);
    let closing = ctx.props::<TabStripProps>().closing;
    Row::new()
      .ref_element(bar)
      .height(Dimension::Px(TITLE_BAR_HEIGHT))
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
      .child(tabs)
  }
}

/// How wide the inactive labels get beside the others in a strip `strip` wide; `None` once the tabs do not fit.
fn label_cap(shown: &[Shown], strip: f32) -> Option<f32> {
  let slots: Vec<Slot> = shown
    .iter()
    .map(|shown| {
      if shown.icon_only {
        Slot::Icon
      } else {
        Slot::Labelled(shown.marks)
      }
    })
    .collect();
  let active = shown
    .iter()
    .position(|shown| shown.active)
    .expect("the active tab is one of the tabs");
  tab_fit::label_cap(tab_fit::room(strip), &slots, active)
}

/// The window tab of `shown`, as wide as what it shows: its label up to `LABEL_MAX_WIDTH` while it is active, else
/// up to `cap`, or `LABEL_MIN_WIDTH` once the tabs do not fit.
fn tab(shown: Shown, cap: Option<f32>) -> WindowTab<TabId> {
  let id = shown.tab.id();
  let route = shown.tab.route();
  let tab = WindowTab::new(id, &shown.label)
    .id(format!("tab-{id}"))
    .describe("state", if shown.active { "active" } else { "inactive" })
    .describe("route", route.screen().name())
    .closable(shown.closable)
    .close_button(shown.active)
    .dirty(shown.unsaved)
    .list_label(&shown.page);
  let tab = match &shown.project {
    Some(project) => tab.list_detail(project),
    None => tab,
  };
  let tab = match shown.row_mark {
    Some(mark) => tab.list_trailing(mark),
    None => tab,
  };
  if shown.icon_only {
    return tab.icon(Icon::Folders.name()).icon_only(true);
  }
  let label = match (shown.active, cap) {
    (true, _) => LABEL_MAX_WIDTH,
    (false, Some(cap)) => cap,
    (false, None) => LABEL_MIN_WIDTH,
  };
  let close = shown.active && shown.closable;
  let tab = tab
    .icon(tab_icon(route).name())
    .fit_content(tab_fit::tab_max(label, shown.marks, close));
  match shown.mark {
    Some(mark) => tab.trailing(mark),
    None => tab,
  }
}

/// The strip of `tabs`, the one `active` selected, its list of every tab under the title bar `bar`.
fn window_tabs(
  ctx: &mut Ctx,
  navigation: &Navigation,
  (active, bar): (TabId, ElementRef),
  tabs: Vec<WindowTab<TabId>>,
) -> Element {
  let (select, close, open) = (navigation.clone(), navigation.clone(), navigation.clone());
  let i18n = ctx.i18n().clone();
  WindowTabs::controlled(active, tabs, move |id| select.select(*id))
    .id(STRIP_ID)
    .label(&ctx.t("shell.tabs.strip"))
    .add_label(&ctx.t("common.newTab"))
    .close_label(&ctx.t("shell.tabs.close"))
    .more_label(&ctx.t("shell.tabs.list"))
    .overflow(WindowTabsOverflow::List)
    .menu(WindowTabsMenu::All)
    .list_shortcut(true)
    .list_clear_of(bar)
    .list_inset(LIST_INSET)
    // "3 more tabs"; lurq puts the keys of the list shortcut after it.
    .more_tooltip(move |count| {
      let key = if count == 1 {
        "shell.tabs.more.one"
      } else {
        "shell.tabs.more.other"
      };
      i18n.t_args(key, [("count", count.to_string())]).to_string()
    })
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
  let unsaved = unsaved(projects, tab.route());
  let dot = if unsaved { DOT_WIDTH } else { 0.0 };
  let page = page(ctx, projects, tab.route());
  let project = project_name(projects, tab.route());
  Shown {
    tab,
    label: in_project(ctx, page.clone(), project.as_deref()),
    page,
    project,
    active,
    closable: tabs.can_close(tab.id()),
    icon_only: icon_only(tab, active),
    unsaved,
    mark: attention(ctx, projects, tab, format!("tab-{}-attention", tab.id())),
    row_mark: attention(ctx, projects, tab, format!("tab-{}-item-attention", tab.id())),
    marks: dot + mark_width(projects, tab),
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
