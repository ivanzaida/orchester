//! How the tabs share the room the title bar leaves them: while they fit, labelled tabs keep their
//! natural width with their labels capped, the cap shrinking as tabs open; once the cap would go below
//! [`LABEL_MIN_WIDTH`], the tabs that do not fit leave the strip and the tab list's chevron takes their
//! place before *New tab*. The tabs that stay are the first ones in order, and always the active one:
//! when it would be hidden it takes the place of the last ones that show. Their labels then share what
//! the shown tabs leave, as while every tab fits.

use crate::theme::ExtraSpacing;

/// An All projects tab (`All projects tab` `TvmmP`): the folders icon alone.
pub const ICON_TAB_WIDTH: f32 = 28.0;
/// The drag area that always stays between *New tab* and the window controls.
pub const MIN_DRAG_WIDTH: f32 = 48.0;
/// The widest and the narrowest a tab's label gets as tabs open.
const LABEL_MAX_WIDTH: f32 = 160.0;
pub const LABEL_MIN_WIDTH: f32 = 56.0;
/// What a labelled tab adds around its label: padding, icon, gaps and close button.
const TAB_CHROME_WIDTH: f32 = 48.0;
/// A labelled tab at its narrowest.
pub const TAB_MIN_WIDTH: f32 = LABEL_MIN_WIDTH + TAB_CHROME_WIDTH;
/// What a tab's unsaved dot (6) and its mark (11) add with the gap before each.
pub const DOT_WIDTH: f32 = 6.0 + 6.0;
pub const MARK_WIDTH: f32 = 11.0 + 6.0;
/// The tab list's chevron, as wide as *New tab*.
pub const CHEVRON_WIDTH: f32 = 26.0;
/// The window controls: Windows' three 46 wide; lurq's macOS traffic lights, 8 and three 20 wide.
#[cfg(not(target_os = "macos"))]
const CONTROLS_WIDTH: f32 = 3.0 * 46.0;
#[cfg(target_os = "macos")]
const CONTROLS_WIDTH: f32 = 8.0 + 3.0 * 20.0;
/// The *N need you* pill and the room after it as the design draws it (`Trailing` `AqLkT`), for while the
/// pill shows but has not been laid out yet.
pub const NEEDS_YOU_WIDTH: f32 = 106.0;
/// What the title bar takes besides the tabs and the *N need you* pill: the app menu and divider (61),
/// the strip's start (8), *New tab* with its gaps (30), the drag area and the window controls.
const TITLE_BAR_RESERVE: f32 = 61.0 + 8.0 + 30.0 + MIN_DRAG_WIDTH + CONTROLS_WIDTH;

/// What a tab shows besides its label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
  /// The folders icon alone.
  Icon,
  /// A label after its icon, with marks this much wider than the bare tab ([`DOT_WIDTH`],
  /// [`MARK_WIDTH`]).
  Labelled(f32),
}

impl Slot {
  /// Its width with its label `label` wide.
  fn width(self, label: f32) -> f32 {
    match self {
      Self::Icon => ICON_TAB_WIDTH,
      Self::Labelled(marks) => label + TAB_CHROME_WIDTH + marks,
    }
  }
}

/// How the tabs share their room: how wide labels get, and which tabs leave the strip.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
  label: f32,
  hidden: Vec<usize>,
}

impl Fit {
  /// The widest a label gets.
  pub const fn label_width(&self) -> f32 {
    self.label
  }

  /// The indexes of the tabs that are only in the tab list, in order.
  pub fn hidden(&self) -> &[usize] {
    &self.hidden
  }

  /// Whether some tabs are only in the tab list, so its chevron shows.
  pub fn overflows(&self) -> bool {
    !self.hidden.is_empty()
  }

  /// Whether the tab at `index` shows in the strip.
  pub fn shows(&self, index: usize) -> bool {
    !self.hidden.contains(&index)
  }
}

/// The room for the tabs and *New tab*'s neighbours in a window `window_width` wide whose title bar ends
/// in `trailing` (the *N need you* pill, 0 while it does not show).
pub fn room(window_width: f32, trailing: f32) -> f32 {
  window_width - TITLE_BAR_RESERVE - trailing
}

/// How tabs shaped as `slots`, the one at `active` active, share `room`.
pub fn fit(room: f32, slots: &[Slot], active: usize) -> Fit {
  let every: Vec<usize> = (0..slots.len()).collect();
  if let Some(label) = label_width(room, slots, &every) {
    return Fit {
      label,
      hidden: Vec::new(),
    };
  }
  let room = room - CHEVRON_WIDTH - gap();
  let shown = shown(room, slots, active);
  Fit {
    label: label_width(room, slots, &shown).unwrap_or(LABEL_MIN_WIDTH),
    hidden: every.into_iter().filter(|index| !shown.contains(index)).collect(),
  }
}

/// The tabs that show in `room` at their narrowest: the active one, then the others in order while they
/// fit.
fn shown(room: f32, slots: &[Slot], active: usize) -> Vec<usize> {
  let narrowest = |index: usize| slots.get(index).map_or(0.0, |slot| slot.width(LABEL_MIN_WIDTH));
  let mut left = room - narrowest(active);
  let mut shown = vec![active];
  for index in (0..slots.len()).filter(|index| *index != active) {
    let width = narrowest(index) + gap();
    if width > left {
      break;
    }
    left -= width;
    shown.push(index);
  }
  shown.sort_unstable();
  shown
}

/// How wide the labels of the tabs `shown` get in `room`; `None` when that is narrower than
/// [`LABEL_MIN_WIDTH`]. Without a labelled tab the cap is the widest.
fn label_width(room: f32, slots: &[Slot], shown: &[usize]) -> Option<f32> {
  let gaps = gap() * shown.len().saturating_sub(1) as f32;
  let (labelled, fixed) =
    shown
      .iter()
      .filter_map(|index| slots.get(*index))
      .fold((0_usize, gaps), |(labelled, fixed), slot| match slot {
        Slot::Icon => (labelled, fixed + ICON_TAB_WIDTH),
        Slot::Labelled(_) => (labelled + 1, fixed + slot.width(0.0)),
      });
  if labelled == 0 {
    return Some(LABEL_MAX_WIDTH);
  }
  let label = (room - fixed) / labelled as f32;
  (label >= LABEL_MIN_WIDTH).then_some(label.min(LABEL_MAX_WIDTH))
}

/// The gap between tabs.
pub const fn gap() -> f32 {
  ExtraSpacing::Tight.value()
}
