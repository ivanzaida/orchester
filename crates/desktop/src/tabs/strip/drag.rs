//! Reordering tabs by dragging (state G `Ymhc7`): a press on a tab moved 4px along the strip lifts
//! it (horizontal only; it does not activate the tab). The other tabs slide aside to show where it
//! lands; releasing drops it there. Escape, or releasing away from the title bar, puts it back.

use std::sync::Arc;

use lurq::core::Signal;

use super::{
  super::{
    Tabs,
    model::{TabId, TabSet},
  },
  layout::StripLayout,
};
use crate::theme::TITLE_BAR_HEIGHT;

/// How far a press moves before it drags the tab.
const DRAG_THRESHOLD: f32 = 4.0;

/// The tab being dragged, `dx` from where it was pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Drag {
  pub(super) tab: TabId,
  pub(super) dx: f32,
}

impl lurq::app::component::DevtoolsInspectable for Drag {}

/// Follows the pointer once the press moved far enough.
pub(super) fn on_move(drag: &Signal<Option<Drag>>, tab: TabId) -> Arc<dyn Fn(f32) + Send + Sync> {
  let drag = drag.clone();
  Arc::new(move |dx| {
    let dragging = drag.get_untracked().is_some_and(|drag| drag.tab == tab);
    if dragging || dx.abs() >= DRAG_THRESHOLD {
      drag.set(Some(Drag { tab, dx }));
    }
  })
}

/// Drops the tab where it is over, unless the drag was put back or the pointer left the title bar.
pub(super) fn on_end(
  drag: &Signal<Option<Drag>>,
  tabs: &Tabs,
  (tab, index): (TabId, usize),
  layout: StripLayout,
) -> Arc<dyn Fn(f32) + Send + Sync> {
  let (drag, tabs) = (drag.clone(), tabs.clone());
  Arc::new(move |y| {
    let Some(ended) = drag.get_untracked().filter(|ended| ended.tab == tab) else {
      return;
    };
    drag.set(None);
    if !(0.0..=TITLE_BAR_HEIGHT).contains(&y) {
      return;
    }
    let Some(&(x, width)) = layout.tabs.get(index) else {
      return;
    };
    let to = layout.drop_index(index, lifted_x(&layout, x + ended.dx, width));
    tabs.reorder(tab, to);
  })
}

/// Where the lifted tab's left edge is: under the pointer, within the strip's content.
pub(super) fn lifted_x(layout: &StripLayout, x: f32, width: f32) -> f32 {
  x.clamp(0.0, (layout.content - width).max(0.0))
}

/// Each tab's left edge in the strip's content. While a tab is dragged, the others close up and
/// leave its width free where it would land.
pub(super) fn places(layout: &StripLayout, set: &TabSet, drag: Option<Drag>) -> Vec<f32> {
  let resting: Vec<f32> = layout.tabs.iter().map(|(x, _)| *x).collect();
  let Some(drag) = drag else {
    return resting;
  };
  let Some(from) = set.index(drag.tab) else {
    return resting;
  };
  let (x, width) = layout.tabs[from];
  let to = layout.drop_index(from, lifted_x(layout, x + drag.dx, width));
  let mut places = resting;
  let mut left = 0.0;
  let mut placed = 0;
  for (index, (_, other)) in layout.tabs.iter().enumerate() {
    if index == from {
      continue;
    }
    if placed == to {
      left += width;
    }
    places[index] = left;
    left += other;
    placed += 1;
  }
  places
}
