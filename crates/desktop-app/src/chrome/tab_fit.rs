//! How the tabs share the room the title bar leaves them (`docs/design/orchester-workspace.md`, "Tabs" ›
//! "Overflow"): while they fit, labelled tabs keep their natural width with their labels capped, the cap
//! shrinking as tabs open; once the cap would go below [`LABEL_MIN_WIDTH`], every labelled tab is
//! [`TAB_MIN_WIDTH`] wide and the strip scrolls. Then each tab's place is known, which is what keeps the
//! active tab in view.

use crate::{
  navigation::tabs::{Tab, TabSet},
  routes::Route,
  theme::ExtraSpacing,
};

/// An All projects tab (`All projects tab` `TvmmP`): the folders icon alone.
pub const ICON_TAB_WIDTH: f32 = 28.0;
/// The drag area that always stays between *New tab* and the window controls.
pub const MIN_DRAG_WIDTH: f32 = 48.0;
/// The widest and the narrowest a tab's label gets as tabs open.
const LABEL_MAX_WIDTH: f32 = 160.0;
const LABEL_MIN_WIDTH: f32 = 56.0;
/// What a labelled tab adds around its label: padding, icon, gaps and close button.
const TAB_CHROME_WIDTH: f32 = 48.0;
/// A labelled tab once the tabs no longer fit.
pub const TAB_MIN_WIDTH: f32 = LABEL_MIN_WIDTH + TAB_CHROME_WIDTH;
/// The window controls: Windows' three 46 wide; lurq's macOS traffic lights, 8 and three 20 wide.
#[cfg(not(target_os = "macos"))]
const CONTROLS_WIDTH: f32 = 3.0 * 46.0;
#[cfg(target_os = "macos")]
const CONTROLS_WIDTH: f32 = 8.0 + 3.0 * 20.0;
/// The *N need you* pill and the room after it (`Trailing` `AqLkT`).
const NEEDS_YOU_WIDTH: f32 = 106.0;
/// What the title bar takes besides the tabs: the app menu and divider (61), the strip's start (8),
/// *New tab* with its gaps (30), the drag area, the *N need you* pill and the window controls.
const TITLE_BAR_RESERVE: f32 = 61.0 + 8.0 + 30.0 + MIN_DRAG_WIDTH + NEEDS_YOU_WIDTH + CONTROLS_WIDTH;

/// How the tabs share their room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
  /// Every tab shows; labels are at most this wide.
  Shrunk(f32),
  /// Labelled tabs are [`TAB_MIN_WIDTH`] wide and the strip scrolls in about this much room.
  Scrolling(f32),
}

/// Whether a tab shows its icon alone.
pub fn is_icon(tab: &Tab) -> bool {
  *tab.route() == Route::AllProjects
}

/// How `tabs` fit a window `window_width` wide.
pub fn fit(window_width: f32, tabs: &TabSet) -> Fit {
  let count = tabs.tabs().len();
  let icons = tabs.tabs().iter().filter(|tab| is_icon(tab)).count();
  let labelled = count - icons;
  let room = window_width - TITLE_BAR_RESERVE;
  if labelled == 0 {
    return Fit::Shrunk(LABEL_MAX_WIDTH);
  }
  let gaps = gap() * count.saturating_sub(1) as f32;
  let label = (room - icons as f32 * ICON_TAB_WIDTH - gaps) / labelled as f32 - TAB_CHROME_WIDTH;
  if label >= LABEL_MIN_WIDTH {
    Fit::Shrunk(label.min(LABEL_MAX_WIDTH))
  } else {
    Fit::Scrolling(room)
  }
}

/// Where the tab at `index` is in the strip while it scrolls: its left edge and width.
pub fn span(tabs: &TabSet, index: usize) -> (f32, f32) {
  let width = |tab: &Tab| if is_icon(tab) { ICON_TAB_WIDTH } else { TAB_MIN_WIDTH };
  let left = tabs.tabs()[..index].iter().map(|tab| width(tab) + gap()).sum();
  (left, width(&tabs.tabs()[index]))
}

/// The scroll offset that shows `left..left + width` in a `viewport` wide view scrolled to `current`:
/// unchanged while it shows, else just enough to bring it in.
pub fn reveal(current: f32, viewport: f32, (left, width): (f32, f32)) -> f32 {
  if left < current {
    left
  } else if left + width > current + viewport {
    left + width - viewport
  } else {
    current
  }
}

/// The gap between tabs.
const fn gap() -> f32 {
  ExtraSpacing::Tight.value()
}
