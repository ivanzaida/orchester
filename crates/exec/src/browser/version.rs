//! Which version a browser's program is, read from what its install leaves on disk, without starting it. Each
//! platform says so in its own place; `None` when that place does not tell, which is never filled in with a guess.
//!
//! - macOS ([`in_bundle`]): `CFBundleShortVersionString` in the `Info.plist` of the app bundle around the program. Only
//!   the XML form is read, which Chromium's builds write; a binary one tells nothing.
//! - Windows ([`in_folder_beside`]): the folder named after the version that an installer puts beside the program; a
//!   program copied without one tells nothing.
//!
//! Both are plain file reads, so they are tested on either platform.

use std::{fs, io, path::Path};

/// The key of `Info.plist` holding the version people see.
const VERSION_KEY: &str = "CFBundleShortVersionString";

/// The version in the `Info.plist` of the app bundle whose program is `program` (`….app/Contents/MacOS/<program>`).
pub fn in_bundle(program: &Path) -> Option<String> {
  let inside = program.parent()?;
  if !inside.ends_with("Contents/MacOS") {
    return None;
  }
  let plist = inside.parent()?.join("Info.plist");
  match fs::read_to_string(&plist) {
    Ok(text) => short_version(&text),
    Err(error) => {
      unreadable(&plist, &error);
      None
    }
  }
}

/// The version `text`, an XML `Info.plist`, names under [`VERSION_KEY`].
fn short_version(text: &str) -> Option<String> {
  let (_, after_key) = text.split_once(&format!("<key>{VERSION_KEY}</key>"))?;
  let (value, _) = after_key
    .trim_start()
    .strip_prefix("<string>")?
    .split_once("</string>")?;
  let value = value.trim();
  numbers(value).map(|_| value.to_owned())
}

/// The newest version named by a folder beside `program`. An update leaves the version before it there until the
/// browser next restarts, and the newest is the one the next start runs.
pub fn in_folder_beside(program: &Path) -> Option<String> {
  let beside = program.parent()?;
  let entries = fs::read_dir(beside)
    .inspect_err(|error| unreadable(beside, error))
    // No version is shown; the failure is logged.
    .ok()?;
  entries
    // An entry that cannot be read names no version folder this can tell about.
    .filter_map(Result::ok)
    .filter(|entry| entry.path().is_dir())
    .filter_map(|entry| entry.file_name().into_string().ok())
    .filter_map(|name| Some((numbers(&name)?, name)))
    .max()
    .map(|(_, name)| name)
}

/// A version's numbers, for comparing: at least two between dots, as every browser's has; `None` for anything else.
fn numbers(version: &str) -> Option<Vec<u64>> {
  let numbers = version
    .split('.')
    .map(|part| {
      part
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| part.parse().ok())
        .flatten()
    })
    .collect::<Option<Vec<u64>>>()?;
  (numbers.len() >= 2).then_some(numbers)
}

/// Logs why `path` cannot tell the version, unless it is simply not there.
fn unreadable(path: &Path, error: &io::Error) {
  if error.kind() != io::ErrorKind::NotFound {
    tracing::warn!(%error, path = %path.display(), "cannot read the browser's version");
  }
}
