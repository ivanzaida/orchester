//! lurq's Markdown with a code block wider than its column (ORC-82): a code block's lines are not wrapped, so a line
//! longer than the column runs past the block's box. The block should scroll sideways inside its box (a horizontal
//! scrollbar, shift+wheel, a trackpad's sideways swipe) so the rest of the line can be read. lurq 0.50.0 gives it
//! no way to: `render_code_block` puts the unwrapped rich text in a plain `Column` with the theme's box, with no
//! scroll container, no theme or props option and no hook for code blocks (`MarkdownProps` has one for lists only).
//! The text is laid out at the box's inner width and its long line is painted past it and clipped, out of reach.
//! These tests pin that gap; when lurq scrolls a code block, flip the last one to assert the line moves by the
//! swipe and the box stays put.

use lurq::{
  app::events::ScrollPhase,
  components::{Column, Markdown, MarkdownProps},
  core::ElementRect,
  node::dimension::Dimension,
};

use super::harness::Kit;

/// The column the document is laid out in.
const COLUMN: f32 = 400.0;
/// The end of the code block's last line, to find its text by.
const LINE_END: &str = "END-OF-THE-LAST-LINE";

/// A reply whose code block holds `first` and a short last line.
fn source(first: &str) -> String {
  format!("A reply with code:\n\n```markdown\n{first}\nshort {LINE_END}\n```\n")
}

/// A line some ten times wider than the column.
fn long_line() -> String {
  "- the long line that runs past the column ".repeat(8)
}

/// `source` in a `COLUMN`-wide column.
fn show(source: String) -> Kit {
  Kit::show(move |ctx| {
    Column::new()
      .width(Dimension::Px(COLUMN))
      .child(Markdown::mount(ctx, MarkdownProps::new(source.clone()).width(Dimension::Pct(100.0))).id("doc"))
      .into()
  })
}

/// Where the code block's text is laid out.
fn code_text(kit: &mut Kit) -> ElementRect {
  kit
    .tree
    .find_element(|element| element.text_content().is_some_and(|text| text.contains(LINE_END)))
    .expect("the code block's text is laid out")
    .bounds()
}

#[test]
fn markdown_code_scroll_keeps_long_lines_unwrapped_in_the_column() {
  let mut long = show(source(&long_line()));
  long.pass();
  let mut short = show(source("short"));
  short.pass();
  let doc = long.bounds("doc");
  let text = code_text(&mut long);
  assert!(doc.width <= COLUMN + 0.5, "the document keeps the column's width: {}", doc.width);
  assert!(text.width < COLUMN, "the code's text is laid out inside its box: {}", text.width);
  assert_eq!(
    text.height,
    code_text(&mut short).height,
    "the long line is not wrapped: the block is as high as one of two short lines"
  );
}

#[test]
fn markdown_code_scroll_gap_a_sideways_swipe_does_not_move_the_line() {
  let mut kit = show(source(&long_line()));
  kit.pass();
  let text = code_text(&mut kit);
  let (x, y) = (text.x + 40.0, text.y + text.height / 2.0);
  kit.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  kit.tree.scroll(x, y, -200.0, 0.0, ScrollPhase::Scroll);
  kit.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  kit.pass();
  assert_eq!(
    code_text(&mut kit).x,
    text.x,
    "lurq 0.50.0: the code block cannot scroll sideways, so the line's end stays out of reach"
  );
}
