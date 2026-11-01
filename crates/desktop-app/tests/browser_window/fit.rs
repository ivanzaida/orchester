//! How the tabs share the title bar (notes `RggGn`): equally up to 220; crowded, the tab shown keeps 150 while the
//! others shrink to 90; past that the strip scrolls, and the tabs not wholly in view are counted.

use orchester_app::browser::fit::{SHOWN_MIN_WIDTH, TAB_MAX_WIDTH, TAB_MIN_WIDTH, fit, gap};

#[test]
fn browser_fit_shares_the_room_equally_up_to_the_widest_tab() {
  let roomy = fit(2000.0, 3, Some(0), false);
  assert!(!roomy.overflows());
  assert_eq!(roomy.widths(), [TAB_MAX_WIDTH; 3]);
  // Six tabs in the default window's room are 150 each, as `01 · Default` draws them.
  let room = 6.0 * 150.0 + 5.0 * gap() + 1.0 + gap();
  let six = fit(room, 6, Some(0), true);
  assert!(!six.overflows());
  for width in six.widths() {
    assert!((width - 150.0).abs() < 0.01, "{six:?}");
  }
}

#[test]
fn browser_fit_keeps_the_tab_shown_wide_while_the_others_shrink() {
  let room = 9.0 * 110.0;
  let crowded = fit(room, 10, Some(3), false);
  assert!(!crowded.overflows(), "{crowded:?}");
  assert_eq!(crowded.widths()[3], SHOWN_MIN_WIDTH);
  for (index, width) in crowded.widths().iter().enumerate() {
    if index != 3 {
      assert!(*width >= TAB_MIN_WIDTH && *width < SHOWN_MIN_WIDTH, "{crowded:?}");
    }
  }
}

#[test]
fn browser_fit_overflows_past_the_narrowest_and_counts_the_hidden_tabs() {
  let room = 800.0;
  let twenty = fit(room, 20, Some(9), true);
  assert!(twenty.overflows());
  assert_eq!(twenty.widths()[9], SHOWN_MIN_WIDTH);
  assert!(
    twenty
      .widths()
      .iter()
      .enumerate()
      .all(|(index, width)| index == 9 || *width == TAB_MIN_WIDTH)
  );
  let divider = Some(18);
  assert!(twenty.content_width(divider) > room);
  let hidden = twenty.hidden(divider, (0.0, room));
  assert!(
    !hidden.is_empty() && !hidden.contains(&0),
    "the first tabs are in view: {hidden:?}"
  );
  // The tab shown is scrolled into view, and then it is not counted.
  let scroll = twenty.scroll_to(9, divider, (0.0, room));
  let (start, end) = twenty.span(9, divider);
  assert!(
    start >= scroll && end <= scroll + room,
    "{start}..{end} in {scroll}+{room}"
  );
  assert!(!twenty.hidden(divider, (scroll, room)).contains(&9));
  assert!(
    twenty.hidden(divider, (scroll, room)).contains(&0),
    "the first tabs scrolled out of view"
  );
}

#[test]
fn browser_fit_of_one_tab_or_none() {
  assert!(fit(500.0, 0, None, false).widths().is_empty());
  assert_eq!(fit(500.0, 1, Some(0), false).widths(), [TAB_MAX_WIDTH]);
  assert!(fit(100.0, 1, Some(0), false).overflows(), "a tab shown keeps its 150");
}
