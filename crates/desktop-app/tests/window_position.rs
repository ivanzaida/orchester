use std::ffi::OsStr;

use orchester_app::startup::{WindowPosition, WindowPositionError};

fn parse(x: &str, y: &str) -> Result<Option<WindowPosition>, WindowPositionError> {
  WindowPosition::parse(Some(OsStr::new(x)), Some(OsStr::new(y)))
}

#[test]
fn window_position_absent_keeps_os_placement() {
  assert_eq!(WindowPosition::parse(None, None), Ok(None));
}

#[test]
fn window_position_signed_secondary_origin_is_preserved() {
  for (x, y, expected) in [
    ("-2500", "40", (-2500, 40)),
    ("0", "-900", (0, -900)),
    ("+40", "0", (40, 0)),
  ] {
    let position = parse(x, y)
      .expect("valid signed origin")
      .expect("both coordinates were supplied");
    assert_eq!(position.coordinates(), expected);
  }
}

#[test]
fn window_position_accepts_signed_integer_bounds() {
  let position = parse("-2147483648", "2147483647")
    .expect("i32 bounds are valid coordinates")
    .expect("both coordinates were supplied");
  assert_eq!(position.coordinates(), (i32::MIN, i32::MAX));
}

#[test]
fn window_position_requires_both_coordinates() {
  assert_eq!(
    WindowPosition::parse(Some(OsStr::new("-2500")), None),
    Err(WindowPositionError::Incomplete)
  );
  assert_eq!(
    WindowPosition::parse(None, Some(OsStr::new("40"))),
    Err(WindowPositionError::Incomplete)
  );
}

#[test]
fn window_position_rejects_invalid_or_overflowing_coordinates() {
  for value in ["", " ", "-", "1.5", "40px", " 40", "40 ", "2147483648", "-2147483649"] {
    assert_eq!(parse(value, "40"), Err(WindowPositionError::Invalid));
    assert_eq!(parse("-2500", value), Err(WindowPositionError::Invalid));
  }
}

#[cfg(windows)]
#[test]
fn window_position_rejects_non_unicode_coordinates() {
  use std::{ffi::OsString, os::windows::ffi::OsStringExt};

  let invalid = OsString::from_wide(&[0xd800]);
  assert_eq!(
    WindowPosition::parse(Some(&invalid), Some(OsStr::new("40"))),
    Err(WindowPositionError::Invalid)
  );
}

#[cfg(unix)]
#[test]
fn window_position_rejects_non_unicode_coordinates() {
  use std::{ffi::OsString, os::unix::ffi::OsStringExt};

  let invalid = OsString::from_vec(vec![0xff]);
  assert_eq!(
    WindowPosition::parse(Some(OsStr::new("-2500")), Some(&invalid)),
    Err(WindowPositionError::Invalid)
  );
}
