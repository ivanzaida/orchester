//! The project header pinned over the panel's list (`o1BDj`, `uioh2`): the list's top is always under the
//! header of the project it is in. lurq has no sticky element, so the header shows in a slot over the list's
//! viewport, and a scroll of the list picks which project's it is: the person's, and one the panel makes to
//! keep an expanded item in view (`reveal`), seen after the layout it moved.

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, events::ScrollEvent, theme::SpacingSize},
  components::{Column, Row},
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
  node::{dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use parking_lot::Mutex;

use crate::theme::ExtraSpacing;

/// The project whose header is pinned: the one the list's top is in, `None` for the first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Pinned(pub(super) Option<ProjectId>);

impl DevtoolsInspectable for Pinned {}

/// Where the pinned header shows, over the list's viewport: 4 over it and 2 under it, as over the first item.
pub(super) fn slot(header: Option<Row>) -> Column {
  Column::new()
    .id("needs-you-pinned")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xs)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(ExtraSpacing::Tight.role()),
    )
    .with_children(header)
}

/// Which project's header is pinned, judged from what the list scrolls (`content`) and the headers of the
/// projects after the first, which scroll with it.
#[derive(Clone)]
pub(super) struct Pin {
  pinned: Signal<Pinned>,
  content: ElementRef,
  heads: Arc<Mutex<Vec<(ProjectId, ElementRef)>>>,
}

impl Pin {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      pinned: ctx.signal(Pinned::default()),
      content: ElementRef::new(),
      heads: Arc::default(),
    }
  }

  /// The pinned project, read in a render.
  pub(super) fn get(&self) -> Pinned {
    self.pinned.get()
  }

  pub(super) fn content(&self) -> ElementRef {
    self.content.clone()
  }

  /// The headers of the projects after the first, as the render laid them out.
  pub(super) fn heads(&self, heads: Vec<(ProjectId, ElementRef)>) {
    *self.heads.lock() = heads;
  }

  /// Pins, on each scroll of the list, the project the offset it leads to is in. A wheel's handlers run before
  /// its delta moves the list, so that is the offset less the delta.
  pub(super) fn on_scroll(&self, scroll: &ScrollState) -> impl Fn(ScrollEvent) + Send + Sync + 'static {
    let (pin, scroll) = (self.clone(), scroll.clone());
    move |event: ScrollEvent| {
      let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
      pin.at((scroll.scroll_y() - event.delta_y).clamp(0.0, end));
    }
  }

  /// After a layout, pins the project the list's offset is in now, which a scroll the panel made moved.
  pub(super) fn after_layout(&self, scroll: &ScrollState) {
    self.at(scroll.scroll_y());
  }

  /// Pins the last header that has passed wholly under the pinned one at `offset`, or the first project's
  /// before any has.
  fn at(&self, offset: f32) {
    let top = self.content.bounds().y;
    let passed = self
      .heads
      .lock()
      .iter()
      .rev()
      .find(|(_, head)| {
        let head = head.bounds();
        head.height > 0.0 && head.y + head.height - top <= offset
      })
      .map(|(project, _)| *project);
    if self.pinned.get_untracked() != Pinned(passed) {
      self.pinned.set(Pinned(passed));
    }
  }
}
