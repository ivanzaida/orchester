//! Path rules on macOS and other Unix systems: every spelling other than NUL (refused by the caller) is a
//! plain path, and names compare byte for byte. Comparing roots exactly can refuse a path on a
//! case-insensitive volume that was written in another case than the folder has, never admit one outside
//! (`docs/architecture/native-agent.md` §5.1, *to verify on a Mac*).

use std::ffi::OsStr;

use super::{PathForm, ScopeError};

pub(super) const fn path_form(_path: &str) -> Result<PathForm, ScopeError> {
  Ok(PathForm::Local)
}

/// No spelling of a Unix path names another machine.
pub(super) const fn names_another_machine(_path: &str) -> bool {
  false
}

pub(super) fn same_name(actual: &OsStr, expected: &OsStr) -> bool {
  actual == expected
}

/// Whether `name` is the protected name `expected`, without ASCII case: a case-insensitive volume, the
/// macOS default, opens `.GIT` as `.git`.
pub(super) fn is_named(name: &OsStr, expected: &str) -> bool {
  name.to_str().is_some_and(|name| name.eq_ignore_ascii_case(expected))
}
