//! An item expanded in the panel's list stays in view under the pinned project header (the answering note of
//! `t7zhw`): expanding it scrolls the list by the least that shows it whole, or brings its top to the list's
//! top when it is taller than the list. Its card is read after it expands and may grow then, so each layout
//! looks again, until the person scrolls the list, collapses the item or expands another.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  core::{ElementRef, Signal},
};
use orchester_core::id::ProjectId;
use parking_lot::Mutex;

/// An item of the list, by project and key.
pub(super) type At = (ProjectId, Arc<str>);

/// How far out of the list's view an edge may be before it counts, for rounding.
const SLACK: f32 = 0.5;

#[derive(Clone)]
pub(super) struct Reveal {
  /// The item to keep in view, while it is.
  wanted: Arc<Mutex<Option<At>>>,
  /// The expanded item, and the list's viewport.
  item: ElementRef,
  list: ElementRef,
  /// Moves on when a layout left the wanted item out of view, so the panel renders and asks again.
  again: Signal<u64>,
}

impl Reveal {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      wanted: Arc::default(),
      item: ElementRef::new(),
      list: ElementRef::new(),
      again: ctx.signal(0),
    }
  }

  pub(super) fn want(&self, at: At) {
    *self.wanted.lock() = Some(at);
  }

  pub(super) fn stop(&self) {
    self.wanted.lock().take();
  }

  /// The expanded item's element.
  pub(super) fn item(&self) -> ElementRef {
    self.item.clone()
  }

  /// The list's element.
  pub(super) fn list(&self) -> ElementRef {
    self.list.clone()
  }

  /// In the render of the item expanded `at`: while it is wanted, the list scrolls it into view after this
  /// pass's layout.
  pub(super) fn request(&self, ctx: &mut Ctx, at: &At) {
    self.again.get();
    if self.wanted.lock().as_ref() == Some(at) {
      ctx.scroll_into_view(&self.item);
    }
  }

  /// After a layout: the wanted item grew out of view, or is not yet in it, so the panel asks again. One
  /// taller than the list counts as in view once its top is at the list's top.
  pub(super) fn after_layout(&self) {
    if self.wanted.lock().is_none() || !self.item.is_attached() || !self.list.is_attached() {
      return;
    }
    let (item, view) = (self.item.bounds(), self.list.bounds());
    let above = item.y < view.y - SLACK;
    let below = item.y + item.height > view.y + view.height + SLACK && item.y > view.y + SLACK;
    if above || below {
      self.again.update(|again| *again += 1);
    }
  }
}
