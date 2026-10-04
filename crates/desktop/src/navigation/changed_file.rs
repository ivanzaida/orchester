//! A task's Changes tab opened on a file keeps the file in the path's `file` query (and `from` for
//! the worktree's version), which the tab reads itself, so the typed routes stay plain values.

use std::fmt::Write as _;

use orchester_core::id::{ProjectId, TaskId};

use super::route::{Page, TaskPageTab};

const FILE_QUERY: &str = "file";
const FROM_QUERY: &str = "from";
const WORKTREE: &str = "worktree";

/// A changed file a task's Changes tab shows: its repository path (`/`-separated), and whether it
/// is the version in the run's worktree that is not checkpointed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
  pub path: String,
  pub worktree: bool,
}

/// The path of `task`'s Changes tab in `project`, on `file`.
pub fn changes_path(project: ProjectId, task: TaskId, file: &ChangedFile) -> String {
  let tab = Page::Task(task, TaskPageTab::Changes).path(project);
  let from = if file.worktree {
    format!("&{FROM_QUERY}={WORKTREE}")
  } else {
    String::new()
  };
  format!("{tab}&{FILE_QUERY}={}{from}", encode(&file.path))
}

/// The changed file the queries of `path` name, if they name one.
pub fn changed_file(path: &str) -> Option<ChangedFile> {
  let (_, query) = path.split_once('?')?;
  let value = |name: &str| {
    query
      .split('&')
      .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
  };
  let path = decode(value(FILE_QUERY)?).filter(|file| !file.is_empty())?;
  Some(ChangedFile {
    path,
    worktree: value(FROM_QUERY) == Some(WORKTREE),
  })
}

/// `value` with every byte outside the unreserved characters and `/` percent-encoded, so a path
/// with spaces, `&`, `?`, `#` or non-ASCII names stays one query value.
fn encode(value: &str) -> String {
  let mut encoded = String::with_capacity(value.len());
  for byte in value.bytes() {
    if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
      encoded.push(char::from(byte));
    } else {
      write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail");
    }
  }
  encoded
}

/// [`encode`] undone; `None` for a malformed escape or bytes that are not UTF-8.
fn decode(value: &str) -> Option<String> {
  let mut bytes = Vec::with_capacity(value.len());
  let mut rest = value.bytes();
  while let Some(byte) = rest.next() {
    if byte != b'%' {
      bytes.push(byte);
      continue;
    }
    let digits = [rest.next()?, rest.next()?];
    let hex = std::str::from_utf8(&digits).ok()?;
    bytes.push(u8::from_str_radix(hex, 16).ok()?);
  }
  String::from_utf8(bytes).ok()
}
