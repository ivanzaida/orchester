//! What a page draws into a file's diff besides its lines (a pull request's agent feedback, `seN1p`): lines
//! marked as chosen or as a thread's (on `info_muted` with a 2 `info` edge at their start, `LhyWc`), blocks under
//! the line they follow (a thread, a composer: `Thread wrap` `p6wOsh`), which the page draws, and a press on a
//! line, which chooses it. A diff without annotations is read-only, as the task page's Changes tab shows it.

use std::{collections::BTreeSet, sync::Arc};

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{diff::LineKind, pull_request::feedback::DiffSide};

/// A line of a file's diff by the side its number is on: an added or unchanged line by its new number, a removed
/// one by its old.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineKey {
  pub side: DiffSide,
  pub number: u32,
}

impl LineKey {
  /// The key of a line of `kind` with the old and new numbers it has.
  pub fn of_numbers(kind: LineKind, (old, new): (Option<u32>, Option<u32>)) -> Option<Self> {
    match kind {
      LineKind::Removed => old.map(|number| Self {
        side: DiffSide::Old,
        number,
      }),
      LineKind::Added | LineKind::Context => new.map(|number| Self {
        side: DiffSide::New,
        number,
      }),
    }
  }
}

/// Draws the block of key `key`.
pub type BlockView = Arc<dyn Fn(&mut Ctx, &Arc<str>) -> Element + Send + Sync>;
/// A press on line `0`; `1`: Shift was held, which extends a choice.
pub type LinePress = Arc<dyn Fn(LineKey, bool) + Send + Sync>;

/// The page's additions to a file's diff.
#[derive(Clone)]
pub struct Annotations {
  /// Lines drawn as chosen.
  pub marked: Arc<BTreeSet<LineKey>>,
  /// Blocks, each under its line, in the order they stand.
  pub blocks: Arc<Vec<(LineKey, Arc<str>)>>,
  pub block: BlockView,
  pub press: Option<LinePress>,
  /// The block whose row is scrolled into view when it changes.
  pub reveal: Option<Arc<str>>,
  /// Tells these annotations apart from others drawn before: what the marks, blocks or the blocks' content are.
  pub version: u64,
}

/// Annotations are told apart by their version; the closures draw what it names.
impl PartialEq for Annotations {
  fn eq(&self, other: &Self) -> bool {
    self.version == other.version && self.reveal == other.reveal
  }
}

impl Annotations {
  /// The marks and blocks rows are made with.
  pub fn marks(&self) -> Marks<'_> {
    Marks {
      marked: &self.marked,
      blocks: &self.blocks,
    }
  }
}

/// What rows are made with besides the file.
#[derive(Clone, Copy)]
pub struct Marks<'a> {
  pub marked: &'a BTreeSet<LineKey>,
  pub blocks: &'a [(LineKey, Arc<str>)],
}
