//! Where a thread's lines are at a later head (`docs/design/orchester-workspace.md`: "New commits never discard
//! comments or move them to unrelated code"): the same code on the same side of the file, found among the lines
//! the pull request's diff shows at that head. Code no longer there leaves the thread outdated; it keeps the code
//! it was about, and nothing guesses a new place for it.

use super::{DiffSide, LineAnchor, LineSpan};
use crate::diff::FileDiff;

/// Where an anchor's lines are in a diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Located {
  /// The same code on lines `0`, which may be the lines it was written on.
  At(LineSpan),
  /// The code is not among the lines of the diff: removed, changed, or the file is gone.
  Outdated,
}

/// Where `anchor`'s code is among the lines `file`, the diff of its file at a later head, shows on the anchor's
/// side: the run of lines holding exactly its code, the one nearest to where it was when there are more. `None`
/// for the file: it has no change in the pull request any more.
pub fn locate(anchor: &LineAnchor, file: Option<&FileDiff>) -> Located {
  let Some(file) = file else {
    return Located::Outdated;
  };
  let lines: Vec<(u32, &str)> = file
    .hunks()
    .iter()
    .flat_map(|hunk| hunk.lines())
    .filter_map(|line| {
      let number = match anchor.side() {
        DiffSide::Old => line.old_number(),
        DiffSide::New => line.new_number(),
      };
      number.map(|number| (number, line.text()))
    })
    .collect();
  let excerpt = anchor.excerpt();
  let wanted = anchor.span().start();
  lines
    .windows(excerpt.len())
    .filter(|window| {
      window
        .iter()
        .zip(excerpt)
        .all(|((_, text), code)| *text == code.as_str())
        && window.windows(2).all(|pair| pair[1].0 == pair[0].0 + 1)
    })
    .map(|window| window[0].0)
    .min_by_key(|start| start.abs_diff(wanted))
    .and_then(|start| {
      let end = start + anchor.span().len() - 1;
      LineSpan::new(start.try_into().ok()?, end.try_into().ok()?).ok()
    })
    .map_or(Located::Outdated, Located::At)
}
