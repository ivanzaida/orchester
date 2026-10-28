//! Links written as plain text: an app route path, a run, and a task key.

use std::{num::NonZeroU32, ops::Range};

use super::{
  DocLink, LinkTarget,
  target::{self, PROJECT_PATH},
};
use crate::project::{Project, TaskKey};

/// What `Run #57` starts with, matched ignoring case.
const RUN: &str = "run #";
/// What joins a run to its task in `Run #3 of ORC-12`.
const OF: &str = " of ";

/// The links the plain text of `body` in `run` holds, a doc of `project`. What lies around the run in `body` (an
/// entity, an escape) still decides where a word starts and ends.
pub(super) fn links(body: &str, run: Range<usize>, project: &Project) -> Vec<DocLink> {
  let scope = &body[..run.end];
  let mut found = Vec::new();
  let mut at = run.start;
  while let Some(ch) = scope[at..].chars().next() {
    let link = if starts_word(body, at) {
      route_path(scope, at, project)
        .or_else(|| run_number((scope, body), at, project))
        .or_else(|| task_key((scope, body), at, project).map(|(end, key)| (end, LinkTarget::Task(key))))
    } else {
      None
    };
    match link {
      Some((end, target)) => {
        found.push(DocLink { target, at: at..end });
        at = end;
      }
      None => at += ch.len_utf8(),
    }
  }
  found
}

/// Whether a link may start at `at`: not inside a word, an address or a path.
fn starts_word(body: &str, at: usize) -> bool {
  body[..at]
    .chars()
    .next_back()
    .is_none_or(|before| !(before.is_alphanumeric() || "-_/\\.:;@#%&=?~+".contains(before)))
}

/// Whether a word ends at `end`: what follows does not carry it on.
fn ends_word(body: &str, end: usize) -> bool {
  body[end..]
    .chars()
    .next()
    .is_none_or(|after| !(after.is_alphanumeric() || after == '_' || after == '-'))
}

/// The end of the app route path of a place at `at` in `scope`, and where it leads.
fn route_path(scope: &str, at: usize, project: &Project) -> Option<(usize, LinkTarget)> {
  let rest = &scope[at..];
  if !rest.starts_with(PROJECT_PATH) {
    return None;
  }
  let len = rest
    .find(|ch: char| ch.is_whitespace() || "()[]{}<>\"'`|*".contains(ch))
    .unwrap_or(rest.len());
  // What ends a sentence or a clause after a path is not part of it.
  let path = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
  target::route(path, project).map(|target| (at + path.len(), target))
}

/// The end of `Run #57`, or of `Run #3 of ORC-12` with a task of `project`, at `at` in `scope`, a part of `body`.
fn run_number((scope, body): (&str, &str), at: usize, project: &Project) -> Option<(usize, LinkTarget)> {
  if !scope[at..].get(..RUN.len())?.eq_ignore_ascii_case(RUN) {
    return None;
  }
  let start = at + RUN.len();
  let digits = scope[start..]
    .find(|ch: char| !ch.is_ascii_digit())
    .unwrap_or(scope.len() - start);
  let end = start + digits;
  if !ends_word(body, end) {
    return None;
  }
  let number = NonZeroU32::new(scope[start..end].parse().ok()?)?;
  let task = scope[end..]
    .starts_with(OF)
    .then(|| task_key((scope, body), end + OF.len(), project))
    .flatten();
  Some(match task {
    Some((key_end, key)) => (
      key_end,
      LinkTarget::Run {
        task: Some(key),
        number,
      },
    ),
    None => (end, LinkTarget::Run { task: None, number }),
  })
}

/// The end of the key of a task of `project` at `at` in `scope`, a part of `body`, and the key.
fn task_key((scope, body): (&str, &str), at: usize, project: &Project) -> Option<(usize, TaskKey)> {
  let prefix = project.key().as_str();
  let number = scope[at..].strip_prefix(prefix)?.strip_prefix('-')?;
  let digits = number.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(number.len());
  let end = at + prefix.len() + 1 + digits;
  if !ends_word(body, end) {
    return None;
  }
  target::task_key(&body[at..end], project).map(|key| (end, key))
}
