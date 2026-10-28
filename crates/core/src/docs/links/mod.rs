//! The links a doc's body holds (`docs/design/docs.md`, *Format and storage*: `doc_links` is derived from a version's
//! body; *The editor* › Links), each with where it leads and where it is written. The body is read as CommonMark with
//! GitHub tables and task lists, as lurq's Markdown reads it, and `[[wiki links]]`. What counts:
//!
//! - **Tasks**: a key of the doc's project (`ORC-123`) in text, or a code span that holds only that key.
//! - **Runs**: `Run #57`, or `Run #3 of ORC-12`, as Orchester writes a run in its notices
//!   (`orchestrator::instructions`); runs have no link of their own in the app.
//! - **Files**: a repository path, with `:42`, `:10-20` or GitHub's `#L42`, `#L10-L20` for its lines, only as a link's
//!   destination (`[lib](crates/core/src/lib.rs:42)`) or as all a code span holds (`` `crates/core/src/lib.rs:42` ``).
//!   Prose is full of words with slashes and dots (*and/or*, *e.g.*), so a path in plain text is not one; a span also
//!   needs a folder in its path, as a bare `lib.rs` in backticks names a file more often than it points to one.
//! - **Docs**: `[[Title]]`, `[[Title#Heading]]` and `[[Title|words]]`.
//! - **Places in the app**: an `orchester:` link to an app route path, the form every Orchester link has
//!   (`orchester-workspace`'s `links`), and the bare path written in text or a code span, as a chat reads it: a task
//!   (`/project/<id>/tasks/ORC-12/summary`), a file with `line=` its lines, and a doc (`/project/<id>/docs/<doc id>`).
//!   Only places of the doc's own project count.
//!
//! Pencil nodes have no link form in Orchester yet, so none is found. Code blocks (fenced or indented), HTML, images
//! and a link's words are not read, only a link's destination; nor is text whose source differs from what it shows (an
//! escape, an entity), so every position is where the body has the link.

mod target;
mod text;

use std::{num::NonZeroU32, ops::Range};

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

use crate::{
  id::DocId,
  project::{Project, TaskKey},
};

/// A link in a doc's body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocLink {
  pub target: LinkTarget,
  /// Its bytes in the body: a whole Markdown link, a code span with its backticks, or the words in text.
  pub at: Range<usize>,
}

/// Where a link leads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
  Task(TaskKey),
  /// A run by its number, of the task it names, if it names one.
  Run {
    task: Option<TaskKey>,
    number: NonZeroU32,
  },
  /// A file of the repository by its path, as written, and a range of its lines.
  File {
    path: String,
    lines: Option<Lines>,
  },
  Doc(DocTarget),
}

/// A doc a link names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocTarget {
  /// By its title as written, and a heading in it.
  Titled { title: String, heading: Option<String> },
  /// By its id, in an Orchester link.
  Id(DocId),
}

/// Lines `first..=last` of a file, counted from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Lines {
  first: NonZeroU32,
  last: NonZeroU32,
}

impl Lines {
  /// Lines `a..=b` in either order; `None` when one of them is 0.
  pub fn new(a: u32, b: u32) -> Option<Self> {
    Some(Self {
      first: NonZeroU32::new(a.min(b))?,
      last: NonZeroU32::new(a.max(b))?,
    })
  }

  pub const fn first(self) -> NonZeroU32 {
    self.first
  }

  pub const fn last(self) -> NonZeroU32 {
    self.last
  }

  /// `12` or `12-20`, as a path's `:12-20` and a route's `line=12-20` write them.
  fn parse(written: &str) -> Option<Self> {
    let (a, b) = written.split_once('-').unwrap_or((written, written));
    Self::new(number(a)?, number(b)?)
  }
}

/// `text` when it is a number written in digits only, without a sign.
fn number(text: &str) -> Option<u32> {
  if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
    return None;
  }
  text.parse().ok()
}

/// The links `body`, a doc of `project`, holds, in the order they are written.
pub fn links(body: &str, project: &Project) -> Vec<DocLink> {
  let mut found = Vec::new();
  for part in parts(body) {
    match part {
      Part::Text(run) => found.extend(text::links(body, run, project)),
      Part::Code { span, inner } => {
        if let Some(target) = target::code(&body[inner], project) {
          found.push(DocLink { target, at: span });
        }
      }
      Part::Link { at, destination, wiki } => {
        let target = if wiki {
          target::doc_title(&destination)
        } else {
          target::destination(&destination, project)
        };
        if let Some(target) = target {
          found.push(DocLink { target, at });
        }
      }
    }
  }
  found
}

/// What of a body may hold a link, by where it is in the body.
enum Part {
  /// A run of plain text, as written.
  Text(Range<usize>),
  /// A code span with its backticks, and what it holds but for the spaces around it.
  Code { span: Range<usize>, inner: Range<usize> },
  /// A Markdown link and its destination; for a wiki link, what is between its brackets before a `|`.
  Link {
    at: Range<usize>,
    destination: String,
    wiki: bool,
  },
}

/// The parts of `body` outside code blocks, HTML, images and links' words.
fn parts(body: &str) -> Vec<Part> {
  let options = Options::ENABLE_TABLES
    | Options::ENABLE_TASKLISTS
    | Options::ENABLE_STRIKETHROUGH
    | Options::ENABLE_FOOTNOTES
    | Options::ENABLE_WIKILINKS;
  let mut parts: Vec<Part> = Vec::new();
  let mut inside = 0_usize;
  for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
    match event {
      Event::Start(Tag::Link {
        link_type, dest_url, ..
      }) => {
        if inside == 0 {
          parts.push(Part::Link {
            at: range,
            destination: dest_url.into_string(),
            wiki: matches!(link_type, LinkType::WikiLink { .. }),
          });
        }
        inside += 1;
      }
      Event::Start(Tag::Image { .. } | Tag::CodeBlock(_) | Tag::HtmlBlock) => inside += 1,
      Event::End(TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock | TagEnd::HtmlBlock) => {
        inside = inside.saturating_sub(1);
      }
      Event::Text(shown) if inside == 0 && body.get(range.clone()) == Some(&*shown) => match parts.last_mut() {
        Some(Part::Text(last)) if last.end == range.start => last.end = range.end,
        _ => parts.push(Part::Text(range)),
      },
      Event::Code(shown) if inside == 0 => {
        if let Some(inner) = code_inner(body, &range).filter(|inner| body[inner.clone()] == *shown.trim()) {
          parts.push(Part::Code { span: range, inner });
        }
      }
      _ => {}
    }
  }
  parts
}

/// Where what the code span at `span` of `body` holds is, without its backticks and the spaces around it.
fn code_inner(body: &str, span: &Range<usize>) -> Option<Range<usize>> {
  let source = body.get(span.clone())?;
  let unquoted = source.trim_matches('`');
  let start = span.start + (source.len() - source.trim_start_matches('`').len());
  let held = unquoted.trim_start();
  let start = start + (unquoted.len() - held.len());
  Some(start..start + held.trim_end().len())
}
