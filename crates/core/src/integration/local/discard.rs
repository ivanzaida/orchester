use super::LocalFile;

/// What discarding a reviewed change does to its path. Nothing goes for good: what leaves the checkout goes to the
/// Recycle Bin / Trash, where the person can still restore it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Discard {
  /// A file HEAD does not have, untracked or only staged: it goes to the Recycle Bin / Trash and leaves the index.
  Trash,
  /// A file HEAD has, changed or staged: a copy of the person's version goes to the Recycle Bin / Trash, then HEAD's
  /// content and index entry come back.
  RestoreKeepingCopy,
  /// A file HEAD has that is gone from the checkout: HEAD's content and index entry come back.
  Restore,
  /// A file only the index holds, gone from the checkout: it leaves the index.
  Unstage,
}

impl LocalFile {
  /// What discarding this change does.
  pub const fn discard(&self) -> Discard {
    match (self.head.is_some(), self.working.is_some()) {
      (false, true) => Discard::Trash,
      (false, false) => Discard::Unstage,
      (true, true) => Discard::RestoreKeepingCopy,
      (true, false) => Discard::Restore,
    }
  }
}
