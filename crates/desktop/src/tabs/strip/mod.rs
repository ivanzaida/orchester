//! The tab strip in the title bar (`tfnSm` in `QQmsI`, `N4qch` in `V70DN`; states `UZF1Q`). The
//! tabs start at x 216 on both platforms and on every page, over the content column: the area over
//! the sidebar is always the window's drag area (on macOS with the traffic lights in it). The +
//! follows the last tab, then at least 48px of drag area before the window controls. Tabs shrink,
//! then scroll with the tab list button after them ([`layout`]); a press moved 4px drags a tab
//! along the strip to reorder it ([`drag`]). While a dialog is open the tabs, + and tab list do
//! not react; the drag areas and window controls do.

mod drag;
pub mod layout;
mod list;
mod tab;

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, Placement, Timeout},
    events::{KeyboardEvent, MouseEvent, ScrollEvent},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Rect, Row, Spacer, Stack, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Gradient, HitTestBehavior, Style, border::Border, dimension::Dimension},
};
use parking_lot::Mutex;

use self::{
  drag::Drag,
  layout::StripLayout,
  list::{LIST_SLOT_WIDTH, list_button, list_menu},
  tab::{TAB_HEIGHT, TabHandlers, TabView, tab},
};
use super::{
  Tabs,
  feed::{Failures, ProjectFeeds, WindowFeed},
  label::{TabLabel, label},
  model::{TabId, TabSet},
  window::LEADING_WIDTH,
};
use crate::{
  tasks::overview::OverviewTab,
  theme::{ExtraColor, ExtraShadow, ExtraSpacing, TITLE_BAR_HEIGHT},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// The + (`j7aSu`) and its slot (`K3W9DL`: 4 on each side), the drag area that always stays after
/// it (`Q5v1g`), the divider between two inactive tabs (`e8S1q`), the fades on a clipped edge
/// (`xpSC0`) and how long the pointer rests on a tab before its tooltip shows.
const NEW_TAB_BUTTON: f32 = 24.0;
const NEW_TAB_SLOT: f32 = 32.0;
const MIN_DRAG_AREA: f32 = 48.0;
const DIVIDER_WIDTH: f32 = 1.0;
const DIVIDER_HEIGHT: f32 = 14.0;
const FADE_WIDTH: f32 = 24.0;
const TOOLTIP_DELAY: Duration = Duration::from_millis(600);
/// Tooltips hang 8px in from the tab's left edge and 6px under it (`S6PTa`).
const TOOLTIP_OFFSET: (f32, f32) = (8.0, 6.0);

/// A tab's label, and why its project wants attention.
pub(super) struct Labeled {
  pub(super) id: TabId,
  pub(super) label: TabLabel,
  pub(super) attention: Option<Arc<str>>,
}

pub(super) struct TabStrip {
  hovered: Signal<Option<TabId>>,
  close_hovered: Signal<Option<TabId>>,
  /// The tab whose tooltip shows, once the pointer rested on it.
  tooltip: Signal<Option<TabId>>,
  tooltip_timer: Timeout,
  list_open: Signal<bool>,
  scroll: Signal<f32>,
  drag: Signal<Option<Drag>>,
  /// The active tab at the last render: switching tabs scrolls the new one into view once.
  shown_active: Mutex<Option<TabId>>,
  tooltip_anchor: ElementRef,
  list_anchor: ElementRef,
}

impl Component for TabStrip {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let (hovered, tooltip) = (ctx.signal(None), ctx.signal(None));
    let (resting, shown) = (hovered.clone(), tooltip.clone());
    let tooltip_timer = ctx.create_timeout(TOOLTIP_DELAY, move || shown.set(resting.get_untracked()));
    Self {
      hovered,
      close_hovered: ctx.signal(None),
      tooltip,
      tooltip_timer,
      list_open: ctx.signal(false),
      scroll: ctx.signal(0.0),
      drag: ctx.signal(None),
      shown_active: Mutex::new(None),
      tooltip_anchor: ElementRef::default(),
      list_anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tabs = ctx
      .use_context::<Tabs>()
      .expect("the window provides Tabs before mounting the tab strip");
    let set = tabs.set().get();
    let inert = tabs.dialogs().is_open();
    if inert && self.list_open.get_untracked() {
      self.list_open.set(false);
    }
    let window_width = ctx.window().logical_width();
    let layout = StripLayout::new(set.len(), set.active_index(), strip_room(window_width), LIST_SLOT_WIDTH);
    let labels = labels(ctx, &tabs, &set);
    let scroll = self.scroll_for(&layout, set.active(), set.active_index());
    let strip = self.strip(&tabs, &set, (&layout, scroll), &labels, inert);
    let mut row = Row::new()
      .id("window-tab-strip")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(TITLE_BAR_HEIGHT))
      .align_items(Alignment::End)
      .child(super::window::leading(ctx, LEADING_WIDTH))
      .child(strip);
    if layout.overflow {
      row = row.child(list_button(set.len(), &self.list_open, &self.list_anchor, !inert));
    }
    row = row
      .child(new_tab_button(&tabs, !inert))
      .child(Spacer::new().flex(1.0).min_width(Dimension::Px(MIN_DRAG_AREA)));
    if layout.overflow && self.list_open.get() {
      let rows = list::rows(&labels, set.active());
      row = row.child(list_menu(&tabs, &rows, &self.list_open, &self.list_anchor));
    }
    let cancel = self.drag.clone();
    row
      .with_children(self.tooltip_popup(&labels, inert))
      // Escape puts a dragged tab back.
      .on_key_down(move |event: KeyboardEvent| {
        if event.key == "Escape" && cancel.get_untracked().is_some() {
          cancel.set(None);
          event.prevent_default();
        }
      })
  }
}

impl TabStrip {
  /// The strip's scroll offset: kept within the tabs, and moved to show the active tab when it
  /// changed since the last render.
  fn scroll_for(&self, layout: &StripLayout, active: TabId, index: usize) -> f32 {
    let mut shown = self.shown_active.lock();
    let scroll = self.scroll.get();
    let scroll = if *shown == Some(active) {
      layout.clamp_scroll(scroll)
    } else {
      *shown = Some(active);
      layout.scroll_to(index, scroll)
    };
    if scroll != self.scroll.get_untracked() {
      self.scroll.set(scroll);
    }
    scroll
  }

  fn strip(
    &self,
    tabs: &Tabs,
    set: &TabSet,
    (layout, scroll): (&StripLayout, f32),
    labels: &[Labeled],
    inert: bool,
  ) -> Element {
    let drag = self.drag.get();
    let places = drag::places(layout, set, drag);
    let pointer = (self.hovered.get(), self.close_hovered.get());
    let tooltip = self.tooltip.get();
    let mut stack = Stack::new()
      .id("window-tab-viewport")
      .width(Dimension::Px(layout.viewport))
      .height(Dimension::Px(TAB_HEIGHT))
      .clip();
    // Every tab keeps its slot and every gap its divider, shown or not, so the strip's children
    // never shift under the pointer between a press and its release.
    let mut dividers = Vec::new();
    for (index, labeled) in labels.iter().enumerate() {
      let id = labeled.id;
      let x = places[index] - scroll;
      let slot = Column::new()
        .id(format!("window-tab-slot-{id}"))
        .absolute_position(x, 0.0);
      // The dragged tab leaves its place empty; it is drawn lifted after the others.
      if drag.is_some_and(|drag| drag.tab == id) {
        stack = stack.child(slot);
        continue;
      }
      if index > 0 {
        let shown = drag.is_none() && divides(set, (labels[index - 1].id, id), pointer.0);
        dividers.push(divider(id, x, shown));
      }
      let view = view(set, (index, labeled), layout, pointer);
      let handlers = (!inert).then(|| self.handlers(tabs, id, index, layout));
      let anchor = (tooltip == Some(id)).then_some(&self.tooltip_anchor);
      stack = stack.child(slot.child(tab(&view, anchor, handlers.as_ref())));
    }
    // Over the tabs: an inactive tab's canvas fill would hide the divider on its edge.
    stack = stack.with_children(dividers);
    if let Some(drag) = drag {
      stack = stack.child(lifted(set, layout, labels, drag, scroll));
    }
    stack = stack.with_children(fades(layout, scroll));
    if inert {
      return stack.into();
    }
    let (wheel, bounds) = (self.scroll.clone(), layout.clone());
    stack
      .on_scroll(move |event: ScrollEvent| {
        let moved = bounds.clamp_scroll(wheel.get_untracked() - event.delta_x - event.delta_y);
        wheel.set(moved);
        event.prevent_default();
      })
      .into()
  }

  fn handlers(&self, tabs: &Tabs, id: TabId, index: usize, layout: &StripLayout) -> TabHandlers {
    let (activate, close) = (tabs.clone(), tabs.clone());
    let (hovered, tooltip, timer) = (self.hovered.clone(), self.tooltip.clone(), self.tooltip_timer.clone());
    let close_hovered = self.close_hovered.clone();
    TabHandlers {
      activate: Arc::new(move || activate.activate(id)),
      close: Arc::new(move || close.request_close(id)),
      hover: Arc::new(move |over| {
        tooltip.set(None);
        if over {
          hovered.set(Some(id));
          timer.restart();
        } else if hovered.get_untracked() == Some(id) {
          hovered.set(None);
          timer.cancel();
        }
      }),
      hover_close: Arc::new(move |over| {
        if over {
          close_hovered.set(Some(id));
        } else if close_hovered.get_untracked() == Some(id) {
          close_hovered.set(None);
        }
      }),
      drag_move: self.clearing_hover(drag::on_move(&self.drag, id)),
      drag_end: self.clearing_hover(drag::on_end(&self.drag, tabs, (id, index), layout.clone())),
    }
  }

  /// `handle` after clearing the pointer's place: a dragged tab moves away under the pointer and
  /// is rebuilt in its new slot, so it gets no leave of its own and would keep its hover look.
  fn clearing_hover(&self, handle: Arc<dyn Fn(f32) + Send + Sync>) -> Arc<dyn Fn(f32) + Send + Sync> {
    let (hovered, close_hovered, tooltip, timer) = (
      self.hovered.clone(),
      self.close_hovered.clone(),
      self.tooltip.clone(),
      self.tooltip_timer.clone(),
    );
    Arc::new(move |value| {
      timer.cancel();
      for signal in [&hovered, &close_hovered, &tooltip] {
        if signal.get_untracked().is_some() {
          signal.set(None);
        }
      }
      handle(value);
    })
  }

  /// The resting tab's tooltip: its full location, or why its project wants attention.
  fn tooltip_popup(&self, labels: &[Labeled], inert: bool) -> Option<Element> {
    let shown = self.tooltip.get().filter(|_| !inert && self.drag.get().is_none())?;
    let labeled = labels.iter().find(|labeled| labeled.id == shown)?;
    let text = labeled.attention.as_ref().unwrap_or(&labeled.label.full);
    let panel = Column::new()
      .id("window-tab-tooltip")
      .padding_vertical(ExtraSpacing::Tight.role())
      .padding_horizontal(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .box_shadow(ExtraShadow::Popover.role())
      .child(
        Text::new(text)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      );
    Some(
      Popup::new(self.tooltip_anchor.clone(), panel)
        .open_when(true)
        .placement(Placement::BottomStart)
        .offset(TOOLTIP_OFFSET.0, TOOLTIP_OFFSET.1)
        .hit_test(HitTestBehavior::None)
        .dismiss_on_outside_click(false)
        .dismiss_on_escape(false)
        .into(),
    )
  }
}

/// How tab `index` looks, from where the pointer is (over a tab, over its close button).
fn view(
  set: &TabSet,
  (index, labeled): (usize, &Labeled),
  layout: &StripLayout,
  (hovered, close_hovered): (Option<TabId>, Option<TabId>),
) -> TabView {
  let id = labeled.id;
  let active = id == set.active();
  TabView {
    id,
    label: labeled.label.clone(),
    active,
    hovered: hovered == Some(id),
    close_hovered: close_hovered == Some(id),
    dragged: false,
    close_slot: set.closable(id) && (active || layout.inactive_close),
    attention: labeled.attention.is_some(),
    width: layout.tabs[index].1,
  }
}

/// The dragged tab, lifted, following the pointer along the strip.
fn lifted(set: &TabSet, layout: &StripLayout, labels: &[Labeled], drag: Drag, scroll: f32) -> Element {
  let Some(index) = set.index(drag.tab) else {
    return Column::new().into();
  };
  let (x, width) = layout.tabs[index];
  let x = drag::lifted_x(layout, x + drag.dx, width) - scroll;
  let view = TabView {
    dragged: true,
    ..view(set, (index, &labels[index]), layout, (None, None))
  };
  Column::new()
    .absolute_position(x, 0.0)
    .child(tab(&view, None, None))
    .into()
}

/// Each tab's label and why its project wants attention, in tab order.
fn labels(ctx: &mut Ctx, tabs: &Tabs, set: &TabSet) -> Vec<Labeled> {
  let feeds = ctx
    .use_context::<ProjectFeeds>()
    .expect("the window provides the project feeds");
  let failures = WindowFeed::of(ctx).failures.get();
  set
    .tabs()
    .iter()
    .map(|tab| {
      let label = label(ctx, tab.path(), tabs.places(), &feeds);
      Labeled {
        id: tab.id(),
        attention: attention(ctx, &feeds, &label, &failures),
        label,
      }
    })
    .collect()
}

/// Why the tab's project wants attention: runs that need the user (the status bar's Needs you),
/// or an unresolved failure (status bar state H), after the tab's label.
fn attention(ctx: &Ctx, feeds: &ProjectFeeds, label: &TabLabel, failures: &Failures) -> Option<Arc<str>> {
  let (project, name) = (label.project?, label.project_name.as_deref()?);
  let tasks = feeds.of(project).tasks.get();
  let needs_you = tasks
    .ready()
    .map_or(0, |tasks| OverviewTab::NeedsYou.count(&tasks.tasks, &tasks.phases));
  let location = &*label.text;
  match needs_you {
    0 => {}
    1 => return Some(ctx.t_args("tabs.attention.needsYou.one", [("location", location)])),
    count => {
      let count = count.to_string();
      return Some(ctx.t_args(
        "tabs.attention.needsYou.other",
        [("location", location), ("count", count.as_str())],
      ));
    }
  }
  let failure = message_text(ctx, failures.of_project(name)?);
  Some(ctx.t_args(
    "tabs.attention.failure",
    [("location", location), ("failure", &*failure)],
  ))
}

/// The width the tabs (and the tab list button) may take in a window `width` wide.
fn strip_room(width: f32) -> f32 {
  width - LEADING_WIDTH - super::window::controls_width() - NEW_TAB_SLOT - MIN_DRAG_AREA
}

/// Whether a divider stands between `left` and `right`: only between two inactive tabs the pointer
/// is not over.
fn divides(set: &TabSet, (left, right): (TabId, TabId), hovered: Option<TabId>) -> bool {
  [left, right]
    .into_iter()
    .all(|id| id != set.active() && Some(id) != hovered)
}

/// The divider before tab `id` (`e8S1q`): 1 × 14 `$border` on its left edge, `shown` only between
/// two inactive tabs the pointer is not over.
fn divider(id: TabId, x: f32, shown: bool) -> Element {
  let top = (TAB_HEIGHT - DIVIDER_HEIGHT) / 2.0;
  Column::new()
    .id(format!("window-tab-divider-{id}"))
    .absolute_position(x, top)
    .opacity(if shown { 1.0 } else { 0.0 })
    .hit_test(HitTestBehavior::None)
    .child(Rect::new(Dimension::Px(DIVIDER_WIDTH), Dimension::Px(DIVIDER_HEIGHT)).background(PaletteColor::Border))
    .into()
}

/// The 24px fades to `$canvas` on each clipped edge of a scrolled strip (`xpSC0`).
fn fades(layout: &StripLayout, scroll: f32) -> Vec<Element> {
  let mut fades = Vec::new();
  if !layout.overflow {
    return fades;
  }
  if scroll > 0.0 {
    fades.push(fade(0.0, true));
  }
  if scroll < layout.content - layout.viewport {
    fades.push(fade(layout.viewport - FADE_WIDTH, false));
  }
  fades
}

fn fade(x: f32, left: bool) -> Element {
  let (from, to, id) = if left {
    (
      PaletteColor::SurfaceBase,
      ExtraColor::CanvasClear.role(),
      "window-tab-fade-left",
    )
  } else {
    (
      ExtraColor::CanvasClear.role(),
      PaletteColor::SurfaceBase,
      "window-tab-fade-right",
    )
  };
  Row::new()
    .id(id)
    .absolute(x, 0.0, Dimension::Px(FADE_WIDTH), Dimension::Px(TAB_HEIGHT))
    .background_gradient(Gradient::linear(90.0, [from, to]))
    .hit_test(HitTestBehavior::None)
    .into()
}

/// The + (`K3W9DL`): a new tab on All projects.
fn new_tab_button(tabs: &Tabs, enabled: bool) -> Element {
  let mut button = Row::new()
    .id("window-new-tab")
    .size(Dimension::Px(NEW_TAB_BUTTON), Dimension::Px(NEW_TAB_BUTTON))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm)
    .child(icon(Icon::Plus, IconSize::Dismiss, PaletteColor::TextSecondary));
  if enabled {
    let tabs = tabs.clone();
    button = button
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_mouse_down(|event: MouseEvent| {
        event.prevent_default();
        event.stop_propagation();
      })
      .on_click(move |_: MouseEvent| tabs.new_tab());
  }
  Row::new()
    .height(Dimension::Px(TAB_HEIGHT))
    .padding_horizontal(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(button)
    .into()
}
