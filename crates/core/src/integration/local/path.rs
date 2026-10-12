use std::fmt;

use serde::{Deserialize, Serialize};

use super::LocalError;

/// Portable repository-relative path. Git metadata, parent traversal and Windows aliases are refused on both hosts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LocalPath(String);

impl LocalPath {
  pub fn new(value: &str) -> Result<Self, LocalError> {
    if value.is_empty() || value.contains(['\\', ':']) || value.chars().any(char::is_control) {
      return Err(LocalError::Unsupported);
    }
    for part in value.split('/') {
      let stem = part.split('.').next().unwrap_or_default().to_ascii_uppercase();
      let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
          stem
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
        });
      if part.is_empty()
        || matches!(part, "." | "..")
        || part.eq_ignore_ascii_case(".git")
        || part.eq_ignore_ascii_case(".orchester")
        || part.ends_with(['.', ' '])
        || reserved
      {
        return Err(LocalError::Unsupported);
      }
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for LocalPath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for LocalPath {
  type Error = LocalError;

  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::new(&value)
  }
}

impl From<LocalPath> for String {
  fn from(value: LocalPath) -> Self {
    value.0
  }
}
