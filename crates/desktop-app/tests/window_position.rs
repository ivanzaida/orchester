//! The place QA asks the window to open at (`ORCHESTER_APP_WINDOW_POSITION`, set by `tools/qa-app.ps1
//! -WindowPosition`): a signed X,Y pair, refused when it is incomplete or not numbers, and nothing when
//! it is not set.

use std::ffi::OsStr;

use orchester_app::startup::{PositionError, WindowPosition};

fn of(value: &str) -> Result<Option<WindowPosition>, PositionError> {
  WindowPosition::of_variable(Some(OsStr::new(value)))
}

#[test]
fn window_position_unset_leaves_the_place_to_the_system() {
  assert_eq!(WindowPosition::of_variable(None), Ok(None));
}

#[test]
fn window_position_takes_negative_coordinates_of_a_display_left_of_the_main_one() {
  assert_eq!(of("-2500,40"), Ok(Some(WindowPosition { x: -2500, y: 40 })));
  assert_eq!(of("-2500,-40"), Ok(Some(WindowPosition { x: -2500, y: -40 })));
  assert_eq!(of("100,0"), Ok(Some(WindowPosition { x: 100, y: 0 })));
}

#[test]
fn window_position_refuses_an_incomplete_pair() {
  for value in ["", "-2500", "-2500,", ",40", "-2500,40,0", ","] {
    assert_eq!(of(value), Err(PositionError::NotAPair(value.to_owned())), "`{value}`");
  }
}

#[test]
fn window_position_refuses_values_that_are_not_whole_pixels() {
  let cases = [
    ("x,40", 'X', "x"),
    ("-2500,top", 'Y', "top"),
    ("-2500.5,40", 'X', "-2500.5"),
    (" -2500,40", 'X', " -2500"),
    ("-2500, 40", 'Y', " 40"),
    ("3000000000,0", 'X', "3000000000"),
  ];
  for (value, axis, part) in cases {
    assert_eq!(
      of(value),
      Err(PositionError::NotANumber {
        axis,
        value: part.to_owned()
      }),
      "`{value}`"
    );
  }
}

#[cfg(unix)]
#[test]
fn window_position_refuses_a_value_that_is_not_text() {
  use std::os::unix::ffi::OsStrExt;
  let value = OsStr::from_bytes(b"\xff,0");
  assert_eq!(WindowPosition::of_variable(Some(value)), Err(PositionError::NotText));
}

#[cfg(windows)]
#[test]
fn window_position_refuses_a_value_that_is_not_text() {
  use std::{ffi::OsString, os::windows::ffi::OsStringExt};
  let value = OsString::from_wide(&[0xD800, u16::from(b','), u16::from(b'0')]);
  assert_eq!(WindowPosition::of_variable(Some(&value)), Err(PositionError::NotText));
}
