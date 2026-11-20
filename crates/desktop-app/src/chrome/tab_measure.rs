//! What the strip learns from its own layout, so that a tab is as wide as what it shows: lurq measures no text
//! before layout, and a window tab in the list mode is as wide as the app says. Each tab's label is laid out once
//! more, unseen, in the tab's own typography, and each tab's mark is watched where it stands; both are known before
//! the frame is painted.

use std::collections::BTreeMap;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Row, Text},
  core::{ElementRef, Signal},
  node::{Element, HitTestBehavior, dimension::Dimension},
};
use parking_lot::Mutex;

use crate::navigation::tabs::TabId;

/// A width this close to the last one is the same layout.
const SAME_WIDTH: f32 = 0.5;

/// Widths by tab, as they were laid out last.
type Widths = BTreeMap<u64, f32>;

/// One measured width of each tab: its element, and the widths it was laid out at.
struct Measure {
  refs: Mutex<BTreeMap<u64, ElementRef>>,
  widths: Signal<Widths>,
}

impl Measure {
  fn new(ctx: &mut Ctx) -> Self {
    Self {
      refs: Mutex::new(BTreeMap::new()),
      widths: ctx.signal(BTreeMap::new()),
    }
  }

  /// The element of `tab`'s width, the same from one render to the next, watched from now on: a new width renders
  /// the strip again before the frame is painted. An element that is not laid out (a mark of a tab in the menu of
  /// the tabs that do not fit) keeps the width it had.
  fn watch(&self, tab: TabId) -> ElementRef {
    let element = self.refs.lock().entry(tab.get()).or_default().clone();
    let widths = self.widths.clone();
    element.observe_rect_before_paint(move |rect| {
      let known = widths
        .get_untracked()
        .get(&tab.get())
        .is_some_and(|width| (width - rect.width).abs() < SAME_WIDTH);
      if rect.width > 0.0 && !known {
        widths.update(|widths| {
          widths.insert(tab.get(), rect.width);
        });
      }
    });
    element
  }

  fn width(&self, tab: TabId) -> Option<f32> {
    self.widths.get().get(&tab.get()).copied()
  }

  /// Forgets the tabs that are no longer open.
  fn keep(&self, open: &[TabId]) {
    let open = |id: &u64| open.iter().any(|tab| tab.get() == *id);
    self.refs.lock().retain(|id, _| open(id));
    if self.widths.get_untracked().keys().any(|id| !open(id)) {
      self.widths.update(|widths| widths.retain(|id, _| open(id)));
    }
  }
}

/// The tabs' labels and marks as they were laid out.
pub struct TabWidths {
  labels: Measure,
  marks: Measure,
}

impl TabWidths {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      labels: Measure::new(ctx),
      marks: Measure::new(ctx),
    }
  }

  /// The width of `tab`'s label in its typography, once it has been laid out.
  pub fn label(&self, tab: TabId) -> Option<f32> {
    self.labels.width(tab)
  }

  /// The width of `tab`'s mark, once it has been laid out; none without one.
  pub fn mark(&self, tab: TabId) -> f32 {
    self.marks.width(tab).unwrap_or(0.0)
  }

  /// `mark`, `tab`'s mark, watched.
  pub fn watch_mark(&self, tab: TabId, mark: Element) -> Element {
    Row::new().ref_element(self.marks.watch(tab)).child(mark).into()
  }

  /// The labels `labels` of the open tabs laid out where nothing shows them and the pointer and tooling pass them
  /// by, each in the typography of its tab (`style`): at their whole width, out of a row that takes no room.
  pub fn unseen(&self, labels: &[(TabId, String, TypographyStyle)]) -> Element {
    let open: Vec<TabId> = labels.iter().map(|(tab, ..)| *tab).collect();
    self.labels.keep(&open);
    self.marks.keep(&open);
    labels
      .iter()
      .fold(
        Row::new()
          .width(Dimension::Px(0.0))
          .height(Dimension::Px(0.0))
          .overflow_visible()
          .opacity(0.0)
          .hit_test(HitTestBehavior::None)
          .describe("a11y_hidden", "true"),
        |row, (tab, label, style)| {
          row.child(
            Text::new(label)
              .variant(*style)
              .nowrap()
              .flex_shrink(0.0)
              .ref_element(self.labels.watch(*tab)),
          )
        },
      )
      .into()
  }
}
