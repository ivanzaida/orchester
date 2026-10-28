//! Where a link's destination, a code span or an app route path leads.

use super::{DocTarget, Lines, LinkTarget};
use crate::{
  id::ProjectId,
  project::{Project, TaskKey},
};

/// The scheme of a link to a place in the app, matched ignoring case as schemes are.
const APP_SCHEME: &str = "orchester:";
/// Where an app route path of a place in a project starts.
pub(super) const PROJECT_PATH: &str = "/project/";

/// The doc a wiki link's `Title` or `Title#Heading` names; `None` for a heading of the doc itself (`[[#Heading]]`).
pub(super) fn doc_title(written: &str) -> Option<LinkTarget> {
  let (title, heading) = match written.split_once('#') {
    Some((title, heading)) => (title, Some(heading.trim()).filter(|heading| !heading.is_empty())),
    None => (written, None),
  };
  let title = title.trim();
  (!title.is_empty()).then(|| {
    LinkTarget::Doc(DocTarget::Titled {
      title: title.to_owned(),
      heading: heading.map(str::to_owned),
    })
  })
}

/// Where a Markdown link's `destination` leads: a place in the app, or a file of the repository.
pub(super) fn destination(destination: &str, project: &Project) -> Option<LinkTarget> {
  if let Some(path) = strip_scheme(destination) {
    return route(path, project);
  }
  if destination.starts_with(PROJECT_PATH) {
    return route(destination, project);
  }
  file(destination, false)
}

/// Where a code span that holds only `held` leads: a place in the app, a task of `project`, or a file of the repository
/// in a folder.
pub(super) fn code(held: &str, project: &Project) -> Option<LinkTarget> {
  if held.starts_with(PROJECT_PATH) {
    return route(held, project);
  }
  task_key(held, project)
    .map(LinkTarget::Task)
    .or_else(|| file(held, true))
}

/// The key of a task of `project` that `written` is, as a key is written.
pub(super) fn task_key(written: &str, project: &Project) -> Option<TaskKey> {
  let key: TaskKey = written.parse().ok()?;
  (key.project() == project.key() && key.to_string() == written).then_some(key)
}

/// Where the app route path `path`, query included, leads: a task, a file or a doc of `project`.
pub(super) fn route(path: &str, project: &Project) -> Option<LinkTarget> {
  let (path, query) = path.split_once('?').unwrap_or((path, ""));
  let (id, rest) = path.strip_prefix(PROJECT_PATH)?.split_once('/')?;
  if id.parse::<ProjectId>().ok()? != project.id() {
    return None;
  }
  let (page, rest) = rest.split_once('/')?;
  match page {
    "tasks" => {
      let key = rest.split_once('/').map_or(rest, |(key, _)| key);
      task_key(key, project).map(LinkTarget::Task)
    }
    "files" => {
      let segments: Option<Vec<String>> = rest.split('/').map(decode).collect();
      let lines = match query.split('&').find_map(|param| param.strip_prefix("line=")) {
        Some(line) => Some(Lines::parse(&decode(line)?)?),
        None => None,
      };
      Some(LinkTarget::File {
        path: segments?.join("/"),
        lines,
      })
    }
    "docs" if !rest.contains('/') => rest.parse().ok().map(|doc| LinkTarget::Doc(DocTarget::Id(doc))),
    _ => None,
  }
}

/// `destination` past the app's scheme.
fn strip_scheme(destination: &str) -> Option<&str> {
  let head = destination.get(..APP_SCHEME.len())?;
  head
    .eq_ignore_ascii_case(APP_SCHEME)
    .then(|| &destination[APP_SCHEME.len()..])
}

/// The file `written` names: a repository path with, after it, `:12` or `:12-20`, or GitHub's `#L12` or `#L12-L20`.
/// With `in_folder`, a path of a file at the repository's root does not count.
fn file(written: &str, in_folder: bool) -> Option<LinkTarget> {
  let (path, lines) = match written.split_once('#') {
    Some((path, fragment)) => {
      let lines = fragment.strip_prefix('L')?;
      let (first, last) = lines.split_once("-L").unwrap_or((lines, lines));
      (path, Some(Lines::new(super::number(first)?, super::number(last)?)?))
    }
    None => match written.split_once(':') {
      Some((path, lines)) => (path, Some(Lines::parse(lines)?)),
      None => (written, None),
    },
  };
  (is_repository_path(path) && (!in_folder || path.contains('/'))).then(|| LinkTarget::File {
    path: path.to_owned(),
    lines,
  })
}

/// Whether `path` reads as a file's path from the repository's root: names of letters, digits and `-_.@+` joined by
/// `/`, none empty, `.` or `..` (those are relative to a file, not to the root), the last with an extension that starts
/// with a letter (`lib.rs`, `.gitignore`, not a version such as `0.51.2`).
fn is_repository_path(path: &str) -> bool {
  let allowed = |ch: char| ch.is_alphanumeric() || "-_.@+/".contains(ch);
  if path.is_empty() || !path.chars().all(allowed) {
    return false;
  }
  let names: Vec<&str> = path.split('/').collect();
  if names
    .iter()
    .any(|name| name.is_empty() || *name == "." || *name == "..")
  {
    return false;
  }
  names
    .last()
    .and_then(|name| name.rsplit_once('.'))
    .is_some_and(|(_, extension)| extension.starts_with(|ch: char| ch.is_alphabetic()))
}

/// `encoded` with its `%XX` escapes decoded, as the app's routes encode their values; `None` when it is not UTF-8 then.
fn decode(encoded: &str) -> Option<String> {
  let bytes = encoded.as_bytes();
  let mut decoded = Vec::with_capacity(bytes.len());
  let mut at = 0;
  while let Some(&byte) = bytes.get(at) {
    if byte == b'%' {
      let hex = encoded
        .get(at + 1..at + 3)
        .filter(|hex| hex.bytes().all(|digit| digit.is_ascii_hexdigit()))?;
      decoded.push(u8::from_str_radix(hex, 16).ok()?);
      at += 3;
    } else {
      decoded.push(byte);
      at += 1;
    }
  }
  String::from_utf8(decoded).ok()
}
