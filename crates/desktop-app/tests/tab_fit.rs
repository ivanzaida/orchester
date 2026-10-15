//! How the title bar's tabs share their room: labels shrink as tabs open, and once even the narrowest
//! tabs do not fit, the ones that do not leave the strip for the tab list, the active one never.

use orchester_app::chrome::tab_fit::{
  CHEVRON_WIDTH, ICON_TAB_WIDTH, LABEL_MIN_WIDTH, MARK_WIDTH, Slot, TAB_MIN_WIDTH, fit, gap,
};

/// The room `count` labelled tabs take at their narrowest.
fn narrowest(count: usize) -> f32 {
  count as f32 * TAB_MIN_WIDTH + count.saturating_sub(1) as f32 * gap()
}

fn labelled(count: usize) -> Vec<Slot> {
  vec![Slot::Labelled(0.0); count]
}

#[test]
fn a_few_tabs_all_show_with_their_labels_capped() {
  let fit = fit(1000.0, &labelled(3), 0);
  assert!(!fit.overflows());
  assert!((0..3).all(|index| fit.shows(index)));
  assert!(fit.label_width() > LABEL_MIN_WIDTH, "room to spare: {fit:?}");
  let more = fit_of(1000.0, 6);
  assert!(
    more.label_width() < fit.label_width(),
    "labels shrink as tabs open: {more:?}"
  );
}

fn fit_of(room: f32, count: usize) -> orchester_app::chrome::tab_fit::Fit {
  fit(room, &labelled(count), 0)
}

#[test]
fn the_list_appears_once_the_narrowest_tabs_do_not_fit() {
  let room = narrowest(8);
  let at = fit_of(room, 8);
  assert!(!at.overflows(), "eight narrowest tabs fit exactly: {at:?}");
  assert_eq!(at.label_width(), LABEL_MIN_WIDTH);
  let over = fit_of(room - 1.0, 8);
  assert!(over.overflows(), "one less and they do not: {over:?}");
  let shown = (0..8).filter(|index| over.shows(*index)).count();
  let chevron = CHEVRON_WIDTH + gap();
  assert_eq!(
    shown,
    ((room - 1.0 - chevron + gap()) / (TAB_MIN_WIDTH + gap())) as usize,
    "as many as fit beside the chevron"
  );
  assert_eq!(
    over.hidden(),
    (shown..8).collect::<Vec<_>>(),
    "the last ones leave: {over:?}"
  );
  assert!(over.label_width() >= LABEL_MIN_WIDTH);
}

#[test]
fn the_active_tab_always_shows_in_place_of_the_last_ones() {
  let slots = labelled(20);
  let room = narrowest(6);
  for active in 0..20 {
    let fit = fit(room, &slots, active);
    assert!(fit.shows(active), "tab {active} is active: {fit:?}");
    assert!(fit.overflows());
    let shown: Vec<usize> = (0..20).filter(|index| fit.shows(*index)).collect();
    let others: Vec<usize> = shown.iter().copied().filter(|index| *index != active).collect();
    let first: Vec<usize> = (0..20).filter(|index| *index != active).take(others.len()).collect();
    assert_eq!(others, first, "the others are the first ones in order");
    assert_eq!(shown.len() + fit.hidden().len(), 20, "every tab shows or is listed");
  }
}

#[test]
fn a_hidden_active_tab_takes_the_last_place_that_shows() {
  let room = narrowest(6);
  let first = fit(room, &labelled(20), 0);
  let shown: Vec<usize> = (0..20).filter(|index| first.shows(*index)).collect();
  let last = fit(room, &labelled(20), 19);
  let now: Vec<usize> = (0..20).filter(|index| last.shows(*index)).collect();
  assert_eq!(now.len(), shown.len(), "as many tabs show: {last:?}");
  assert_eq!(now.last(), Some(&19));
  assert_eq!(&now[..now.len() - 1], &shown[..shown.len() - 1]);
}

#[test]
fn icon_tabs_and_marks_take_their_own_room() {
  let room = narrowest(4);
  let icons = vec![Slot::Icon; 6];
  let icon_fit = fit(room, &icons, 0);
  assert!(
    !icon_fit.overflows(),
    "six {ICON_TAB_WIDTH} wide icon tabs fit where four labelled ones do"
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
