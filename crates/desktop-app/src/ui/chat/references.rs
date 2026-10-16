//! What an agent writes as plain text that names a place in the app, made a link the chat follows as any
//! `orchester:` link (`links`): a task key of the chat's project that exists (`ORC-38`), and an app route path of a
//! place in a project (`/project/<id>/tasks/ORC-38/summary`, the `link` every Orchester MCP tool returns). Only text
//! counts: not a code span or block, math, HTML, or a link's words. An unknown key, a route outside a project or one
//! the app does not know stays text. The text is found by its offsets in what the agent wrote, parsed as lurq's
//! Markdown parses it, and a link is written in its place, its words as written.

use std::{borrow::Cow, ops::Range, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::{id::ProjectId, project::TaskKey};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use super::links::APP_SCHEME;
use crate::{
  project::Projects,
  routes::{ProjectPage, Route, TaskView},
};

/// Where an app route path starts.
const PATH_START: &str = "/project/";

/// `text` as the chat of `project` shows it: with the places it names linked, its tasks as the window last read
/// them. Reading them in a render re-renders it when they change.
pub fn shown(ctx: &mut Ctx, text: &Arc<str>, project: ProjectId) -> Arc<str> {
  let tasks = Projects::of(ctx).feed(project).tasks.get();
  let tasks = tasks.ready().map(|tasks| tasks.tasks.clone());
  let exists = |key: &TaskKey| tasks.as_ref().is_some_and(|tasks| tasks.iter().any(|task| task.key() == key));
  match linked(text, project, &exists) {
    Cow::Borrowed(_) => text.clone(),
    Cow::Owned(linked) => Arc::from(linked),
  }
}

/// `text` with every place it names as text written as a link to it. `project` is the chat's; `exists` says whether
/// a key is one of its tasks.
pub fn linked<'a>(text: &'a str, project: ProjectId, exists: &dyn Fn(&TaskKey) -> bool) -> Cow<'a, str> {
  let places: Vec<_> = plain_runs(text)
    .into_iter()
    .flat_map(|run| references(text, run, project, exists))
    .collect();
  if places.is_empty() {
    return Cow::Borrowed(text);
  }
  let mut linked = String::with_capacity(text.len() + places.len() * 64);
  let mut at = 0;
  for (range, route) in places {
    linked.push_str(&text[at..range.start]);
    linked.push('[');
    for ch in text[range.clone()].chars() {
      if ch.is_ascii_punctuation() {
        linked.push('\\');
      }
      linked.push(ch);
    }
    linked.push_str("](");
    linked.push_str(APP_SCHEME);
    linked.push_str(&route.path());
    linked.push(')');
    at = range.end;
  }
  linked.push_str(&text[at..]);
  Cow::Owned(linked)
}

/// The runs of `text` that Markdown shows as plain text, as written: outside code, math, HTML, links and images.
/// Text whose source differs from what it shows (an escape, an entity) is left out.
fn plain_runs(text: &str) -> Vec<Range<usize>> {
  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  options.insert(Options::ENABLE_STRIKETHROUGH);
  options.insert(Options::ENABLE_TASKLISTS);
  options.insert(Options::ENABLE_FOOTNOTES);
  options.insert(Options::ENABLE_MATH);
  let mut runs: Vec<Range<usize>> = Vec::new();
  let mut inside = 0_usize;
  for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
    match event {
      Event::Start(Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_) | Tag::HtmlBlock) => inside += 1,
      Event::End(TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock | TagEnd::HtmlBlock) => {
        inside = inside.saturating_sub(1);
      }
      Event::Text(shown) if inside == 0 && text.get(range.clone()) == Some(&*shown) => match runs.last_mut() {
        Some(last) if last.end == range.start => last.end = range.end,
        _ => runs.push(range),
      },
      _ => {}
    }
  }
  runs
}

/// The places the plain text of `text` in `run` names, by where they are in `text`. What lies around the run in
/// `text` (an entity, an escape) still decides where a word starts and ends.
fn references(
  text: &str,
  run: Range<usize>,
  project: ProjectId,
  exists: &dyn Fn(&TaskKey) -> bool,
) -> Vec<(Range<usize>, Route)> {
  let scope = &text[..run.end];
  let mut found = Vec::new();
  let mut at = run.start;
  while let Some(ch) = scope[at..].chars().next() {
    let place = if starts_word(text, at) {
      route_path(scope, at).or_else(|| task_key((scope, text), at, project, exists))
    } else {
      None
    };
    match place {
      Some((end, route)) => {
        found.push((at..end, route));
        at = end;
      }
      None => at += ch.len_utf8(),
    }
  }
  found
}

/// Whether a reference may start at `at`: not inside a word, an address or a path.
fn starts_word(text: &str, at: usize) -> bool {
  text[..at]
    .chars()
    .next_back()
    .is_none_or(|before| !(before.is_alphanumeric() || "-_/\\.:;@#%&=?~+".contains(before)))
}

/// The end of the app route path of a place in a project at `at` in `scope`, and its route.
fn route_path(scope: &str, at: usize) -> Option<(usize, Route)> {
  let rest = &scope[at..];
  if !rest.starts_with(PATH_START) {
    return None;
  }
  let len = rest
    .find(|ch: char| ch.is_whitespace() || "()[]{}<>\"'`|*".contains(ch))
    .unwrap_or(rest.len());
  // What ends a sentence or a clause after a path is not part of it.
  let path = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
  match Route::parse(path) {
    Ok(route @ Route::Project(..)) => Some((at + path.len(), route)),
    Ok(_) | Err(_) => None,
  }
}

/// The end of the key of one of `project`'s tasks at `at` in `scope`, a part of `text`, and the task's route.
fn task_key(
  (scope, text): (&str, &str),
  at: usize,
  project: ProjectId,
  exists: &dyn Fn(&TaskKey) -> bool,
) -> Option<(usize, Route)> {
  let rest = &scope[at..];
  let prefix = rest.find(|ch: char| !(ch.is_ascii_uppercase() || ch.is_ascii_digit()))?;
  let number = rest[prefix..].strip_prefix('-')?;
  let digits = number.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(number.len());
  let end = at + prefix + 1 + digits;
  if text[end..]
    .chars()
    .next()
    .is_some_and(|after| after.is_alphanumeric() || after == '_' || after == '-')
  {
    return None;
  }
  let key: TaskKey = text[at..end].parse().ok()?;
  exists(&key).then(|| (end, Route::Project(project, ProjectPage::Task(key, TaskView::Summary))))
}
