//! A screen's own change of a project's data, recorded as automation's are (`OutsideChanges`) so every other
//! screen that shows it reads it again, while the screen that made it reads it again once, at once.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::outside::{Outside, OutsideChanges};

use super::ProjectFeed;

/// How often one kind of a project's data changed, as a screen that changes it too follows it.
#[derive(Clone)]
pub struct OwnChanges {
  kind: Outside,
  /// The count right after the screen's own last change.
  seen: Signal<u64>,
}

impl OwnChanges {
  pub fn new(ctx: &mut Ctx, kind: Outside) -> Self {
    Self {
      kind,
      seen: ctx.signal(0),
    }
  }

  /// How often the kind changed: the feed's count, or the count right after the screen's own last change while
  /// the feed has not caught up with it. Reading it in a render renders again when it moves.
  pub fn count(&self, feed: &ProjectFeed) -> u64 {
    feed.outside.get().of(self.kind).max(self.seen.get())
  }

  /// Records the screen's own change of `project`, which the screen reads again itself in the same render: when
  /// the feed catches up with it, [`Self::count`] stays where it is.
  pub fn record(&self, changes: &OutsideChanges, project: ProjectId) {
    changes.record(project, self.kind);
    self.seen.set(changes.counts(project).of(self.kind));
  }
}
