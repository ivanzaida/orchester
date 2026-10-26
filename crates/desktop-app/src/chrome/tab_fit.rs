//! How the tabs share the room the title bar leaves them (`oHEGQ`, `YAOSy`): while they fit, labelled
//! tabs keep their natural width with their labels capped, the inactive ones' cap shrinking as tabs open
//! down to [`LABEL_MIN_WIDTH`] (a 120 wide tab). The active tab never shrinks: its label keeps up to
//! [`LABEL_MAX_WIDTH`]. Once the inactive tabs would go below that, they stay at their narrowest, the tabs
//! that do not fit leave the strip, and the tab list button with the count of those takes their place
//! before *New tab*. The tabs that stay are the longest run from the left that fits beside the active
//! one; an active tab out of that run takes the last place after it.

use crate::theme::ExtraSpacing;

/// An All projects tab (`All projects tab` `TvmmP`): the folders icon alone.
pub const ICON_TAB_WIDTH: f32 = 28.0;
/// The drag area that always stays between *New tab* and the window controls.
pub const MIN_DRAG_WIDTH: f32 = 48.0;
/// The widest an active or roomy tab's label gets, and the narrowest an inactive one's (`Kontur · Ove…`).
pub const LABEL_MAX_WIDTH: f32 = 160.0;
pub const LABEL_MIN_WIDTH: f32 = 81.0;
/// What an inactive labelled tab adds around its label: 10 at each side, the 13 icon and the 6 after it
/// (`Tab/inactive` `KSWzU`).
const INACTIVE_CHROME_WIDTH: f32 = 10.0 + 13.0 + 6.0 + 10.0;
/// What the active tab adds: 10 before the 13 icon, 6 gaps, the 12 close button and 6 after it
/// (`Tab/active` `ytOjT`).
const ACTIVE_CHROME_WIDTH: f32 = 10.0 + 13.0 + 6.0 + 6.0 + 12.0 + 6.0;
/// An inactive labelled tab at its narrowest (`Tab 2 · Kontur · Overview` in `oHEGQ`).
pub const TAB_MIN_WIDTH: f32 = LABEL_MIN_WIDTH + INACTIVE_CHROME_WIDTH;
/// What a tab's unsaved dot (6) and its mark (11) add with the gap before each.
pub const DOT_WIDTH: f32 = 6.0 + 6.0;
pub const MARK_WIDTH: f32 = 11.0 + 6.0;
/// What a tab's ✋ count adds before its digits: the gap before it, the 12 hand and the 4 after it, as the Needs you
/// panel's count of a project draws them (`Count` `ruG7r` in `o3xekF`); a digit in Inter 11.5 medium is about 0.62 em.
const BADGE_CHROME_WIDTH: f32 = 6.0 + 12.0 + 4.0;
const BADGE_DIGIT_WIDTH: f32 = 11.5 * 0.62;
/// The tab list button around its count (`Tab list button` `Dq1Mf`): 8 before the count, 4 after it, the
/// 12 chevron and 6 after that; a digit of the count in Geist Mono 11 is 0.6 em.
const BUTTON_CHROME_WIDTH: f32 = 8.0 + 4.0 + 12.0 + 6.0;
const COUNT_DIGIT_WIDTH: f32 = 11.0 * 0.6;
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
  /// A tab this wide as it was laid out: the active tab once lurq measured it, so it takes its real room.
  Measured(f32),
}

impl Slot {
  /// Its width with its label `label` wide; an active tab not measured yet counts at its widest, since it
  /// never shrinks and lurq measures no text before layout.
  pub fn width(self, label: f32, active: bool) -> f32 {
    match self {
      Self::Icon => ICON_TAB_WIDTH,
      Self::Measured(width) => width,
      Self::Labelled(marks) if active => LABEL_MAX_WIDTH + ACTIVE_CHROME_WIDTH + marks,
      Self::Labelled(marks) => label + INACTIVE_CHROME_WIDTH + marks,
    }
  }
}

/// How the tabs share their room: how wide inactive labels get, and which tabs leave the strip.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
  label: f32,
  hidden: Vec<usize>,
}

impl Fit {
  /// The widest an inactive tab's label gets; the active tab's is [`LABEL_MAX_WIDTH`].
  pub const fn label_width(&self) -> f32 {
    self.label
  }

  /// The indexes of the tabs that are only in the tab list, in order.
  pub fn hidden(&self) -> &[usize] {
    &self.hidden
  }

  /// Whether some tabs are only in the tab list, so its button shows.
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

/// The tab list button's width with a count of `count` hidden tabs.
pub fn button_width(count: usize) -> f32 {
  BUTTON_CHROME_WIDTH + digits(count) as f32 * COUNT_DIGIT_WIDTH
}

/// What a tab's ✋ count of `count` things waiting adds to it.
pub fn badge_width(count: usize) -> f32 {
  BADGE_CHROME_WIDTH + digits(count) as f32 * BADGE_DIGIT_WIDTH
}

/// How tabs shaped as `slots`, the one at `active` active, share `room`.
pub fn fit(room: f32, slots: &[Slot], active: usize) -> Fit {
  let every: Vec<usize> = (0..slots.len()).collect();
  if let Some(label) = label_width(room, slots, (&every, active)) {
    return Fit {
      label,
      hidden: Vec::new(),
    };
  }
  // The button counts at most every tab but the active one, which never hides; once the count is known,
  // a narrower count may leave room for another tab.
  let hidden = |count: usize| {
    let shown = shown(room - button_width(count) - gap(), slots, active);
    every
      .iter()
      .copied()
      .filter(|index| !shown.contains(index))
      .collect::<Vec<_>>()
  };
  let most = hidden(slots.len().saturating_sub(1));
  let hidden = if digits(most.len()) < digits(slots.len().saturating_sub(1)) {
    hidden(most.len())
  } else {
    most
  };
  Fit {
    label: LABEL_MIN_WIDTH,
    hidden,
  }
}

/// The tabs that show in `room` with inactive tabs at their narrowest: the active one, then the others
/// in order while they fit.
fn shown(room: f32, slots: &[Slot], active: usize) -> Vec<usize> {
  let narrowest = |index: usize| {
    slots
      .get(index)
      .map_or(0.0, |slot| slot.width(LABEL_MIN_WIDTH, index == active))
  };
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

/// How wide the inactive labels of the tabs `shown` get in `room` beside the tab at `active`; `None`
/// when that is narrower than [`LABEL_MIN_WIDTH`]. Without an inactive labelled tab the cap is the widest.
fn label_width(room: f32, slots: &[Slot], (shown, active): (&[usize], usize)) -> Option<f32> {
  let gaps = gap() * shown.len().saturating_sub(1) as f32;
  let (labelled, fixed) = shown
    .iter()
    .filter_map(|index| slots.get(*index).map(|slot| (*index, slot)))
    .fold((0_usize, gaps), |(labelled, fixed), (index, slot)| match slot {
      Slot::Labelled(_) if index != active => (labelled + 1, fixed + slot.width(0.0, false)),
      _ => (labelled, fixed + slot.width(LABEL_MAX_WIDTH, index == active)),
    });
  if labelled == 0 {
    return (fixed <= room).then_some(LABEL_MAX_WIDTH);
  }
  let label = (room - fixed) / labelled as f32;
  (label >= LABEL_MIN_WIDTH).then_some(label.min(LABEL_MAX_WIDTH))
}

/// The digits of `count`, at least one.
const fn digits(count: usize) -> u32 {
  match count.checked_ilog10() {
    Some(log) => log + 1,
    None => 1,
  }
}

/// The gap between tabs.
pub const fn gap() -> f32 {
  ExtraSpacing::Tight.value()
}
