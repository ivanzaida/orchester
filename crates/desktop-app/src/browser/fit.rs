//! How the Browser window's tabs share the title bar (notes `RggGn`, *Drag area*, *Crowded strip*, *Overflow*): they
//! share the room up to *+* equally, each at most [`TAB_MAX_WIDTH`]; a crowded strip shrinks them to
//! [`TAB_MIN_WIDTH`] (the icon and the task key, `01c · Many runs` `fPkxD`) while the tab shown keeps
//! [`SHOWN_MIN_WIDTH`] for its title and ×; past that the strip scrolls sideways (`01d` `fWhIr`, `01e` `cmofl`), and
//! the tabs not wholly in view are counted on the overflow button.

use crate::theme::ExtraSpacing;

/// The widest a tab gets.
pub const TAB_MAX_WIDTH: f32 = 220.0;
/// A tab at its narrowest: its icon and the start of its task key.
pub const TAB_MIN_WIDTH: f32 = 90.0;
/// The narrowest the tab shown gets: room for its title and its ×.
pub const SHOWN_MIN_WIDTH: f32 = 150.0;
/// The line between agents' tabs and the person's (`Divider · yours` `vMuoB`).
pub const DIVIDER_WIDTH: f32 = 1.0;

/// How wide each tab of the strip is, and whether the strip scrolls.
#[derive(Clone, Debug, PartialEq)]
pub struct StripFit {
  widths: Vec<f32>,
  overflows: bool,
}

impl StripFit {
  /// The tabs' widths, in the strip's order.
  pub fn widths(&self) -> &[f32] {
    &self.widths
  }

  /// The tabs are wider than the room together: the strip scrolls and the overflow button shows.
  pub const fn overflows(&self) -> bool {
    self.overflows
  }

  /// Where the tab at `index` starts and ends in the strip, a divider before tab `divider` counted.
  pub fn span(&self, index: usize, divider: Option<usize>) -> (f32, f32) {
    let start: f32 = self.widths.iter().take(index).map(|width| width + gap()).sum();
    let start = start + divider.filter(|at| *at <= index && *at > 0).map_or(0.0, |_| DIVIDER_WIDTH + gap());
    (start, start + self.widths.get(index).copied().unwrap_or_default())
  }

  /// The whole strip's width.
  pub fn content_width(&self, divider: Option<usize>) -> f32 {
    self.span(self.widths.len().saturating_sub(1), divider).1
  }

  /// The tabs not wholly in view when the strip is scrolled to `scroll` and shows `room` of itself.
  pub fn hidden(&self, divider: Option<usize>, (scroll, room): (f32, f32)) -> Vec<usize> {
    let tolerance = 0.5;
    (0..self.widths.len())
      .filter(|index| {
        let (start, end) = self.span(*index, divider);
        start < scroll - tolerance || end > scroll + room + tolerance
      })
      .collect()
  }

  /// The scroll that brings the tab at `index` wholly into view from `scroll`, moving as little as it can.
  pub fn scroll_to(&self, index: usize, divider: Option<usize>, (scroll, room): (f32, f32)) -> f32 {
    let (start, end) = self.span(index, divider);
    let most = (self.content_width(divider) - room).max(0.0);
    let scroll = if start < scroll {
      start
    } else if end > scroll + room {
      end - room
    } else {
      scroll
    };
    scroll.clamp(0.0, most)
  }
}

/// How `count` tabs share `room`, the one at `shown` shown, with the divider before the person's tabs when both kinds
/// are open.
pub fn fit(room: f32, count: usize, shown: Option<usize>, divider: bool) -> StripFit {
  if count == 0 {
    return StripFit {
      widths: Vec::new(),
      overflows: false,
    };
  }
  let lines = if divider { DIVIDER_WIDTH + gap() } else { 0.0 };
  let free = (room - gap() * (count - 1) as f32 - lines).max(0.0);
  let equal = free / count as f32;
  if equal >= SHOWN_MIN_WIDTH || shown.is_none() || count == 1 {
    let floor = if shown.is_some() { SHOWN_MIN_WIDTH } else { TAB_MIN_WIDTH };
    let width = equal.clamp(floor, TAB_MAX_WIDTH);
    return StripFit {
      widths: vec![width; count],
      overflows: width * count as f32 > free + 0.5,
    };
  }
  let others = (free - SHOWN_MIN_WIDTH) / (count - 1) as f32;
  let width = others.max(TAB_MIN_WIDTH);
  let widths = (0..count)
    .map(|index| if Some(index) == shown { SHOWN_MIN_WIDTH } else { width })
    .collect();
  StripFit {
    widths,
    overflows: others < TAB_MIN_WIDTH,
  }
}

/// The gap between tabs (`Tab strip` `X1V8q`).
pub const fn gap() -> f32 {
  ExtraSpacing::Tight.value()
}
