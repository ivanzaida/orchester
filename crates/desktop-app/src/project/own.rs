//! A screen's own change of a project's data, recorded as automation's are (`OutsideChanges`) so every other
//! screen that shows it reads it again, while the screen that made it reads it again once, at once.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::outside::{Outside, OutsideChanges, OutsideCounts};

use super::ProjectFeed;

/// How often the kinds of a project's data a screen shows changed, as a screen that changes them too follows
/// them.
#[derive(Clone)]
pub struct OwnChanges {
  kinds: &'static [Outside],
  /// The count right after the screen's own last change.
  seen: Signal<u64>,
}

impl OwnChanges {
  pub fn new(ctx: &mut Ctx, kinds: &'static [Outside]) -> Self {
    Self {
      kinds,
      seen: ctx.signal(0),
    }
  }

  /// How often the kinds changed: the feed's count, or the count right after the screen's own last change while
  /// the feed has not caught up with it. Reading it in a render renders again when it moves.
  pub fn count(&self, feed: &ProjectFeed) -> u64 {
    self.of(feed.outside.get()).max(self.seen.get())
  }

  /// Whether the feed counted a change after the screen's own last one.
  pub fn newer(&self, feed: &ProjectFeed) -> bool {
    self.of(feed.outside.get()) > self.seen.get()
  }

  /// Records the screen's own change of `kind` in `project`, which the screen reads again itself in the same
  /// render: when the feed catches up with it, [`Self::count`] stays where it is.
  pub fn record(&self, changes: &OutsideChanges, project: ProjectId, kind: Outside) {
    self.saw(changes.record(project, kind));
  }

  /// Takes `counts`, the project's counts right after the screen's own change was recorded, as seen: the feed
  /// reaching them reads nothing again, a change counted after them does.
  pub fn saw(&self, counts: OutsideCounts) {
    let seen = self.of(counts);
    if seen > self.seen.get_untracked() {
      self.seen.set(seen);
    }
  }

  fn of(&self, counts: OutsideCounts) -> u64 {
    self.kinds.iter().map(|kind| counts.of(*kind)).sum()
  }
}
