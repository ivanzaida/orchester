//! Agent-written Markdown in the chat (ORC-78): a message is cut into its Markdown and its top-level
//! code blocks; it keeps the line breaks it was written with; search and one-line views read it as
//! rendered; laid out, its long code lines scroll inside their block and its tables share the
//! text's width; hundreds of messages lay out, and a growing one moves nothing above it.

use std::{sync::Arc, time::Instant};

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx, events::ScrollPhase},
  components::Column,
  core::{ElementRect, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_desktop::{
  i18n,
  markdown::{
    MarkdownText,
    plain::{has_markup, plain_text},
    segments::{Segment, keep_line_breaks, segments},
  },
  theme,
};

#[test]
fn markdown_segments_cut_out_top_level_code_blocks() {
  let source = "Intro **bold**\n\n```rust\nfn main() {}\n```\nAfter.\n";
  assert_eq!(
    segments(source),
    vec![
      Segment::Markdown("Intro **bold**\n\n".to_owned()),
      Segment::Code {
        language: Some("rust"),
        code: "fn main() {}",
      },
      Segment::Markdown("After.\n".to_owned()),
    ]
  );
  // A fence that is not closed yet, as while a message grows, runs to the end.
  assert_eq!(
    segments("Text\n~~~\nline one\nline two"),
    vec![
      Segment::Markdown("Text\n".to_owned()),
      Segment::Code {
        language: None,
        code: "line one\nline two",
      },
    ]
  );
  // A longer closing fence closes it; a shorter one is a line of the block.
  assert_eq!(
    segments("````\n```\nstill code\n`````\n"),
    vec![Segment::Code {
      language: None,
      code: "```\nstill code",
    }]
  );
  // A fence inside a list item, and inline code of three backticks, stay in their Markdown.
  for source in ["- item\n  ```\n  code\n  ```\n", "```a``b\n"] {
    assert!(matches!(&segments(source)[..], [Segment::Markdown(_)]), "{source:?}");
  }
  assert!(segments("  \n\n").is_empty(), "blank text has no part");
}

#[test]
fn markdown_keeps_the_line_breaks_a_message_was_written_with() {
  assert_eq!(keep_line_breaks("one\ntwo\n\nthree"), "one  \ntwo\n\nthree\n");
  assert_eq!(keep_line_breaks("one  \ntwo\r\nthree"), "one  \ntwo  \nthree\n");
  assert_eq!(plain_text("one\ntwo"), "one\ntwo");
}

#[test]
fn markdown_plain_text_reads_as_rendered() {
  let source = "# Title\n\nSome **bold** text with `code` and [a link](https://example.com).";
  assert_eq!(plain_text(source), "Title\nSome bold text with code and a link.");
  assert_eq!(
    plain_text("- one\n- two\n\n| A | B |\n|---|---|\n| a | b |"),
    "one\ntwo\nA B \na b"
  );
  assert!(has_markup(source));
  assert!(!has_markup("Appending the line."));
}

#[derive(Clone)]
struct Props {
  texts: Signal<Vec<Arc<str>>>,
}

impl PartialEq for Props {
  fn eq(&self, _other: &Self) -> bool {
    true
  }
}

impl lurq::app::component::DevtoolsInspectable for Props {}

/// Messages one under the other, as the chat's column holds them.
struct Messages;

impl Component for Messages {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let texts = ctx.props::<Props>().texts.get();
    let messages: Vec<Element> = texts
      .iter()
      .enumerate()
      .map(|(index, text)| MarkdownText::message(format!("message-{index}"), text.clone()).mount(ctx))
      .collect();
    Column::new()
      .id("messages")
      .width(Dimension::Pct(100.0))
      .with_children(messages)
  }
}

fn app() -> App {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales");
  app
}

fn mount(app: &mut App, width: u32, texts: Vec<Arc<str>>) -> (Tree, Signal<Vec<Arc<str>>>) {
  let texts = Signal::new(texts);
  let mut tree = Tree::new();
  tree.resize(width, 960);
  tree.mount_root::<Messages>(app, Props { texts: texts.clone() });
  tree.pass_headless(app);
  (tree, texts)
}

fn bounds(tree: &mut Tree, id: &str) -> ElementRect {
  tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

/// Where the text that reads `text` is laid out.
fn text_bounds(tree: &mut Tree, text: &str) -> ElementRect {
  tree
    .find_element(|element| element.text_content() == Some(text))
    .unwrap_or_else(|| panic!("a text reads {text:?}"))
    .bounds()
}

#[test]
fn markdown_message_keeps_long_code_and_wide_tables_inside_its_width() {
  let mut app = app();
  let header = (1..=8).map(|column| format!("Column {column}")).collect::<Vec<_>>();
  let table = format!(
    "| {} |\n|{}\n| {} |",
    header.join(" | "),
    "---|".repeat(8),
    (1..=8).map(|cell| format!("v{cell}")).collect::<Vec<_>>().join(" | ")
  );
  let long = format!("let line = \"{}\";", "long ".repeat(80));
  let text = format!("Intro with **bold** and `code`.\n\n{table}\n\n```rust\n{long}\n```\n");
  let (mut tree, _texts) = mount(&mut app, 400, vec![text.into()]);

  let message = bounds(&mut tree, "message-0");
  assert!(message.width <= 400.0, "{message:?}");
  assert!(
    tree
      .find_element(|element| element.text_content().is_some_and(|text| text.contains("**")))
      .is_none(),
    "no raw markup remains"
  );
  let last = text_bounds(&mut tree, "Column 8");
  assert!(
    last.x + last.width <= message.x + message.width + 0.5,
    "the table's last column stays inside the message: {last:?} in {message:?}"
  );

  let (code, scroll) = (
    bounds(&mut tree, "message-0-code-1-text"),
    bounds(&mut tree, "message-0-code-1-scroll"),
  );
  assert_eq!(
    tree
      .get_element_by_id("message-0-code-1-text")
      .and_then(|text| text.text_content()),
    Some(long.as_str())
  );
  assert!(scroll.x + scroll.width <= message.x + message.width + 0.5, "{scroll:?}");
  assert!(code.width > scroll.width, "the long line runs past the block");
  let (x, y) = (scroll.x + scroll.width / 2.0, scroll.y + scroll.height / 2.0);
  tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  tree.scroll(x, y, -100.0, 0.0, ScrollPhase::Scroll);
  tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  tree.pass_headless(&mut app);
  assert_eq!(
    bounds(&mut tree, "message-0-code-1-text").x,
    code.x - 100.0,
    "the line scrolls sideways"
  );
  assert_eq!(bounds(&mut tree, "message-0"), message, "the message does not move");
}

/// A message an agent might write: a heading, a list, inline code, a table and a code block.
fn sample(index: usize) -> Arc<str> {
  format!(
    "## Step {index}\n\nI changed **`src/lib.rs`** and ran the checks:\n\n- `cargo check` passed\n- {index} tests \
     passed\n\n| File | Lines |\n|---|---|\n| lib.rs | {index} |\n\n```sh\ncargo test -p core {index}\n```\n"
  )
  .into()
}

#[test]
fn markdown_messages_by_the_hundred_lay_out_and_a_growing_one_moves_nothing_above_it() {
  let mut app = app();
  let count = 300;
  let started = Instant::now();
  let (mut tree, texts) = mount(&mut app, 960, (0..count).map(sample).collect());
  let first = started.elapsed();
  let before: Vec<ElementRect> = (0..count - 1)
    .map(|index| bounds(&mut tree, &format!("message-{index}")))
    .collect();
  let growing = bounds(&mut tree, &format!("message-{}", count - 1));
  let nodes = node_ids(&mut tree, count - 1);

  // The last message grows as its agent streams, into a code block not closed yet.
  let mut times = Vec::new();
  for chunk in ["More text.\n\n```\nline", " one\nline two", "\n```\nDone."] {
    texts.update(|texts| {
      let last = texts.last_mut().expect("messages");
      *last = format!("{last}{chunk}").into();
    });
    let started = Instant::now();
    tree.pass_headless(&mut app);
    times.push(started.elapsed());
    let after: Vec<ElementRect> = (0..count - 1)
      .map(|index| bounds(&mut tree, &format!("message-{index}")))
      .collect();
    assert_eq!(after, before, "no message above the growing one moves");
    let grown = bounds(&mut tree, &format!("message-{}", count - 1));
    assert_eq!(
      (grown.x, grown.y),
      (growing.x, growing.y),
      "the growing message keeps its place"
    );
    assert!(grown.height >= growing.height);
    assert_eq!(
      node_ids(&mut tree, count - 1),
      nodes,
      "the other messages are not built again"
    );
  }
  assert!(
    tree
      .get_element_by_id(&format!("message-{}-code-3", count - 1))
      .is_some(),
    "the block it streamed"
  );
  // Only the message that changed is built again: a pass costs a fraction of the first.
  for time in &times {
    assert!(
      *time * 2 < first,
      "a pass after growth took {time:?}, the first {first:?}"
    );
  }
}

/// The nodes of the first `count` messages, which a message keeps while it is not built again.
fn node_ids(tree: &mut Tree, count: usize) -> Vec<lurq::core::NodeId> {
  (0..count)
    .map(|index| {
      tree
        .get_element_by_id(&format!("message-{index}"))
        .expect("the message")
        .node_id()
    })
    .collect()
}
