//! Segmented control (`Segmented/text` `S5Fsve`, `Segmented` `C9VsTA`): lurq's `ui::SegmentedControl` in the theme's
//! recipe (`theme::kit::segmented`), each segment a choice of its own id. One Tab stop whose arrows move the choice
//! in a form, Tab passing it by elsewhere; a press of the chosen segment does nothing. A press is the control's own:
//! it does not reach a row around it.

use std::sync::Arc;

use lurq::{
  app::events::{MouseButton, MouseEvent},
  components::Row,
  core::ElementRef,
  node::{Element, dimension::Dimension},
  ui::{Choice, SegmentedControl},
};

use crate::ui::{icon::Icon, pressable::Focus};

/// The states of a segment the theme's recipe names, beside lurq's `selected`.
pub mod face {
  /// A glyph alone.
  pub const ICON: &str = "icon_face";
  /// A glyph before the label.
  pub const LABELLED: &str = "labelled";
  /// A choice that warns.
  pub const WARNED: &str = "warned";
}

/// What a segment shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentFace {
  Text(Arc<str>),
  /// A label and its count, 6 apart, the count in 12 `text_muted` (`Segment · All` in `Agents · v2` A1).
  Counted(Arc<str>, Arc<str>),
  /// A glyph; the label names it for tools.
  Icon(Icon, Arc<str>),
  /// A 13 glyph before the label, 6 apart, both in the label's color (`System` `J9tyr` in Settings › General).
  Labelled(Icon, Arc<str>),
  /// A choice that warns: an 11 `triangle-alert` in `danger` before the label, 5 apart; chosen, on `danger_muted` with
  /// its label in `danger` (`Segment · Dangerous auto-approve` `cky7h`, `vBuWb` in `v5d5r`).
  Warned(Arc<str>),
}

pub struct Segment {
  id: Arc<str>,
  face: SegmentFace,
  chosen: bool,
  choose: Arc<dyn Fn() + Send + Sync>,
  element: Option<ElementRef>,
  /// The segments share the control's width equally.
  fill: bool,
}

impl Segment {
  pub fn new(
    id: impl Into<Arc<str>>,
    face: SegmentFace,
    chosen: bool,
    choose: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      face,
      chosen,
      choose: Arc::new(choose),
      element: None,
      fill: false,
    }
  }

  /// The segment's element, for a popup it opens to anchor to: lurq switches to that popup in one press, from
  /// another open dropdown, only when the pressed segment is the popup's anchor itself.
  pub fn element(mut self, element: ElementRef) -> Self {
    self.element = Some(element);
    self
  }

  /// The segment takes an equal share of a control as wide as its place (`Layout` `O1Xwma` in `J0ncJ`).
  #[must_use]
  pub const fn fill(mut self) -> Self {
    self.fill = true;
    self
  }

  /// The segment as the choice of `index`, on `element`: the caller's, else `own`.
  fn choice(self, index: usize, own: ElementRef) -> Choice<Option<usize>> {
    let (label, glyph, state, count) = match self.face {
      SegmentFace::Text(label) => (label, None, None, None),
      SegmentFace::Counted(label, count) => (label, None, None, Some(count)),
      SegmentFace::Icon(glyph, label) => (label, Some(glyph), Some(face::ICON), None),
      SegmentFace::Labelled(glyph, label) => (label, Some(glyph), Some(face::LABELLED), None),
      SegmentFace::Warned(label) => (label, Some(Icon::TriangleAlert), Some(face::WARNED), None),
    };
    let mut choice = Choice::new(Some(index), &*label).id(self.id);
    if let Some(count) = count {
      choice = choice.count(count);
    }
    if let Some(glyph) = glyph {
      choice = choice.icon(glyph.name()).icon_only(state == Some(face::ICON));
    }
    if let Some(state) = state {
      choice = choice.state(state, true);
    }
    choice.element_ref(self.element.unwrap_or(own))
  }
}

/// A segmented control of `segments`; in a form it is a Tab stop.
pub fn segmented(id: &str, segments: impl IntoIterator<Item = Segment>, focus: Focus) -> Row {
  let segments: Vec<Segment> = segments.into_iter().collect();
  let chosen = segments.iter().position(|segment| segment.chosen);
  let fill = segments.iter().any(|segment| segment.fill);
  let chooses: Vec<_> = segments.iter().map(|segment| segment.choose.clone()).collect();
  let elements: Vec<ElementRef> = segments
    .iter()
    .map(|segment| segment.element.clone().unwrap_or_default())
    .collect();
  let choices: Vec<_> = segments
    .into_iter()
    .zip(elements.iter().cloned())
    .enumerate()
    .map(|(index, (segment, element))| segment.choice(index, element))
    .collect();
  let control = SegmentedControl::controlled(chosen, choices, move |index: &Option<usize>| {
    if let Some(choose) = index.and_then(|index| chooses.get(index)) {
      choose();
    }
  })
  .id(id.to_owned())
  .full_width(fill)
  .tab_stop(focus != Focus::Pointer);
  let pressed = Arc::new(move |event: &MouseEvent| {
    if on_a_segment(&elements, event) {
      event.stop_propagation();
    }
  });
  let middle = pressed.clone();
  let room = Row::new()
    .on_click(move |event: MouseEvent| pressed(&event))
    .on_mouse_click(MouseButton::Middle, move |event: MouseEvent| middle(&event));
  let room = if fill { room.width(Dimension::Pct(100.0)) } else { room };
  room.child(Element::from(control))
}

/// Whether `event` pressed one of the segments on `elements`. A press of a segment is the control's: lurq hands a click
/// to every ancestor, and an openable row around it must not open too, with the left button or the middle one. A press
/// beside the segments, on the control's edge, reaches the row as before.
fn on_a_segment(elements: &[ElementRef], event: &MouseEvent) -> bool {
  elements.iter().any(|element| {
    let bounds = element.bounds();
    (bounds.x..bounds.x + bounds.width).contains(&event.x) && (bounds.y..bounds.y + bounds.height).contains(&event.y)
  })
}
