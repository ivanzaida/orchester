//! How the title bar's tabs share their room (`oHEGQ`, `YAOSy`): inactive labels shrink as tabs open down
//! to a 120 wide tab while the active tab keeps its width; once that does not fit, the tabs that do not
//! leave the strip for the tab list, the active one never, and the button counts them.

use orchester_app::chrome::tab_fit::{
  Fit, ICON_TAB_WIDTH, LABEL_MAX_WIDTH, LABEL_MIN_WIDTH, MARK_WIDTH, Slot, TAB_MIN_WIDTH, button_width, fit, gap,
};

/// The active tab at its widest: a 160 label, the 13 icon, the 12 close button, their gaps and padding.
const ACTIVE_WIDTH: f32 = LABEL_MAX_WIDTH + 53.0;

fn labelled(count: usize) -> Vec<Slot> {
  vec![Slot::Labelled(0.0); count]
}

fn shown(fit: &Fit, count: usize) -> Vec<usize> {
  (0..count).filter(|index| fit.shows(*index)).collect()
}

/// The room `inactive` tabs at 120 take beside the active tab, with the gaps between them.
fn narrowest(inactive: usize) -> f32 {
  inactive as f32 * TAB_MIN_WIDTH + ACTIVE_WIDTH + inactive as f32 * gap()
}

#[test]
fn a_few_tabs_all_show_and_their_labels_shrink_as_tabs_open() {
  let few = fit(1000.0, &labelled(3), 0);
  assert!(!few.overflows());
  assert_eq!(shown(&few, 3), [0, 1, 2]);
  assert_eq!(few.label_width(), LABEL_MAX_WIDTH, "room to spare: {few:?}");
  let more = fit(1000.0, &labelled(6), 0);
  assert!(!more.overflows());
  assert!(
    more.label_width() < LABEL_MAX_WIDTH && more.label_width() > LABEL_MIN_WIDTH,
    "inactive labels shrink: {more:?}"
  );
}

#[test]
fn a_tab_is_120_wide_at_its_narrowest() {
  assert_eq!(TAB_MIN_WIDTH, 120.0);
}

#[test]
fn the_list_appears_once_inactive_tabs_at_120_do_not_fit_beside_the_active_one() {
  let room = narrowest(7);
  let at = fit(room, &labelled(8), 0);
  assert!(!at.overflows(), "seven 120 wide tabs fit beside the active one: {at:?}");
  assert_eq!(at.label_width(), LABEL_MIN_WIDTH);
  let over = fit(room - 1.0, &labelled(8), 0);
  assert!(over.overflows(), "one less and they do not: {over:?}");
  assert_eq!(over.label_width(), LABEL_MIN_WIDTH, "shown tabs stay at 120");
  let kept = shown(&over, 8).len();
  assert_eq!(over.hidden(), (kept..8).collect::<Vec<_>>(), "the last ones leave");
  let room_beside_button = room - 1.0 - button_width(7) - gap() - ACTIVE_WIDTH;
  assert_eq!(
    kept - 1,
    (room_beside_button / (TAB_MIN_WIDTH + gap())) as usize,
    "as many as fit beside the active tab and the button"
  );
}

#[test]
fn the_active_tab_always_shows_in_place_of_the_last_ones() {
  let slots = labelled(20);
  let room = narrowest(5);
  for active in 0..20 {
    let fit = fit(room, &slots, active);
    assert!(fit.shows(active), "tab {active} is active: {fit:?}");
    let others: Vec<usize> = shown(&fit, 20).into_iter().filter(|index| *index != active).collect();
    let first: Vec<usize> = (0..20).filter(|index| *index != active).take(others.len()).collect();
    assert_eq!(others, first, "the others are the first ones in order");
    assert_eq!(
      shown(&fit, 20).len() + fit.hidden().len(),
      20,
      "every tab shows or is listed"
    );
  }
}

#[test]
fn a_hidden_active_tab_takes_the_last_place_that_shows() {
  let room = narrowest(5);
  let first = shown(&fit(room, &labelled(20), 0), 20);
  let last = shown(&fit(room, &labelled(20), 19), 20);
  assert_eq!(last.len(), first.len(), "as many tabs show");
  assert_eq!(last.last(), Some(&19));
  assert_eq!(
    &last[..last.len() - 1],
    &first[..first.len() - 1],
    "the run from the left stays"
  );
}

/// The 15 tabs of `oHEGQ`: All projects, Kontur's Overview in 2–9, 11 and 13–15, Atlas' Pipelines (needs
/// you) in 10, and Orchester's Orchestrator (needs you) in 12, active.
fn designed_tabs() -> Vec<Slot> {
  let mut slots = vec![Slot::Icon];
  slots.extend(labelled(14));
  slots[9] = Slot::Labelled(MARK_WIDTH);
  slots[11] = Slot::Labelled(MARK_WIDTH);
  slots
}

/// The tabs' room in the design's Windows title bars with the *N need you* pill: the window less the app
/// menu, the strip's start, *New tab*, 48 of drag area, the window controls and the pill.
fn designed_room(window: f32) -> f32 {
  window - (61.0 + 8.0 + 30.0 + 48.0 + 138.0) - 106.0
}

#[test]
fn fifteen_tabs_at_1440_show_six_beside_all_projects_and_the_active_one() {
  let fit = fit(designed_room(1440.0), &designed_tabs(), 11);
  assert_eq!(shown(&fit, 15), [0, 1, 2, 3, 4, 5, 6, 11], "{fit:?}");
  assert_eq!(fit.hidden().len(), 7);
}

#[test]
fn fifteen_tabs_at_960_show_two_beside_all_projects_and_the_active_one() {
  let fit = fit(designed_room(960.0), &designed_tabs(), 11);
  assert_eq!(shown(&fit, 15), [0, 1, 2, 11], "{fit:?}");
  assert_eq!(fit.hidden().len(), 11);
}

#[test]
fn the_button_grows_with_the_digits_of_its_count() {
  assert!(
    (button_width(7) - 37.0).abs() < 1.0,
    "`7` is 37 wide: {}",
    button_width(7)
  );
  assert!(
    (button_width(11) - 44.0).abs() < 1.0,
    "`11` is 44 wide: {}",
    button_width(11)
  );
}

#[test]
fn icon_tabs_and_marks_take_their_own_room() {
  let room = narrowest(3);
  let mut icons = vec![Slot::Icon; 7];
  icons.push(Slot::Labelled(0.0));
  assert!(
    !fit(room, &icons, 7).overflows(),
    "seven {ICON_TAB_WIDTH} wide icon tabs fit where three labelled ones do"
  );
  let marked = vec![Slot::Labelled(MARK_WIDTH); 4];
  assert!(
    fit(room, &marked, 0).overflows(),
    "a mark on every tab leaves no room for the fourth"
  );
}

#[test]
fn a_window_too_narrow_for_one_tab_still_shows_the_active_one() {
  let fit = fit(10.0, &labelled(5), 3);
  assert_eq!(fit.hidden(), &[0, 1, 2, 4]);
  assert_eq!(fit.label_width(), LABEL_MIN_WIDTH);
}
