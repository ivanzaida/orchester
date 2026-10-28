use orchester_core::docs::blocks::{BlockKind, Body, InlineText, ListItem, ListMarker, Mark, MarkKind};

/// The kinds of the blocks `markdown` parses to.
pub fn kinds(markdown: &str) -> Vec<BlockKind> {
  Body::parse(markdown)
    .blocks()
    .iter()
    .map(|block| block.kind().clone())
    .collect()
}

/// `markdown` written back after a parse, checked to parse to the same blocks and to be written the same again.
pub fn normalized(markdown: &str) -> String {
  let body = Body::parse(markdown);
  let written = body.to_markdown();
  assert_eq!(Body::parse(&written), body, "written as:\n{written}");
  assert_eq!(Body::parse(&written).to_markdown(), written, "written again differs");
  written
}

/// Checks that `kinds` built into a body are parsed back from its Markdown, and returns the Markdown.
pub fn round_trip(kinds: Vec<BlockKind>) -> String {
  let body: Body = kinds.into_iter().collect();
  let written = body.to_markdown();
  assert_eq!(Body::parse(&written), body, "written as:\n{written}");
  written
}

pub fn plain(text: &str) -> InlineText {
  InlineText::plain(text)
}

pub fn marked(text: &str, marks: &[(usize, usize, MarkKind)]) -> InlineText {
  let marks = marks
    .iter()
    .map(|(start, end, kind)| Mark {
      start: *start,
      end: *end,
      kind: kind.clone(),
    })
    .collect();
  InlineText::new(text, marks).expect("marks inside the text")
}

pub fn link(target: &str) -> MarkKind {
  MarkKind::Link(target.to_owned())
}

pub fn paragraph(text: &str) -> BlockKind {
  BlockKind::Paragraph(plain(text))
}

pub fn item(marker: ListMarker, depth: usize, text: &str) -> BlockKind {
  BlockKind::ListItem(ListItem {
    marker,
    task: None,
    depth,
    text: plain(text),
  })
}

pub fn bullet(depth: usize, text: &str) -> BlockKind {
  item(ListMarker::Bullet, depth, text)
}

pub fn ordered(number: u64, depth: usize, text: &str) -> BlockKind {
  item(ListMarker::Ordered(number), depth, text)
}
