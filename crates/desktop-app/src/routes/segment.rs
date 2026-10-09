//! Identifiers that routes carry in their paths: a project and a task key. Each is one path segment.

use std::{fmt, sync::Arc};

/// The longest identifier a path segment holds.
pub const MAX_SEGMENT_LEN: usize = 64;

/// Why a value cannot be one segment of a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SegmentError {
  #[error("it is empty")]
  Empty,
  #[error("it is longer than {MAX_SEGMENT_LEN} characters")]
  TooLong,
  #[error("it may hold only ASCII letters, digits, '-', '_' and '.'")]
  InvalidCharacter,
  #[error("it cannot be `.` or `..`")]
  Dots,
}

fn check(value: &str) -> Result<(), SegmentError> {
  if value.is_empty() {
    return Err(SegmentError::Empty);
  }
  if value.len() > MAX_SEGMENT_LEN {
    return Err(SegmentError::TooLong);
  }
  if !value
    .bytes()
    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
  {
    return Err(SegmentError::InvalidCharacter);
  }
  if value == "." || value == ".." {
    return Err(SegmentError::Dots);
  }
  Ok(())
}

macro_rules! segment {
  ($(#[$doc:meta])* $name:ident) => {
    $(#[$doc])*
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct $name(Arc<str>);

    impl $name {
      pub fn new(value: &str) -> Result<Self, SegmentError> {
        check(value)?;
        Ok(Self(value.into()))
      }

      pub fn as_str(&self) -> &str {
        &self.0
      }
    }

    impl fmt::Display for $name {
      fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
      }
    }
  };
}

segment! {
  /// A project as a route names it: its identifier, one path segment (`kontur`).
  ProjectRef
}

segment! {
  /// A task as a route names it: its key, one path segment (`KON-12`).
  TaskKey
}
