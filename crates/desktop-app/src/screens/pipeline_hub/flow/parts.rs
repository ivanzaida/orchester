//! What stands between the cards of the flow: the start (`Trigger` `KZ0KL`), an approval gate (`Gate` `Kr7gZ`), the
//! frame of steps that run side by side (`Parallel` `Aft8T`) and the end (`End` `jK7w7`) in `Steps` `uSoll`.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::pipelines::flow::Gate;

use crate::theme::{ExtraSpacing, FlowText};

/// A line of words on the rail's side: a label and what it says after it (`Trigger` `KZ0KL`, `End` `jK7w7`), as high as
/// a node.
pub(super) const LINE_HEIGHT: f32 = 22.0;

/// *ON TASK START* and what starts the run.
pub(super) fn start(label: &str, hint: &str) -> Row {
  words_line(
    "pipeline-hub-flow-start",
    (label, FlowText::Label),
    (hint, FlowText::Hint),
  )
}

/// *MERGED INTO THE BASE BRANCH* and what it means for the task.
pub(super) fn end(label: &str, hint: &str) -> Row {
  words_line(
    "pipeline-hub-flow-end",
    (label, FlowText::Label),
    (hint, FlowText::Hint),
  )
}

/// *Waits for your approval* and what is approved (`Gate` `Kr7gZ`).
pub(super) fn gate(index: usize, title: &str, hint: &str, step: &Gate) -> Row {
  words_line(
    &format!("pipeline-hub-flow-gate-{index}"),
    (title, FlowText::Gate),
    (hint, FlowText::Hint),
  )
  .describe("step", step.step.to_string())
}

fn words_line(id: &str, (first, first_style): (&str, FlowText), (then, then_style): (&str, FlowText)) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LINE_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(first)
        .id(format!("{id}-label"))
        .variant(first_style.role())
        .nowrap(),
    )
    .child(
      Text::new(then)
        .id(format!("{id}-hint"))
        .variant(then_style.role())
        .nowrap(),
    )
}

/// The steps that run side by side in one frame: *RUN IN PARALLEL · both must pass* over the cards, two in a row; more
/// than two stack inside it.
pub(super) fn parallel(id: &str, (label, hint): (&str, &str), cards: Vec<Element>) -> Element {
  let head = Row::new()
    .id(format!("{id}-head"))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .left(SpacingSize::Xs)
        .right(SpacingSize::Xs),
    )
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(FlowText::Label.role())
        .nowrap(),
    )
    .child(
      Text::new(hint)
        .id(format!("{id}-hint"))
        .variant(FlowText::Hint.role())
        .nowrap(),
    );
  let cards: Element = if cards.len() > 2 {
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(cards)
      .into()
  } else {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Stretch)
      .with_children(
        cards
          .into_iter()
          .map(|card| Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(card)),
      )
      .into()
  };
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(head)
    .child(cards)
    .into()
}
