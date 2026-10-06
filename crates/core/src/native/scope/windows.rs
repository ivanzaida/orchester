//! Path rules on Windows (`docs/architecture/native-agent.md` §5.1). Refused are spellings that reach
//! something other than a file in a folder: verbatim and device paths (`\\?\`, `\\.\`), device names
//! (`CON`, `NUL`, `COM1`, … also with an extension), alternate data streams (`file:stream`) and
//! drive-relative paths (`C:file`, relative to a per-drive folder the agent does not see). Network paths
//! pass here and are allowed only inside a granted writable folder. Names compare without case, and
//! Windows drops trailing dots and spaces from a name, so `.git.` is `.git`.

use std::{
  ffi::OsStr,
  path::{Component, Path},
};

use super::{PathForm, ScopeError};

/// Device names without a number.
const DEVICES: [&str; 7] = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"];
/// Device names followed by one digit, including the superscript digits Windows also accepts.
const NUMBERED: [&str; 2] = ["COM", "LPT"];
const DEVICE_DIGITS: &str = "0123456789¹²³";

/// Whether `name` names a stream (`file:stream`, `folder::$INDEX_ALLOCATION`), which opens the file or
/// folder before the colon.
pub(super) fn names_stream(name: &OsStr) -> bool {
  name.to_string_lossy().contains(':')
}

/// Whether `path` starts with two separators: a network, device or verbatim path, which resolving could
/// send to another machine.
pub(super) fn names_another_machine(path: &str) -> bool {
  let mut chars = path.chars();
  matches!((chars.next(), chars.next()), (Some('\\' | '/'), Some('\\' | '/')))
}

pub(super) fn path_form(path: &str) -> Result<PathForm, ScopeError> {
  let unified = path.replace('/', "\\");
  if unified.starts_with(r"\\?\") || unified.starts_with(r"\\.\") || unified.starts_with(r"\??\") {
    return Err(ScopeError::DevicePath(path.to_owned()));
  }
  let form = if unified.starts_with(r"\\") {
    PathForm::Network
  } else {
    PathForm::Local
  };
  let mut components = Path::new(path).components().peekable();
  if let Some(Component::Prefix(_)) = components.peek() {
    components.next();
    if !matches!(components.peek(), Some(Component::RootDir)) {
      return Err(ScopeError::DriveRelative(path.to_owned()));
    }
  }
  for component in components {
    let Component::Normal(name) = component else {
      continue;
    };
    let name = name.to_string_lossy();
    if name.contains(':') {
      return Err(ScopeError::AlternateStream(path.to_owned()));
    }
    if is_device(&name) {
      return Err(ScopeError::DeviceName(name.into_owned()));
    }
  }
  Ok(form)
}

/// Whether `name` opens a device: its part before the first dot, without trailing spaces, is a device
/// name.
fn is_device(name: &str) -> bool {
  let name = trimmed(name);
  let stem = name
    .split('.')
    .next()
    .unwrap_or(name)
    .trim_end_matches(' ')
    .to_uppercase();
  if DEVICES.contains(&stem.as_str()) {
    return true;
  }
  NUMBERED.iter().any(|prefix| {
    stem.strip_prefix(prefix).is_some_and(|rest| {
      let mut chars = rest.chars();
      matches!((chars.next(), chars.next()), (Some(digit), None) if DEVICE_DIGITS.contains(digit))
    })
  })
}

pub(super) fn same_name(actual: &OsStr, expected: &OsStr) -> bool {
  match (actual.to_str(), expected.to_str()) {
    (Some(actual), Some(expected)) => actual == expected || actual.to_uppercase() == expected.to_uppercase(),
    _ => actual == expected,
  }
}

/// Whether `name` is the protected name `expected`, as Windows opens it.
pub(super) fn is_named(name: &OsStr, expected: &str) -> bool {
  name
    .to_str()
    .is_some_and(|name| trimmed(name).eq_ignore_ascii_case(expected))
}

/// `name` as Windows stores it: without trailing dots and spaces.
fn trimmed(name: &str) -> &str {
  name.trim_end_matches(['.', ' '])
}
