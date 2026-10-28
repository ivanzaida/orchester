//! Markdown to blocks: pulldown-cmark's events folded into the flat list of blocks the module documentation
//! describes.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::{BlockKind, Checkbox, ColumnAlign, InlineText, ListItem, ListMarker, Mark, MarkKind, Table};

/// The Markdown a doc is written in: CommonMark with GitHub tables, task lists and strikethrough.
fn options() -> Options {
  Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

pub(super) fn blocks(markdown: &str) -> Vec<BlockKind> {
  let mut reader = Reader::default();
  for event in Parser::new_ext(markdown, options()) {
    reader.event(event);
  }
  let mut blocks = reader.blocks;
  flatten_depths(&mut blocks);
  blocks
}

/// Keeps every list item at most one level deeper than the list item before it, and at level 0 after any other
/// block, as the written form can say it.
fn flatten_depths(blocks: &mut [BlockKind]) {
  let mut deepest = 0;
  for block in blocks {
    match block {
      BlockKind::ListItem(item) => {
        item.depth = item.depth.min(deepest);
        deepest = item.depth + 1;
      }
      _ => deepest = 0,
    }
  }
}

/// Where the inline text being read goes when it ends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leaf {
  Heading(u8),
  Paragraph,
  Quote,
  /// The text of the innermost open list item.
  Item,
  Cell,
}

struct OpenItem {
  marker: ListMarker,
  depth: usize,
  task: Option<Checkbox>,
  /// Whether its block is out: it is, once its text ended or another block started inside it. An item inside a
  /// quote never writes one; its text is the quote's.
  written: bool,
}

struct OpenTable {
  columns: Vec<ColumnAlign>,
  header: Vec<InlineText>,
  rows: Vec<Vec<InlineText>>,
  row: Vec<InlineText>,
}

#[derive(Default)]
struct Reader {
  blocks: Vec<BlockKind>,
  /// The open lists, each with the number of its next item when it is ordered.
  lists: Vec<Option<u64>>,
  items: Vec<OpenItem>,
  /// For each open quote, whether a block was written inside it.
  quotes: Vec<bool>,
  text: Option<(Leaf, Inline)>,
  code: Option<(String, String)>,
  table: Option<OpenTable>,
}

impl Reader {
  fn event(&mut self, event: Event<'_>) {
    match event {
      Event::Start(tag) => self.start(tag),
      Event::End(tag) => self.end(tag),
      Event::Text(text) if self.code.is_some() => {
        if let Some((_, code)) = &mut self.code {
          code.push_str(&text);
        }
      }
      Event::Html(html) => self.inline().push_raw(&html),
      Event::TaskListMarker(checked) => {
        if let Some(item) = self.items.last_mut() {
          item.task = Some(if checked {
            Checkbox::Checked
          } else {
            Checkbox::Unchecked
          });
        }
      }
      Event::Rule => {
        self.block_starts();
        self.push(BlockKind::Divider);
      }
      event => {
        let leaf = self.leaf();
        self.inline().event(event, leaf);
      }
    }
  }

  fn start(&mut self, tag: Tag<'_>) {
    match tag {
      Tag::Paragraph | Tag::HtmlBlock => {
        self.close();
        self.open(self.text_leaf());
      }
      Tag::Heading { level, .. } => {
        let leaf = if self.quotes.is_empty() {
          self.block_starts();
          // `HeadingLevel` counts from 1 (`H1 = 1`).
          Leaf::Heading(level as u8)
        } else {
          Leaf::Quote
        };
        self.open(leaf);
      }
      Tag::BlockQuote(_) => {
        self.block_starts();
        self.quotes.push(false);
      }
      Tag::CodeBlock(kind) => {
        self.block_starts();
        let language = match kind {
          CodeBlockKind::Fenced(info) => info.trim().to_owned(),
          CodeBlockKind::Indented => String::new(),
        };
        self.code = Some((language, String::new()));
      }
      Tag::List(start) => {
        self.block_starts();
        self.lists.push(start);
      }
      Tag::Item => self.start_item(),
      Tag::Table(columns) => {
        self.block_starts();
        self.table = Some(OpenTable {
          columns: columns.into_iter().map(column_align).collect(),
          header: Vec::new(),
          rows: Vec::new(),
          row: Vec::new(),
        });
      }
      Tag::TableCell => self.open(Leaf::Cell),
      Tag::TableHead | Tag::TableRow => {}
      tag => {
        let leaf = self.leaf();
        self.inline().event(Event::Start(tag), leaf);
      }
    }
  }

  fn end(&mut self, tag: TagEnd) {
    match tag {
      TagEnd::Paragraph | TagEnd::HtmlBlock | TagEnd::Heading(_) | TagEnd::TableCell => self.close(),
      TagEnd::BlockQuote(_) => {
        self.close();
        let wrote = self.quotes.pop().unwrap_or(true);
        if !wrote {
          self.push(BlockKind::Quote(InlineText::default()));
        } else if let Some(outer) = self.quotes.last_mut() {
          *outer = true;
        }
      }
      TagEnd::CodeBlock => {
        if let Some((language, mut text)) = self.code.take() {
          if text.ends_with('\n') {
            text.pop();
          }
          self.push(BlockKind::Code { language, text });
        }
      }
      TagEnd::List(_) => {
        self.lists.pop();
      }
      TagEnd::Item => {
        self.close();
        if self.items.last().is_some_and(|item| !item.written) {
          self.write_item(InlineText::default());
        }
        self.items.pop();
      }
      TagEnd::TableHead => {
        if let Some(table) = &mut self.table {
          table.header = std::mem::take(&mut table.row);
        }
      }
      TagEnd::TableRow => {
        if let Some(table) = &mut self.table {
          let row = std::mem::take(&mut table.row);
          table.rows.push(row);
        }
      }
      TagEnd::Table => {
        if let Some(table) = self.table.take() {
          self.push(BlockKind::Table(Table {
            columns: table.columns,
            header: table.header,
            rows: table.rows,
          }));
        }
      }
      tag => {
        let leaf = self.leaf();
        self.inline().event(Event::End(tag), leaf);
      }
    }
  }

  fn start_item(&mut self) {
    self.block_starts();
    let marker = match self.lists.last_mut() {
      Some(Some(next)) => {
        let number = *next;
        *next += 1;
        ListMarker::Ordered(number)
      }
      Some(None) | None => ListMarker::Bullet,
    };
    self.items.push(OpenItem {
      marker,
      depth: self.lists.len().saturating_sub(1),
      task: None,
      written: !self.quotes.is_empty(),
    });
  }

  /// Ends the inline text a block starting now follows (a tight item's text), and writes the open item before the
  /// block when it had no text.
  fn block_starts(&mut self) {
    self.close();
    if self.items.last().is_some_and(|item| !item.written) {
      self.write_item(InlineText::default());
    }
  }

  /// Where a paragraph or loose text starting now goes.
  fn text_leaf(&self) -> Leaf {
    if !self.quotes.is_empty() {
      Leaf::Quote
    } else if self.items.last().is_some_and(|item| !item.written) {
      Leaf::Item
    } else {
      Leaf::Paragraph
    }
  }

  fn leaf(&self) -> Leaf {
    self.text.as_ref().map_or_else(|| self.text_leaf(), |(leaf, _)| *leaf)
  }

  fn open(&mut self, leaf: Leaf) {
    self.close();
    self.text = Some((leaf, Inline::default()));
  }

  /// The inline text being read, opened where text starting now goes when none is.
  fn inline(&mut self) -> &mut Inline {
    let leaf = self.text_leaf();
    &mut self.text.get_or_insert_with(|| (leaf, Inline::default())).1
  }

  fn close(&mut self) {
    let Some((leaf, inline)) = self.text.take() else {
      return;
    };
    let (text, images) = inline.finish();
    match leaf {
      Leaf::Heading(level) => self.push(BlockKind::Heading { level, text }),
      Leaf::Paragraph | Leaf::Quote if !images.is_empty() => {
        for (alt, target) in images {
          self.push(BlockKind::Image { alt, target });
        }
      }
      Leaf::Paragraph if text.is_empty() => {}
      Leaf::Paragraph => self.push(BlockKind::Paragraph(text)),
      Leaf::Quote => self.push(BlockKind::Quote(text)),
      Leaf::Item => self.write_item(text),
      Leaf::Cell => {
        if let Some(table) = &mut self.table {
          table.row.push(text);
        }
      }
    }
  }

  fn write_item(&mut self, text: InlineText) {
    let Some(item) = self.items.last_mut() else {
      return;
    };
    item.written = true;
    let block = BlockKind::ListItem(ListItem {
      marker: item.marker,
      task: item.task,
      depth: item.depth,
      text,
    });
    self.push(block);
  }

  fn push(&mut self, block: BlockKind) {
    self.blocks.push(block);
    if let Some(quote) = self.quotes.last_mut() {
      *quote = true;
    }
  }
}

const fn column_align(align: Alignment) -> ColumnAlign {
  match align {
    Alignment::None => ColumnAlign::Auto,
    Alignment::Left => ColumnAlign::Left,
    Alignment::Center => ColumnAlign::Center,
    Alignment::Right => ColumnAlign::Right,
  }
}

/// The inline text of one leaf as it is read.
#[derive(Default)]
struct Inline {
  text: String,
  marks: Vec<Mark>,
  /// The marks open now, by where they started.
  open: Vec<(usize, MarkKind)>,
  /// How deep inside images the text is: an image's alt text is read as plain text.
  in_image: usize,
  /// The alt text start and target of the outermost image open now.
  image: Option<(usize, String)>,
  /// The images read, as alt text and target.
  images: Vec<(String, String)>,
  /// Whether anything but images and spaces was read.
  other: bool,
}

impl Inline {
  fn event(&mut self, event: Event<'_>, leaf: Leaf) {
    match event {
      Event::Text(text) => self.push_text(&text),
      Event::Code(code) => {
        let start = self.text.len();
        self.push_text(&code);
        if self.in_image == 0 {
          self.marks.push(Mark {
            start,
            end: self.text.len(),
            kind: MarkKind::Code,
          });
        }
      }
      Event::InlineHtml(html) => self.push_raw(&html),
      Event::SoftBreak => self.text.push(' '),
      Event::HardBreak if matches!(leaf, Leaf::Heading(_) | Leaf::Cell) => self.text.push(' '),
      Event::HardBreak => self.text.push('\n'),
      Event::Start(Tag::Image { dest_url, .. }) => {
        if self.in_image == 0 {
          self.image = Some((self.text.len(), dest_url.into_string()));
        }
        self.in_image += 1;
      }
      Event::End(TagEnd::Image) => self.end_image(),
      Event::Start(_) | Event::End(_) if self.in_image > 0 => {}
      Event::Start(tag) => {
        if let Some(kind) = mark_kind(tag) {
          self.open.push((self.text.len(), kind));
        }
      }
      Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link) => {
        if let Some((start, kind)) = self.open.pop() {
          self.marks.push(Mark {
            start,
            end: self.text.len(),
            kind,
          });
        }
      }
      _ => {}
    }
  }

  fn end_image(&mut self) {
    self.in_image = self.in_image.saturating_sub(1);
    if self.in_image > 0 {
      return;
    }
    if let Some((start, target)) = self.image.take() {
      self.images.push((self.text[start..].to_owned(), target.clone()));
      self.marks.push(Mark {
        start,
        end: self.text.len(),
        kind: MarkKind::Link(target),
      });
    }
  }

  fn push_text(&mut self, text: &str) {
    if self.in_image == 0 && !text.trim().is_empty() {
      self.other = true;
    }
    self.text.push_str(text);
  }

  /// Raw HTML, kept as literal text: its line breaks are hard breaks.
  fn push_raw(&mut self, html: &str) {
    self.other = true;
    self.text.push_str(html);
  }

  /// The text read, and its images when it is nothing but them (else none).
  fn finish(mut self) -> (InlineText, Vec<(String, String)>) {
    // An HTML block's text ends with its line break.
    let kept = self.text.trim_end_matches('\n').len();
    self.text.truncate(kept);
    for mark in &mut self.marks {
      mark.start = mark.start.min(kept);
      mark.end = mark.end.min(kept);
    }
    if self.other {
      self.images.clear();
    }
    (InlineText::canonical(self.text, self.marks), self.images)
  }
}

fn mark_kind(tag: Tag<'_>) -> Option<MarkKind> {
  match tag {
    Tag::Emphasis => Some(MarkKind::Italic),
    Tag::Strong => Some(MarkKind::Bold),
    Tag::Strikethrough => Some(MarkKind::Strikethrough),
    Tag::Link { dest_url, .. } => Some(MarkKind::Link(dest_url.into_string())),
    _ => None,
  }
}
