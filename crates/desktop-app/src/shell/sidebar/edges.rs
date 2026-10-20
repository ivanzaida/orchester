//! The edges of the sidebar's scrolling part in a window too short for it: a hairline in `border_panel` under
//! the switcher while rows have scrolled away under it, and over the bottom group while the part has more
//! than it shows, so that rows cut at an edge never read as part of the switcher or the bottom group. Each
//! hairline lies at the edge of an 8 gap toward the scrolling part: under the switcher the design's `Gap`
//! (`Eo8Zf`), which keeps its room without the hairline, and over the bottom group one of the same height
//! that shows only with it, so a window tall enough for everything shows the design's sidebar unchanged.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    events::ScrollEvent,
    theme::{BorderSize, SpacingSize},
  },
  components::Column,
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
  node::{border::Border, dimension::Dimension, padding::Padding},
};

use crate::theme::{ExtraColor, ExtraSpacing};

/// Less than this is no overflow and no scroll: what layout rounding leaves.
const SLACK: f32 = 0.5;

/// Which hairlines show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Edges {
  /// Rows have scrolled away under the switcher.
  scrolled: bool,
  /// The part holds more than it shows.
  overflows: bool,
}

impl DevtoolsInspectable for Edges {}

impl Edges {
  /// The gap under the switcher, with the hairline at its bottom while rows have scrolled under it.
  pub(super) fn top(self) -> Column {
    let gap = gap().id("sidebar-top-edge");
    if self.scrolled {
      lined(gap).border_bottom(hairline())
    } else {
      gap
    }
  }
}

/// What the sidebar keeps to know its edges: the part's scroll, the hairlines shown, and the gap over the
/// bottom group, whose room the part would have without it.
#[derive(Clone)]
pub(super) struct Watch {
  scroll: ScrollState,
  edges: Signal<Edges>,
  bottom: ElementRef,
}

impl Watch {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      scroll: ScrollState::new(),
      edges: ctx.signal(Edges::default()),
      bottom: ElementRef::new(),
    }
  }

  pub(super) fn scroll(&self) -> ScrollState {
    self.scroll.clone()
  }

  /// The hairlines to show, read in a render.
  pub(super) fn edges(&self) -> Edges {
    self.edges.get()
  }

  /// The edges at `offset` from the part's top. The part holds more only if it would without the gap over the
  /// bottom group: with that, the gap never keeps itself, so a window that grows tall enough loses it.
  fn at(&self, offset: f32) -> Edges {
    let room = if self.edges.get_untracked().overflows {
      self.bottom.height() + ExtraSpacing::Tight.value()
    } else {
      0.0
    };
    Edges {
      scrolled: offset > SLACK,
      overflows: self.scroll.content_height() - self.scroll.viewport_height() - room > SLACK,
    }
  }

  /// After a layout, which a resized window or a run that started or ended may have made: the edges as it
  /// left them. A part that held everything and now holds more starts at its top: lurq keeps a scroll that
  /// was at its end there, which would put the screens out of view.
  pub(super) fn after_layout(&self) {
    let now = self.at(self.scroll.scroll_y());
    if now.overflows && now.scrolled && !self.edges.get_untracked().overflows {
      self.scroll.scroll_to_top_pending();
    }
    self.show(now);
  }

  /// On each scroll of the part: the edges at the offset it leads to. A wheel's handlers run before its delta
  /// moves the part, so that is the offset less the delta.
  pub(super) fn on_scroll(&self) -> impl Fn(ScrollEvent) + Send + Sync + 'static {
    let watch = self.clone();
    move |event: ScrollEvent| {
      let scroll = &watch.scroll;
      let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
      watch.show(watch.at((scroll.scroll_y() - event.delta_y).clamp(0.0, end)));
    }
  }

  /// While the part holds more than it shows, a gap over the bottom group with the hairline at its top. It
  /// takes its room only then: the design's sidebar has none there.
  pub(super) fn bottom(&self, edges: Edges) -> Option<Column> {
    edges.overflows.then(|| {
      lined(gap().id("sidebar-bottom-edge"))
        .ref_element(self.bottom.clone())
        .border_top(hairline())
    })
  }

  fn show(&self, now: Edges) {
    if self.edges.get_untracked() != now {
      self.edges.set(now);
    }
  }
}

fn gap() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Sm))
}

/// `gap` marked as showing its hairline, for the window's element tree and the tests.
fn lined(gap: Column) -> Column {
  gap.class("hairline").describe("state", "hairline")
}

fn hairline() -> Border {
  Border::inside(BorderSize::Sm, ExtraColor::PanelBorder.role())
}
