//! Why a report could not be read, with what was readable before it.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::{ReportFormat, report::Totals};

/// A report that cannot be taken as a result. `read` counts the tests read whole before the problem; formats read as
/// one document (JSON, XML) read none when the document does not parse, and `at` or `detail` says where it stopped.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TestReportError {
  #[error("the {format} report is empty")]
  Empty { format: ReportFormat },
  #[error("the {format} report cannot be read at {at}: {detail}; read before it: {read}")]
  Malformed {
    format: ReportFormat,
    at: Position,
    detail: String,
    read: Totals,
  },
  #[error("the {format} report ends early: {detail}; read before it: {read}")]
  Truncated {
    format: ReportFormat,
    detail: String,
    read: Totals,
  },
  /// The TAP producer stopped the run (`Bail out!`).
  #[error("the TAP report bailed out: {reason}; read before it: {read}")]
  BailedOut { reason: String, read: Totals },
}

impl TestReportError {
  /// A JSON document that ended early (cut off) or does not parse as `format`.
  pub(super) fn json(format: ReportFormat, error: &serde_json::Error) -> Self {
    let read = Totals::default();
    if error.is_eof() {
      return Self::Truncated {
        format,
        detail: error.to_string(),
        read,
      };
    }
    let at = Position {
      line: error.line(),
      column: error.column(),
    };
    Self::Malformed {
      format,
      at,
      detail: error.to_string(),
      read,
    }
  }
}

/// A place in a report's text, from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
  pub line: usize,
  pub column: usize,
}

impl Position {
  pub(super) const fn line(line: usize) -> Self {
    Self { line, column: 1 }
  }
}

impl fmt::Display for Position {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "line {}, column {}", self.line, self.column)
  }
}
