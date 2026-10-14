//! The project header pinned over the panel's list (`o1BDj`, `uioh2`): the list's top is always under the
//! header of the project it is in. lurq has no sticky element, so the header shows in a slot over the list's
//! viewport, and a scroll of the list picks which project's it is.

use lurq::{
  app::{component::DevtoolsInspectable, events::ScrollEvent, theme::SpacingSize},
  components::{Column, Row},
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
  node::{dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;

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

/// Pins, on each scroll of the list, the last of `heads` that has passed wholly under the pinned header, or
/// the first project's before any has. `content` is what the list scrolls. A wheel's handlers run before its
/// delta moves the list, so the offset it is judged at is the one the delta leads to.
pub(super) fn on_scroll(
  scroll: &ScrollState,
  pinned: &Signal<Pinned>,
  content: ElementRef,
  heads: Vec<(ProjectId, ElementRef)>,
) -> impl Fn(ScrollEvent) + Send + Sync + 'static {
  let (scroll, pinned) = (scroll.clone(), pinned.clone());
  move |event: ScrollEvent| {
    let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
    let offset = (scroll.scroll_y() - event.delta_y).clamp(0.0, end);
    let top = content.bounds().y;
    let passed = heads
      .iter()
      .rev()
      .find(|(_, head)| {
        let head = head.bounds();
        head.height > 0.0 && head.y + head.height - top <= offset
      })
      .map(|(project, _)| *project);
    if pinned.get_untracked() != Pinned(passed) {
      pinned.set(Pinned(passed));
    }
  }
}
