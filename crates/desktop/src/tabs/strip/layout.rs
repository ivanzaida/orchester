//! How wide each tab is and where it sits (design doc, "Tabs" › "Overflow"): tabs shrink together
//! from 224 to 120, the active tab keeps at least 160, and below 140 an inactive tab drops its
//! close button. When 120-wide tabs do not fit, the strip stops shrinking and scrolls, keeping the
//! active tab in view, and the tab list button appears after it. Pure arithmetic, so it is tested
//! without a window.

use std::cmp::Ordering;

/// Tab width at most and at least, the active tab's least width, and the least width at which an
/// inactive tab keeps its close button (`nAKyb`, state F `FtUhB`).
pub const TAB_MAX_WIDTH: f32 = 224.0;
pub const TAB_MIN_WIDTH: f32 = 120.0;
pub const ACTIVE_MIN_WIDTH: f32 = 160.0;
pub const CLOSE_MIN_WIDTH: f32 = 140.0;

#[derive(Clone, Debug, PartialEq)]
pub struct StripLayout {
  /// Each tab's left edge in the strip's content, and its width, in tab order.
  pub tabs: Vec<(f32, f32)>,
  /// Whether an inactive tab is wide enough for its close button.
  pub inactive_close: bool,
  /// The strip's visible width.
  pub viewport: f32,
  /// The width of all tabs together.
  pub content: f32,
  /// Whether the tabs do not fit: the strip scrolls and the tab list button shows.
  pub overflow: bool,
}

impl StripLayout {
  /// `count` tabs, the one at `active` active, in `available` width; `list_button` is the room the
  /// tab list button takes when the tabs overflow.
  pub fn new(count: usize, active: usize, available: f32, list_button: f32) -> Self {
    let available = available.max(0.0);
    let (inactive, overflow) = inactive_width(count, available);
    let active_width = if count == 1 {
      available.min(TAB_MAX_WIDTH)
    } else {
      inactive.clamp(ACTIVE_MIN_WIDTH, TAB_MAX_WIDTH)
    };
    let mut x = 0.0;
    let tabs = (0..count)
      .map(|index| {
        let width = if index == active { active_width } else { inactive };
        let tab = (x, width);
        x += width;
        tab
      })
      .collect();
    let viewport = if overflow {
      (available - list_button).max(0.0)
    } else {
      x
    };
    Self {
      tabs,
      inactive_close: inactive >= CLOSE_MIN_WIDTH,
      viewport,
      content: x,
      overflow,
    }
  }

  /// The scroll offset that shows the tab at `index`, starting from `scroll`: unchanged while it
  /// is in view, else just enough to bring it in.
  pub fn scroll_to(&self, index: usize, scroll: f32) -> f32 {
    let Some(&(x, width)) = self.tabs.get(index) else {
      return self.clamp_scroll(scroll);
    };
    let scroll = if x < scroll {
      x
    } else if x + width > scroll + self.viewport {
      x + width - self.viewport
    } else {
      scroll
    };
    self.clamp_scroll(scroll)
  }

  /// `scroll` kept within the content.
  pub fn clamp_scroll(&self, scroll: f32) -> f32 {
    scroll.clamp(0.0, (self.content - self.viewport).max(0.0))
  }

  /// Where a tab dragged from `from` lands when its left edge is at `x` in the content: after
  /// every other tab whose middle its middle has passed.
  pub fn drop_index(&self, from: usize, x: f32) -> usize {
    let Some(&(_, width)) = self.tabs.get(from) else {
      return from;
    };
    let middle = x + width / 2.0;
    self
      .tabs
      .iter()
      .enumerate()
      .filter(|(index, (left, other))| {
        // A tab pushed against either end of the strip lands there: its middle can only reach
        // the middle of the last tab, and of the first.
        let other_middle = left + other / 2.0;
        match index.cmp(&from) {
          Ordering::Less => other_middle < middle,
          Ordering::Greater => other_middle <= middle,
          Ordering::Equal => false,
        }
      })
      .count()
  }
}

/// The width of an inactive tab, and whether the tabs overflow at that width.
fn inactive_width(count: usize, available: f32) -> (f32, bool) {
  if count <= 1 {
    return (TAB_MAX_WIDTH, false);
  }
  let shared = available / count as f32;
  let width = if shared >= ACTIVE_MIN_WIDTH {
    shared
  } else {
    // The active tab keeps its least width; the others share the rest.
    (available - ACTIVE_MIN_WIDTH) / (count - 1) as f32
  };
  if width >= TAB_MIN_WIDTH {
    (width.min(TAB_MAX_WIDTH), false)
  } else {
    (TAB_MIN_WIDTH, true)
  }
}
