//! Event cursors (S11): the canonical decimal of a committed sequence position, ordered by that integer and refused in
//! any other spelling.

use std::{collections::BTreeSet, num::NonZeroU64};

use orchester_api::v2::{EventCursor, EventCursorError};
use serde_json::json;

fn cursor(position: &str) -> EventCursor {
  EventCursor::new(position).expect("cursor")
}

#[test]
fn event_cursor_orders_by_integer_not_string() {
  assert!(cursor("9") < cursor("10"));
  assert!(cursor("10") > cursor("9"));
  assert!(cursor("2") < cursor("18446744073709551615"));
  let sorted: Vec<String> = ["100", "9", "10", "1"]
    .into_iter()
    .map(cursor)
    .collect::<BTreeSet<_>>()
    .into_iter()
    .map(String::from)
    .collect();
  assert_eq!(sorted, ["1", "9", "10", "100"]);
}

#[test]
fn event_cursor_equal_positions_are_equal() {
  assert_eq!(cursor("42"), cursor("42"));
  assert_eq!(cursor("42").cmp(&cursor("42")), std::cmp::Ordering::Equal);
  assert_eq!(cursor("42").position(), NonZeroU64::new(42).expect("positive"));
  assert_ne!(cursor("42"), cursor("43"));
}

#[test]
fn event_cursor_refuses_non_canonical_spellings() {
  for (wire, error) in [
    ("", EventCursorError::Empty),
    ("-1", EventCursorError::NotDecimal),
    ("+1", EventCursorError::NotDecimal),
    (" 1", EventCursorError::NotDecimal),
    ("1 ", EventCursorError::NotDecimal),
    ("c1", EventCursorError::NotDecimal),
    ("1.0", EventCursorError::NotDecimal),
    ("0x1f", EventCursorError::NotDecimal),
    ("1e3", EventCursorError::NotDecimal),
    ("١٢", EventCursorError::NotDecimal),
    ("0", EventCursorError::Zero),
    ("00", EventCursorError::LeadingZero),
    ("007", EventCursorError::LeadingZero),
    ("18446744073709551616", EventCursorError::OutOfRange),
  ] {
    assert_eq!(EventCursor::new(wire), Err(error), "{wire:?}");
    assert!(serde_json::from_value::<EventCursor>(json!(wire)).is_err(), "{wire:?}");
  }
  for not_a_string in [json!(7), json!(null), json!(["7"])] {
    assert!(
      serde_json::from_value::<EventCursor>(not_a_string.clone()).is_err(),
      "{not_a_string}"
    );
  }
}

#[test]
fn event_cursor_round_trips_as_a_string() {
  for wire in ["1", "9", "10", "41", "18446744073709551615"] {
    let read: EventCursor = serde_json::from_value(json!(wire)).expect("reads");
    assert_eq!(read, cursor(wire));
    assert_eq!(read.to_string(), wire);
    assert_eq!(serde_json::to_value(read).expect("serializes"), json!(wire));
  }
}
