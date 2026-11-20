//! How wide the title bar's tabs are (`oHEGQ`, `YAOSy`): a tab is as wide as what it shows, its label capped;
//! inactive labels' cap shrinks as tabs open down to a 120 wide tab while the active tab keeps its width. Which tabs
//! leave the strip once they do not fit is lurq's window tabs' (their list mode), tested there.

use orchester_app::chrome::tab_fit::{
  DOT_WIDTH, ICON_TAB_WIDTH, LABEL_MAX_WIDTH, LABEL_MIN_WIDTH, Slot, TAB_MIN_WIDTH, label_cap, room, strip_room,
  tab_width,
};

/// The active tab at its widest: a 160 label, the 13 icon, the 18 close button, their gaps and padding
/// (`Tab/Active` `jhz5t`).
const ACTIVE_WIDTH: f32 = LABEL_MAX_WIDTH + 59.0;

fn labelled(count: usize) -> Vec<Slot> {
  vec![Slot::Labelled(0.0); count]
}

/// The room `inactive` tabs at 120 take beside the active tab.
fn narrowest(inactive: usize) -> f32 {
  inactive as f32 * TAB_MIN_WIDTH + ACTIVE_WIDTH
}

#[test]
fn a_few_tabs_keep_their_widest_labels_and_they_shrink_as_tabs_open() {
  assert_eq!(
    label_cap(1000.0, &labelled(3), 0),
    Some(LABEL_MAX_WIDTH),
    "room to spare"
  );
  let more = label_cap(1000.0, &labelled(6), 0).expect("six fit");
  assert!(
    more < LABEL_MAX_WIDTH && more > LABEL_MIN_WIDTH,
    "inactive labels shrink: {more}"
  );
}

#[test]
fn a_tab_is_120_wide_at_its_narrowest() {
  assert_eq!(TAB_MIN_WIDTH, 120.0);
  assert_eq!(tab_width(None, LABEL_MIN_WIDTH, 0.0, false), 120.0);
}

#[test]
fn inactive_labels_stop_at_81_where_tabs_no_longer_fit() {
  let room = narrowest(7);
  assert_eq!(
    label_cap(room, &labelled(8), 0),
    Some(LABEL_MIN_WIDTH),
    "seven 120 wide tabs just fit"
  );
  assert_eq!(label_cap(room - 1.0, &labelled(8), 0), None, "one less and they do not");
  assert!(
    label_cap(room + 70.0, &labelled(8), 0).is_some_and(|cap| cap > LABEL_MIN_WIDTH),
    "more room, longer labels"
  );
}

#[test]
fn icon_tabs_and_marks_take_their_own_room() {
  let room = narrowest(3);
  let mut icons = vec![Slot::Icon; 7];
  icons.push(Slot::Labelled(0.0));
  icons.push(Slot::Labelled(0.0));
  assert!(
    label_cap(room, &icons, 7).is_some_and(|cap| cap > LABEL_MIN_WIDTH),
    "seven {ICON_TAB_WIDTH} wide icon tabs leave room for longer labels"
  );
  let marked = vec![Slot::Labelled(DOT_WIDTH); 4];
  let roomy = room + 150.0;
  assert!(
    label_cap(roomy, &marked, 0) < label_cap(roomy, &labelled(4), 0),
    "marks take room from the labels"
  );
}

#[test]
fn a_measured_active_tab_leaves_its_unused_room_to_the_others() {
  let room = narrowest(5) + 50.0;
  let mut slots = labelled(6);
  let widest = label_cap(room, &slots, 0).expect("they fit");
  slots[0] = Slot::Measured(tab_width(Some(60.0), LABEL_MAX_WIDTH, 0.0, true));
  assert!(
    label_cap(room, &slots, 0).is_some_and(|cap| cap > widest),
    "a short active label leaves room"
  );
}

#[test]
fn a_tab_is_as_wide_as_its_label_up_to_the_cap() {
  let short = tab_width(Some(50.0), LABEL_MAX_WIDTH, 0.0, false);
  assert_eq!(short, 50.0 + 39.0, "a short label, the icon and the padding");
  assert_eq!(
    tab_width(Some(300.0), LABEL_MAX_WIDTH, 0.0, true),
    ACTIVE_WIDTH,
    "a long one is cut"
  );
  assert_eq!(
    tab_width(None, LABEL_MAX_WIDTH, 0.0, true),
    ACTIVE_WIDTH,
    "counted at its widest until measured"
  );
  assert_eq!(
    tab_width(Some(50.0), LABEL_MIN_WIDTH, DOT_WIDTH, false),
    short + DOT_WIDTH,
    "the unsaved dot adds its room"
  );
}

#[test]
fn the_strip_leaves_the_drag_area_and_new_tab_their_room() {
  let strip = strip_room(1440.0, 106.0);
  assert!(
    strip < 1440.0 - 106.0 - 48.0,
    "the drag area and the pill stay: {strip}"
  );
  assert_eq!(room(strip), strip - 26.0, "New tab after the tabs");
}
