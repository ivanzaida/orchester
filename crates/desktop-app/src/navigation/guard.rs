//! A mounted editor can hold any departure and offer an in-place leave decision.
use std::sync::{Arc, Weak};

use lurq::core::Signal;

use super::{Navigation, tabs::TabSet};
use crate::routes::Route;

pub(super) type Within = Box<dyn Fn(&Route) -> bool + Send + Sync>;

pub(super) struct PendingLeave {
  holds_edits: Box<dyn Fn() -> bool + Send + Sync>,
  /// Whether a place is the editor's own, e.g. another of its panels: going there in the same tab leaves
  /// nothing.
  within: Within,
  destination: Signal<Option<TabSet>>,
}

impl PendingLeave {
  pub(super) fn hold(&self, destination: TabSet) -> bool {
    if !(self.holds_edits)() {
      return false;
    }
    self.destination.set(Some(destination));
    true
  }

  pub(super) fn within(&self, route: &Route) -> bool {
    (self.within)(route)
  }
}

/// Owned by the mounted editor. Navigation holds only a weak reference, so unmounting releases it.
#[derive(Clone)]
pub struct LeaveGuard(Arc<PendingLeave>);

impl LeaveGuard {
  pub(super) fn new(holds_edits: impl Fn() -> bool + Send + Sync + 'static, within: Within) -> Self {
    Self(Arc::new(PendingLeave {
      holds_edits: Box::new(holds_edits),
      within,
      destination: Signal::new(None),
    }))
  }

  pub(super) fn downgrade(&self) -> Weak<PendingLeave> {
    Arc::downgrade(&self.0)
  }

  /// Reading this in render subscribes the editor to attempted departures.
  pub fn requested(&self) -> bool {
    self.0.destination.get().is_some()
  }

  pub fn keep_editing(&self) {
    self.0.destination.set(None);
  }

  /// Called only by the explicit leave action, once any pending write has finished.
  pub fn leave(&self, navigation: &Navigation) {
    let destination = self.0.destination.get_untracked();
    self.keep_editing();
    if let Some(destination) = destination {
      navigation.apply(destination);
    }
  }
}
