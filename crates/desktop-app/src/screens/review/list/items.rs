//! The list's lines from what was read: each group's header line and, unless it is folded, its rows, 2 apart.
//! *Waiting for you* is always there, with one row-high line when nothing waits (`03` `QpQTI`); the lower groups only
//! when they hold rows. The search narrows the rows by their key and title.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::review_list::{ReviewList, ReviewRow};

use super::{
  line::{GroupLine, Line},
  row::RowProps,
  words::{self, Phrase},
};

/// A group of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
  Waiting,
  WithAgent,
  Approved,
}

/// The groups the person folded away.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Folded(Vec<Group>);

impl DevtoolsInspectable for Folded {}

impl Folded {
  /// A list not read before shows every group open; one read before, *Approved today* folded.
  pub fn opened(read_before: bool) -> Self {
    Self(if read_before { vec![Group::Approved] } else { Vec::new() })
  }

  pub fn holds(&self, group: Group) -> bool {
    self.0.contains(&group)
  }
}

/// Folds and unfolds the groups; the screen provides it to the header lines.
#[derive(Clone)]
pub(super) struct Folding(pub Signal<Folded>);

impl Folding {
  pub fn toggle(&self, group: Group) {
    self.0.update(|folded| {
      if folded.holds(group) {
        folded.0.retain(|held| *held != group);
      } else {
        folded.0.push(group);
      }
    });
  }
}

/// The list's lines for `search` and what `folded` holds away.
pub(super) fn lines(ctx: &Ctx, project: ProjectId, list: &ReviewList, (search, folded): (&str, &Folded)) -> Vec<Line> {
  let search = search.trim().to_lowercase();
  let found = |rows: &[ReviewRow]| -> Vec<Line> {
    rows
      .iter()
      .filter(|row| {
        search.is_empty()
          || row.key.to_string().to_lowercase().contains(&search)
          || row.title.to_lowercase().contains(&search)
      })
      .map(|row| Line::Row(Box::new(props(ctx, project, row))))
      .collect()
  };
  let mut lines = Vec::new();
  let waiting = found(&list.waiting);
  let empty = match (waiting.is_empty(), list.waiting.is_empty()) {
    (false, _) => None,
    (true, true) => Some(Line::Empty),
    (true, false) => Some(Line::NoMatch),
  };
  lines.push(group(Group::Waiting, folded, waiting.len()));
  if !folded.holds(Group::Waiting) {
    lines.extend(empty);
    lines.extend(waiting);
  }
  for (part, rows) in [
    (Group::WithAgent, &list.with_agent),
    (Group::Approved, &list.approved_today),
  ] {
    let rows = found(rows);
    if rows.is_empty() {
      continue;
    }
    lines.push(group(part, folded, rows.len()));
    if !folded.holds(part) {
      lines.extend(rows);
    }
  }
  lines
}

fn group(group: Group, folded: &Folded, count: usize) -> Line {
  Line::Group(GroupLine {
    group,
    count,
    folded: folded.holds(group),
  })
}

fn props(ctx: &Ctx, project: ProjectId, row: &ReviewRow) -> RowProps {
  RowProps {
    project,
    key: row.key.clone(),
    title: row.title.clone(),
    tag: row.tag.clone(),
    phrase: Phrase::of(ctx, &row.state),
    comments: row.open_threads,
    size: row.size.map(|size| {
      (
        words::files(ctx, size.files),
        ctx.t_args("review.additions", [("count", size.additions.to_string())]),
        ctx.t_args("review.deletions", [("count", size.deletions.to_string())]),
      )
    }),
    time: words::since(ctx, row.since),
  }
}
