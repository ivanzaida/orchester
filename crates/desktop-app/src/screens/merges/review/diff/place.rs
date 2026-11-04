//! Where the review's threads and the comment box stand in the diff (`Thread · 4xx` `hxP69` under line 44 in 09i): a
//! thread under the last of its lines where the diff shows them, the same code found again when the head moved on
//! (`locate`); one whose lines are not shown at the file's top, after its head. Outdated threads fold together into
//! one line there (*N outdated*), and a resolved one into a line of its own under its lines; a fold opens them again.
//! The comment box stands under the chosen lines, which are marked, after the threads that end there.

use std::{
  collections::{BTreeSet, HashMap},
  sync::Arc,
};

use orchester_core::{
  diff::{Diff, FileDiff},
  pull_request::feedback::{LineSpan, Located, locate},
  review::ThreadState,
  revision::Revision,
};
use orchester_workspace::tasks::review_threads::read::{Review, ThreadView};

use super::super::state::Composing;
use crate::screens::task::changes::diff::{LineKey, Marks};

/// The key of the comment box's block.
pub(in crate::screens::merges::review) const COMPOSER: &str = "composer";

/// What one file's diff shows besides its lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct FilePlaces {
  /// Blocks under their lines, in the order they stand.
  pub blocks: Vec<(LineKey, Arc<str>)>,
  /// Blocks after the file's head, before its lines.
  pub top: Vec<Arc<str>>,
  pub marked: BTreeSet<LineKey>,
}

impl FilePlaces {
  pub fn marks(&self) -> Marks<'_> {
    Marks {
      marked: &self.marked,
      blocks: &self.blocks,
    }
  }
}

/// The key of a thread's block.
pub(in crate::screens::merges::review) fn thread_key(thread: &ThreadView) -> Arc<str> {
  format!("thread:{}", thread.thread.id()).into()
}

/// The key of the fold of a file's outdated threads.
pub(in crate::screens::merges::review) fn outdated_key(path: &str) -> Arc<str> {
  format!("outdated:{path}").into()
}

/// The key of a resolved thread's fold.
pub(in crate::screens::merges::review) fn resolved_key(thread: &ThreadView) -> Arc<str> {
  format!("resolved:{}", thread.thread.id()).into()
}

/// What each file of `diff`, at `head`, shows besides its lines, by path; `opened`: the folds opened.
pub(super) fn places(
  diff: &Diff,
  (review, head): (&Review, &Revision),
  composing: Option<&Composing>,
  opened: &BTreeSet<Arc<str>>,
) -> HashMap<Arc<str>, FilePlaces> {
  let mut places: HashMap<Arc<str>, FilePlaces> = HashMap::new();
  let files: HashMap<&str, &FileDiff> = diff.files().iter().map(|file| (file.path(), file)).collect();
  for view in &review.threads {
    let path = view.thread.anchor().path();
    let file = files.get(path).copied();
    let place = places.entry(Arc::from(path)).or_default();
    let at = located(view, file, head).map(|span| LineKey {
      side: view.thread.anchor().side(),
      number: span.end(),
    });
    match &view.state {
      ThreadState::Outdated { .. } => {
        let fold = outdated_key(path);
        if !place.top.contains(&fold) {
          place.top.insert(0, fold.clone());
        }
        if opened.contains(&fold) {
          place.top.push(thread_key(view));
        }
      }
      ThreadState::Resolved => put(place, at, resolved_key(view)),
      _ => put(place, at, thread_key(view)),
    }
  }
  if let Some(composing) = composing.filter(|composing| composing.head == *head) {
    let place = places.entry(composing.path.clone()).or_default();
    place
      .marked
      .extend((composing.span.start()..=composing.span.end()).map(|number| LineKey {
        side: composing.side,
        number,
      }));
    place.blocks.push((composing.end(), Arc::from(COMPOSER)));
  }
  places
}

/// `key` under line `at`, or at the file's top without it.
fn put(place: &mut FilePlaces, at: Option<LineKey>, key: Arc<str>) {
  match at {
    Some(at) => place.blocks.push((at, key)),
    None => place.top.push(key),
  }
}

/// The lines of `view` in `file`'s diff at `head`: those it was written on at its own head, else where the same code
/// is; `None` when the diff does not show them.
fn located(view: &ThreadView, file: Option<&FileDiff>, head: &Revision) -> Option<LineSpan> {
  let file = file?;
  let anchor = view.thread.anchor();
  let span = if view.thread.head() == head {
    anchor.span()
  } else {
    match locate(anchor, Some(file)) {
      Located::At(span) => span,
      Located::Outdated => return None,
    }
  };
  let end = LineKey {
    side: anchor.side(),
    number: span.end(),
  };
  let shown = file
    .hunks()
    .iter()
    .flat_map(|hunk| hunk.lines())
    .any(|line| LineKey::of_numbers(line.kind(), (line.old_number(), line.new_number())) == Some(end));
  shown.then_some(span)
}
