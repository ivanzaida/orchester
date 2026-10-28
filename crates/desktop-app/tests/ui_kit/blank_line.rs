//! A blank line of text, a single space in a room no wider than nothing, is as high as a line of words in the same
//! style: the Orchestrator's decision cards keep their facts' room with such lines while the facts are read
//! (`screens::orchestrator::change::reading`), so the card does not grow when they arrive.

use lurq::{
  components::{Column, Row, Text},
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_app::theme::OrchestratorText;

use super::harness::{Kit, assert_close};

#[test]
fn a_blank_line_is_as_high_as_a_line_of_words() {
  let mut kit = Kit::show(|_ctx| {
    Row::new()
      .align_items(Alignment::Start)
      .child(
        Column::new().id("blank").width(Dimension::Px(0.0)).child(
          Text::new(" ")
            .variant(OrchestratorText::FactValue.role())
            .width(Dimension::Px(0.0)),
        ),
      )
      .child(
        Column::new()
          .id("words")
          .child(Text::new("Run #2 · Feature delivery").variant(OrchestratorText::FactValue.role())),
      )
      .into()
  });
  let (blank, words) = (kit.bounds("blank"), kit.bounds("words"));
  assert!(words.height > 0.0, "{words:?}");
  assert_close(blank.height, words.height, "a blank line's height");
}
