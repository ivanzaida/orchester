//! lurq's Markdown with a code block wider than its column (ORC-82), as the app's theme draws it: a code block's
//! lines are not wrapped, and a line longer than the column scrolls sideways inside the block's box (lurq 0.51's
//! `ScrollHorizontal` around the lines, under the app's scrollbar), so the rest of the line can be read. The box, its
//! label and the column keep their width, a wheel turned up or down over the block is not taken by it, and a
//! selection still copies whole lines, the part scrolled out of view too.

use lurq::{
  app::events::{MouseButton, ScrollPhase},
  components::{Column, Markdown, MarkdownProps, ScrollVertical},
  core::ElementRect,
  node::dimension::Dimension,
};

use super::harness::Kit;

/// The column the document is laid out in.
const COLUMN: f32 = 400.0;
/// The end of the code block's last line, to find its text by.
const LINE_END: &str = "END-OF-THE-LAST-LINE";
/// The end of the long line.
const LONG_END: &str = "THE-LONG-LINE-ENDS";
/// The code block's label: its fence's language.
const LABEL: &str = "markdown";

/// A reply whose code block holds `first` and a short last line, between enough paragraphs to scroll the view.
fn source(first: &str) -> String {
  let filler = "A paragraph around the code.\n\n".repeat(12);
  format!("{filler}```{LABEL}\n{first}\nshort {LINE_END}\n```\n\n{filler}")
}

/// A line some eight times wider than the column.
fn long_line() -> String {
  format!("{}{LONG_END}", "- the long line that runs past the column ".repeat(8))
}

/// `source` in a `COLUMN`-wide column, selectable as the app's chats show it, in a 300-high vertical scroll.
fn show(source: String) -> Kit {
  Kit::show(move |ctx| {
    let markdown = MarkdownProps::new(source.clone())
      .width(Dimension::Pct(100.0))
      .selectable(true);
    ScrollVertical::new(
      Column::new()
        .width(Dimension::Px(COLUMN))
        .child(Markdown::mount(ctx, markdown).id("doc")),
    )
    .id("view")
    .width(Dimension::Px(COLUMN))
    .height(Dimension::Px(300.0))
    .into()
  })
}

/// Where the code block's lines are laid out.
fn code_text(kit: &mut Kit) -> ElementRect {
  kit
    .tree
    .find_element(|element| element.text_content().is_some_and(|text| text.contains(LINE_END)))
    .expect("the code block's lines are laid out")
    .bounds()
}

/// The code block's label.
fn label(kit: &mut Kit) -> ElementRect {
  kit
    .tree
    .find_element(|element| element.text_content() == Some(LABEL))
    .expect("the code block's label is laid out")
    .bounds()
}

/// A swipe of `delta` with the pointer at `at`, as a trackpad sends it: a positive delta scrolls toward the start.
fn wheel(kit: &mut Kit, (x, y): (f32, f32), (dx, dy): (f32, f32)) {
  kit.tree.mouse_move(x, y);
  kit.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  kit.tree.scroll(x, y, dx, dy, ScrollPhase::Scroll);
  kit.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  kit.pass();
  kit.pass();
}

/// A long code block, scrolled to 100 below the top of the 300-high view.
fn long_block() -> Kit {
  let mut kit = show(source(&long_line()));
  kit.pass();
  let (view, text) = (kit.bounds("view"), code_text(&mut kit));
  wheel(&mut kit, (view.x + 20.0, view.y + 20.0), (0.0, view.y + 100.0 - text.y));
  kit
}

/// Over the code block's lines, near their start.
fn over_code(kit: &mut Kit) -> (f32, f32) {
  let text = code_text(kit);
  (kit.bounds("doc").x + 40.0, text.y + text.height / 2.0)
}

#[test]
fn markdown_code_scroll_keeps_long_lines_unwrapped_in_the_column() {
  let mut long = long_block();
  let mut short = show(source("short"));
  short.pass();
  let doc = long.bounds("doc");
  let text = code_text(&mut long);
  assert!(
    doc.width <= COLUMN + 0.5,
    "the document keeps the column's width: {}",
    doc.width
  );
  assert!(
    text.width > COLUMN * 4.0,
    "the long line is laid out whole, past the column: {}",
    text.width
  );
  assert_eq!(
    text.height,
    code_text(&mut short).height,
    "the long line is not wrapped: the block is as high as one of two short lines"
  );
  assert!(
    label(&mut long).width < COLUMN,
    "the label stays as it is, above the lines"
  );
}

#[test]
fn markdown_code_scroll_a_sideways_swipe_moves_the_line_inside_its_box() {
  let mut kit = long_block();
  let (doc, text, tag) = (kit.bounds("doc"), code_text(&mut kit), label(&mut kit));
  let at = over_code(&mut kit);
  wheel(&mut kit, at, (-200.0, 0.0));
  let moved = code_text(&mut kit);
  assert!(
    (moved.x - (text.x - 200.0)).abs() < 0.5,
    "the swipe scrolls the line by its delta: {} then {}",
    text.x,
    moved.x
  );
  assert!((moved.y - text.y).abs() < 0.5, "the block does not move up or down");
  assert_eq!(kit.bounds("doc"), doc, "the document stays put");
  assert_eq!(label(&mut kit), tag, "the label stays put, outside the scroll");
  // As far as it goes: the line's end comes into the box, so nothing is out of reach.
  for _ in 0..20 {
    wheel(&mut kit, at, (-2000.0, 0.0));
  }
  let end = code_text(&mut kit);
  let (line_right, doc_right) = (end.x + end.width, doc.x + doc.width);
  assert!(
    line_right <= doc_right + 0.5 && line_right > doc_right - 40.0,
    "the line's end is in the box at the end of the scroll: it ends at {line_right}, the box at {doc_right}"
  );
  for _ in 0..20 {
    wheel(&mut kit, at, (2000.0, 0.0));
  }
  assert!(
    (code_text(&mut kit).x - text.x).abs() < 0.5,
    "a swipe back returns to the line's start"
  );
}

#[test]
fn markdown_code_scroll_a_wheel_up_or_down_over_the_block_scrolls_the_view() {
  let mut kit = long_block();
  let text = code_text(&mut kit);
  let at = over_code(&mut kit);
  wheel(&mut kit, at, (0.0, -60.0));
  let moved = code_text(&mut kit);
  assert!(
    (moved.y - (text.y - 60.0)).abs() < 0.5,
    "the view scrolls under a wheel over the block: {} then {}",
    text.y,
    moved.y
  );
  assert!((moved.x - text.x).abs() < 0.5, "the block does not scroll sideways");
}

#[test]
fn markdown_code_scroll_a_selection_copies_whole_lines() {
  let mut kit = long_block();
  let text = code_text(&mut kit);
  // From the start of the long line down into the short line under it.
  let (from_x, from_y) = (text.x + 1.0, text.y + 2.0);
  let (to_x, to_y) = (kit.bounds("doc").x + 300.0, text.y + text.height - 2.0);
  kit.tree.mouse_move(from_x, from_y);
  kit.tree.mouse_down(from_x, from_y, MouseButton::Left);
  kit.tree.mouse_move(to_x, to_y);
  kit.tree.mouse_up(to_x, to_y, MouseButton::Left);
  kit.pass();
  let copied = kit.tree.selected_text().expect("the drag selects the code");
  assert!(
    copied.contains(&long_line()),
    "the selection copies the long line whole, its end out of view too: {copied:?}"
  );
  assert!(
    copied.contains(LINE_END),
    "the selection goes on to the end of the next line: {copied:?}"
  );
}
