//! How wide the title bar's tabs may get (`oHEGQ`, `YAOSy`). A labelled tab is as wide as what it shows (lurq's
//! `WindowTab::fit_content`) up to a cap: the active tab's label up to [`LABEL_MAX_WIDTH`], the inactive ones' up to
//! a cap that shrinks as tabs open, from that down to [`LABEL_MIN_WIDTH`] (a 120 wide tab). Past that every inactive
//! labelled tab is capped at 120, its label cut or not (`Tab 2 · Kontur · Overview` in `oHEGQ`), and lurq's window tabs
//! move the ones that do not fit, the last first and never the active one, into the menu of the button after the
//! tabs that counts them. The caps count each label at its cap and each tab's marks as the design draws them, since
//! no text is measured before layout. The strip itself is never wider than the bar leaves it beside the room that
//! drags the window.

use crate::{
  chrome::{BAR_GAP, END_RESERVE, LEAD_IN},
  theme::kit::window_tabs::{CLOSE_SIDE, DOT_SIDE, NEW_TAB_SIDE, gap},
};

/// A tab at the window's home but the active one at All projects (`Tab/Home` `qqzuJ`): the folders icon alone.
pub const ICON_TAB_WIDTH: f32 = 28.0;
/// The drag area that always stays between the tabs and the *N need you* pill.
pub const MIN_DRAG_WIDTH: f32 = 48.0;
/// The widest an active or roomy tab's label gets, and the narrowest an inactive one's (`Kontur · Ove…`).
pub const LABEL_MAX_WIDTH: f32 = 160.0;
pub const LABEL_MIN_WIDTH: f32 = 81.0;
/// What a labelled tab without a close button adds around its label: 10 at each side, the 13 icon and the 6 after
/// it (`Tab/Inactive` `mxo09`).
const INACTIVE_CHROME_WIDTH: f32 = 10.0 + 13.0 + 6.0 + 10.0;
/// What the active tab adds: 10 before the 13 icon, 6 gaps, the close button and 6 after it (`Tab/Active` `jhz5t`).
const ACTIVE_CHROME_WIDTH: f32 = 10.0 + 13.0 + 6.0 + 6.0 + CLOSE_SIDE + 6.0;
/// An inactive labelled tab at its narrowest (`Tab 2 · Kontur · Overview` in `oHEGQ`).
pub const TAB_MIN_WIDTH: f32 = LABEL_MIN_WIDTH + INACTIVE_CHROME_WIDTH;
/// What a tab's unsaved dot adds with the gap before it.
pub const DOT_WIDTH: f32 = 6.0 + DOT_SIDE;
/// What a tab's red alert (11) adds with the gap before it.
pub const MARK_WIDTH: f32 = 6.0 + 11.0;
/// What a tab's ✋ count adds before its digits: the gap before it, the 12 hand and the 4 after it, as the Needs you
/// panel's count of a project draws them (`Count` `ruG7r` in `o3xekF`); a digit in Inter 11.5 medium is about 0.62 em.
const BADGE_CHROME_WIDTH: f32 = 6.0 + 12.0 + 4.0;
const BADGE_DIGIT_WIDTH: f32 = 11.5 * 0.62;
/// The *N need you* pill as the design draws it (`Needs you` `v2g7Gc`), for while the pill shows but has not been
/// laid out yet.
pub const NEEDS_YOU_WIDTH: f32 = 106.0;
/// What the title bar takes besides the strip and the *N need you* pill: the bar's start ([`LEAD_IN`]: the macOS
/// lights), the app menu and divider (61), the gaps before and after the strip, the drag area, the gap before the
/// pill and the bar's end ([`END_RESERVE`]: the Windows controls).
const TITLE_BAR_RESERVE: f32 = LEAD_IN + 61.0 + 3.0 * BAR_GAP + MIN_DRAG_WIDTH + END_RESERVE;

/// What a tab shows besides its label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
  /// The folders icon alone.
  Icon,
  /// A label after its icon, with marks this much wider than the bare tab ([`DOT_WIDTH`], [`MARK_WIDTH`],
  /// [`badge_width`]).
  Labelled(f32),
}

impl Slot {
  /// Its width with its label `label` wide; the active tab counts at its widest.
  pub fn width(self, label: f32, active: bool) -> f32 {
    match self {
      Self::Icon => ICON_TAB_WIDTH,
      Self::Labelled(marks) if active => LABEL_MAX_WIDTH + ACTIVE_CHROME_WIDTH + marks,
      Self::Labelled(marks) => label + INACTIVE_CHROME_WIDTH + marks,
    }
  }
}

/// The most a labelled tab is wide: its label at `cap`, its marks `marks` wide, and the close button where it has
/// one. Narrower content leaves it narrower.
pub fn tab_max(cap: f32, marks: f32, close: bool) -> f32 {
  let chrome = if close {
    ACTIVE_CHROME_WIDTH
  } else {
    INACTIVE_CHROME_WIDTH
  };
  chrome + cap + marks
}

/// What a tab's ✋ count of `count` things waiting adds to it.
pub fn badge_width(count: usize) -> f32 {
  BADGE_CHROME_WIDTH + digits(count) as f32 * BADGE_DIGIT_WIDTH
}

/// The most the strip may be in a window `window_width` wide whose title bar ends in `trailing` (the *N need you*
/// pill, 0 while it does not show).
pub fn strip_room(window_width: f32, trailing: f32) -> f32 {
  window_width - TITLE_BAR_RESERVE - trailing
}

/// The room for the tabs in a strip `strip` wide: what *New tab* and the gap before it leave.
pub fn room(strip: f32) -> f32 {
  strip - gap() - NEW_TAB_SIDE
}

/// How wide the inactive tabs' labels get when tabs shaped as `slots`, the one at `active` active, share `room` with
/// the gaps between them: the widest that lets them all fit, at most [`LABEL_MAX_WIDTH`]; `None` once they do not fit
/// at [`LABEL_MIN_WIDTH`], when every inactive labelled tab is capped at [`TAB_MIN_WIDTH`] and its marks.
pub fn label_cap(room: f32, slots: &[Slot], active: usize) -> Option<f32> {
  let gaps = gap() * slots.len().saturating_sub(1) as f32;
  let (labelled, fixed) =
    slots
      .iter()
      .enumerate()
      .fold((0_usize, gaps), |(labelled, fixed), (index, slot)| match slot {
        Slot::Labelled(_) if index != active => (labelled + 1, fixed + slot.width(0.0, false)),
        _ => (labelled, fixed + slot.width(LABEL_MAX_WIDTH, index == active)),
      });
  if labelled == 0 {
    return Some(LABEL_MAX_WIDTH);
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
