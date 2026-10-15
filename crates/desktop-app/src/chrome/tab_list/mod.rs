//! The tab list (`oHEGQ`, `qWCBQ`, `a1i6Ws`, `YAOSy`): while some tabs do not fit the strip (`tab_fit`),
//! a button after the last tab that shows says how many are hidden and opens a list of every open tab in
//! the strip's order.
//!
//! The button (`Tab list button` `Dq1Mf`) is the count in Geist Mono 11 `text_secondary` and a 12
//! `chevron-down` in `text_muted`, 26 high, radius 6, 8 before and 6 after, 4 apart. Hovered it takes
//! `hover`, its count `text_primary`, and says *N more tabs* with its shortcut; open it takes `pressed`
//! with a `chevron-up` in `text_secondary` (`D9NzGQ`). It is a Tab stop as well, so the focus can come
//! back to it when the list closes.
//!
//! The list is a menu surface 320 wide (`Tab list` `pB0VB`: `surface_raised`, radius 10, `border_strong`
//! inside, padding 6, 2 apart), its start under the button's, 4 under the title bar, and 8 inside the
//! window's right edge where it would cross it. It grows with its rows up to the window's height less 48,
//! then they scroll. With more than eight tabs a search field leads it, focused, with the shortcut's keys
//! while it is empty; it matches the page and the project, and with no match the rows give way to *No
//! open tab matches “…”* (`tkm8u`). Its rows are in `row`, its keys in `keys`. A choice, Esc or the
//! shortcut give the focus back to the button.

mod keys;
mod row;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use parking_lot::Mutex;

use self::{
  keys::{ListKeys, Listing, Ring},
  row::{ListRow, RowProps},
};
use super::{
  tab_label::{page, project_name},
  tab_measure::ListHold,
  tab_strip::TAB_HEIGHT,
};
use crate::{
  navigation::{Navigation, input::FirstKeys, tabs::TabId},
  project::Projects,
  theme::{ControlHeight, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::FieldInput,
    icon::{Icon, icon},
    pressable::{Focus, in_title_bar, pressable},
    tooltip::Tooltip,
  },
};

/// The list's width (`pB0VB`).
const LIST_WIDTH: f32 = 320.0;
/// What the list leaves of the window's height.
const LIST_BOTTOM_ROOM: f32 = 48.0;
/// How far the list stays inside the window's right edge.
const WINDOW_INSET: f32 = 8.0;
/// A list of more than this many tabs leads with its search field.
const SEARCH_ABOVE: usize = 8;

/// The tabs that are only in the list, so their rows say so.
#[derive(Clone, Debug, PartialEq)]
pub struct TabListProps {
  pub hidden: Vec<TabId>,
  /// The strip's view of the list: its button's element and whether it is open.
  pub list: ListHold,
}

impl DevtoolsInspectable for TabListProps {}

/// Where the focus goes once the list has rendered.
#[derive(Clone, Copy)]
enum FocusTo {
  Button,
  Search,
}

pub struct TabList {
  open: Signal<bool>,
  query: Signal<String>,
  ring: Signal<Option<Ring>>,
  button_hovered: Signal<bool>,
  search: ElementRef,
  kept_row: ElementRef,
  listing: Arc<Mutex<Listing>>,
  focus: Arc<Mutex<Option<FocusTo>>>,
  first_keys: Option<FirstKeys>,
  /// The ringed and the current row the list last scrolled to, so it scrolls only when they change.
  scrolled_to: Mutex<Option<(Option<TabId>, Option<TabId>)>>,
}

/// A list that goes away while open leaves the window's keys to the window.
impl Drop for TabList {
  fn drop(&mut self) {
    if let Some(first) = &self.first_keys {
      first.release();
    }
  }
}

impl Component for TabList {
  type Props = TabListProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      query: ctx.signal(String::new()),
      ring: ctx.signal(None),
      button_hovered: ctx.signal(false),
      search: ctx.element_ref(),
      kept_row: ctx.element_ref(),
      listing: Arc::new(Mutex::new(Listing::default())),
      focus: Arc::new(Mutex::new(None)),
      first_keys: ctx.use_context::<FirstKeys>(),
      scrolled_to: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let open = self.open.get();
    let list = ctx.props::<TabListProps>().list.clone();
    list.set_open(open);
    let button = list.button;
    let keys = ListKeys {
      navigation: navigation.clone(),
      open: self.open.clone(),
      query: self.query.clone(),
      ring: self.ring.clone(),
      listing: self.listing.clone(),
      toggle: self.toggle(),
      dismiss: self.dismiss(),
    };
    if let Some(first) = &self.first_keys {
      if open {
        first.hold(Arc::new(keys.clone().handler()));
      } else {
        first.release();
      }
    }
    let mut root = Row::new()
      .id("tab-list")
      .on_key_down(keys.handler())
      .child(self.button(ctx, open, &button));
    if open {
      let panel = self.panel(ctx, &navigation);
      self.keep_in_view(ctx);
      root = root.child(
        Popup::new(button.clone(), panel)
          .open(self.open.clone())
          .placement(Placement::BottomStart)
          // The button is 5 above the title bar's bottom edge; the list is 4 under it.
          .offset(shift(ctx, &button), ExtraSpacing::Inline.value()),
      );
    } else {
      *self.scrolled_to.lock() = None;
    }
    match self.focus.lock().take() {
      Some(FocusTo::Button) => ctx.focus(&button),
      Some(FocusTo::Search) if open => ctx.focus(&self.search),
      _ => {}
    }
    root
  }
}

impl TabList {
  /// Opens the list fresh (no search, no ring, the search field focused) or closes it.
  fn toggle(&self) -> Arc<dyn Fn() + Send + Sync> {
    let (open, query, ring, focus) = (
      self.open.clone(),
      self.query.clone(),
      self.ring.clone(),
      self.focus.clone(),
    );
    Arc::new(move || {
      let opening = !open.get_untracked();
      query.set(String::new());
      ring.set(None);
      *focus.lock() = Some(if opening { FocusTo::Search } else { FocusTo::Button });
      open.set(opening);
    })
  }

  /// Closes the list, the focus back on its button.
  fn dismiss(&self) -> Arc<dyn Fn() + Send + Sync> {
    let (open, focus) = (self.open.clone(), self.focus.clone());
    Arc::new(move || {
      *focus.lock() = Some(FocusTo::Button);
      open.set(false);
    })
  }

  /// The button with the count of the hidden tabs (`Dq1Mf`, `D9NzGQ`), and its tooltip.
  fn button(&self, ctx: &mut Ctx, open: bool, element: &ElementRef) -> Element {
    let count = ctx.props::<TabListProps>().hidden.len();
    let hovered = self.button_hovered.get();
    let (enter, leave) = (self.button_hovered.clone(), self.button_hovered.clone());
    let toggle = self.toggle();
    let button = in_title_bar(Focus::Keyboard.apply(pressable(move || toggle())))
      .id("tab-list-button")
      .ref_element(element.clone())
      .describe("a11y_name", ctx.t("shell.tabs.list"))
      .describe("state", if open { "open" } else { "closed" })
      .height(Dimension::Px(TAB_HEIGHT))
      .flex_shrink(0.0)
      .padding(Padding::new().left(SpacingSize::Sm).right(ExtraSpacing::Snug.role()))
      .spacing(SpacingSize::Xs)
      .rounded(RadiusSize::Md)
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(
        Text::new(&count.to_string())
          .id("tab-list-count")
          .variant(ExtraTypography::MonoNote.role())
          .color(if open || hovered {
            PaletteColor::TextPrimary
          } else {
            PaletteColor::TextSecondary
          })
          .nowrap(),
      )
      .child(if open {
        icon(Icon::ChevronUp, IconSize::Sm, PaletteColor::TextSecondary)
      } else {
        icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted)
      });
    let button = if open {
      button.background(ExtraColor::Pressed.role())
    } else {
      button.hovered_style(Style::new().background(ExtraColor::Hover.role()))
    };
    let more = if count == 1 {
      "shell.tabs.more.one"
    } else {
      "shell.tabs.more.other"
    };
    let text = ctx.t_args(more, [("count", count.to_string())]);
    Tooltip::new("tab-list-button", text)
      .shortcut(list_keys(ctx))
      .silent(open)
      .wrap(ctx, button)
  }

  /// Scrolls the ringed row, or the current one, into view when either changed since the list last did:
  /// a list scrolled with the wheel stays where it is otherwise.
  fn keep_in_view(&self, ctx: &Ctx) {
    let listing = self.listing.lock().clone();
    let now = Some((listing.ringed, listing.current));
    let mut scrolled_to = self.scrolled_to.lock();
    if *scrolled_to != now {
      *scrolled_to = now;
      ctx.scroll_into_view(&self.kept_row);
    }
  }

  /// The list: the search field when it holds more than eight tabs, then the rows that match.
  fn panel(&self, ctx: &mut Ctx, navigation: &Navigation) -> Column {
    let projects = Projects::of(ctx);
    let tabs = navigation.tabs();
    let query = self.query.get();
    let hidden = ctx.props::<TabListProps>().hidden.clone();
    let searched = tabs.tabs().len() > SEARCH_ABOVE;
    let rows: Vec<RowProps> = tabs
      .tabs()
      .iter()
      .map(|tab| {
        let page: Arc<str> = page(ctx, &projects, tab.route()).into();
        let project = project_name(&projects, tab.route()).map(Arc::from);
        (tab, page, project)
      })
      .filter(|(_, page, project)| matches(&query, page, project.as_deref()))
      .map(|(tab, page, project)| RowProps {
        tab: tab.clone(),
        page,
        project,
        hidden: hidden.contains(&tab.id()),
        current: tab.id() == tabs.active().id(),
        ringed: false,
        closable: tabs.can_close(tab.id()),
        kept: None,
        choose: self.chooser(navigation.clone(), tab.id()),
      })
      .collect();
    let ringed = self.ringed(&rows, &query, searched);
    *self.listing.lock() = Listing {
      rows: rows.iter().map(|row| row.tab.id()).collect(),
      ringed,
      current: Some(tabs.active().id()),
    };
    let kept = ringed.unwrap_or(tabs.active().id());
    let rows = rows.into_iter().map(|row| RowProps {
      ringed: Some(row.tab.id()) == ringed,
      kept: (row.tab.id() == kept).then(|| self.kept_row.clone()),
      ..row
    });
    self.surface(ctx, rows.collect(), (searched, &query, tabs.tabs().len()))
  }

  /// The keyboard's row: the one the arrows chose in this search, else the first match of a search, else
  /// the current row of a list without a search field.
  fn ringed(&self, rows: &[RowProps], query: &str, searched: bool) -> Option<TabId> {
    let listed = |tab: TabId| rows.iter().any(|row| row.tab.id() == tab);
    let chosen = self
      .ring
      .get()
      .filter(|ring| ring.query == query && listed(ring.tab))
      .map(|ring| ring.tab);
    chosen.or_else(|| {
      if !query.is_empty() {
        rows.first().map(|row| row.tab.id())
      } else if searched {
        None
      } else {
        rows.iter().find(|row| row.current).map(|row| row.tab.id())
      }
    })
  }

  fn chooser(&self, navigation: Navigation, tab: TabId) -> row::Choose {
    let dismiss = self.dismiss();
    Arc::new(move || {
      navigation.select(tab);
      dismiss();
    })
  }

  /// The menu surface around the search field and the rows.
  fn surface(&self, ctx: &mut Ctx, rows: Vec<RowProps>, (searched, query, count): (bool, &str, usize)) -> Column {
    let window = ctx.window().info().logical_height();
    let search_room = if searched {
      ControlHeight::Md.value() + ExtraSpacing::Tight.value() + ctx.theme().spacing_value(SpacingSize::Xs).to_px()
    } else {
      0.0
    };
    let padding = 2.0 * ExtraSpacing::Snug.value();
    let max_rows = (window - LIST_BOTTOM_ROOM - padding - search_room).max(ControlHeight::Md.value());
    let empty = rows.is_empty();
    // Under the search field the rows follow one another 4 below it (`jRzf1`); alone, 2 apart (`VX3qJ`).
    let list = Column::new().id("tab-list-rows").width(Dimension::Pct(100.0));
    let list = if searched {
      list.padding(Padding::new().top(SpacingSize::Xs))
    } else {
      list.spacing(ExtraSpacing::Tight.role())
    };
    let list = rows.into_iter().fold(list, |list, row| {
      let key = format!("tab-list-row-{}", row.tab.id());
      list.child(ctx.mount_keyed::<ListRow>(&key, row))
    });
    let panel = Column::new()
      .id("tab-list-menu")
      .width(Dimension::Px(LIST_WIDTH))
      .padding(ExtraSpacing::Snug.role())
      .spacing(ExtraSpacing::Tight.role())
      .rounded(RadiusSize::Lg)
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
      .box_shadow(ShadowStyle::Lg)
      .with_children(searched.then(|| self.search_field(ctx, count)));
    if empty {
      return panel.child(no_match(ctx, query));
    }
    panel.child(ScrollVertical::new(list).max_height(Dimension::Px(max_rows)))
  }

  fn search_field(&self, ctx: &mut Ctx, count: usize) -> Element {
    let placeholder = ctx.t_args("shell.tabs.search", [("count", count.to_string())]);
    FieldInput::new("tab-list-search", placeholder.clone(), self.query.clone())
      .search()
      .placeholder(placeholder)
      .keys(list_keys(ctx))
      .clearable(ctx.t("shell.tabs.clearSearch"))
      .element(self.search.clone())
      .render(ctx)
  }
}

/// How far the list moves left of the start of its `button` so it ends 8 inside the window's right edge.
fn shift(ctx: &Ctx, button: &ElementRef) -> f32 {
  let window = ctx.window().info().logical_width();
  let overshoot = button.x() + LIST_WIDTH - (window - WINDOW_INSET);
  -overshoot.max(0.0)
}

/// Whether a tab at `page` in `project` matches the search `query`: either contains it, in any case.
fn matches(query: &str, page: &str, project: Option<&str>) -> bool {
  let query = query.trim().to_lowercase();
  query.is_empty()
    || page.to_lowercase().contains(&query)
    || project.is_some_and(|project| project.to_lowercase().contains(&query))
}

/// *No open tab matches “…”*, in place of the rows (`Empty` `Jmopb`).
fn no_match(ctx: &mut Ctx, query: &str) -> Element {
  Row::new()
    .id("tab-list-empty")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Loose.role()))
    .child(
      Text::new(&ctx.t_args("shell.tabs.noMatch", [("query", query.trim())]))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// The keys that open the list, as the platform writes them.
fn list_keys(ctx: &mut Ctx) -> Arc<str> {
  ctx.t(if cfg!(target_os = "macos") {
    "shell.tabs.listKeysMac"
  } else {
    "shell.tabs.listKeys"
  })
}
