//! Blocks to Markdown, in the normalized form the module documentation describes.

use super::{
  Block, BlockKind, Checkbox, ColumnAlign, InlineText, ListItem, ListMarker, Table,
  write_inline::{self, Flow},
};

pub(super) fn markdown(blocks: &[Block]) -> String {
  let mut writer = Writer::default();
  for block in blocks {
    writer.block(block.kind());
  }
  if !writer.out.is_empty() {
    writer.out.push('\n');
  }
  writer.out
}

/// A list open at one level while list items are written.
struct Level {
  /// The column its items' text starts at, where the items of a list inside them are indented to.
  content: usize,
  marker: ListMarker,
  /// `.` or `)`: a new ordered list right after another ordered list at its level takes the other one.
  delimiter: char,
}

#[derive(Default)]
struct Writer {
  out: String,
  /// The lists the last block is in, by level; empty when it is no list item.
  levels: Vec<Level>,
  /// Whether the last block is a list item without text: what follows it must not leave a blank line.
  empty_item: bool,
}

impl Writer {
  fn block(&mut self, block: &BlockKind) {
    if let BlockKind::ListItem(item) = block {
      self.item(item);
      return;
    }
    self.levels.clear();
    self.separate("\n\n");
    match block {
      BlockKind::Heading { level, text } => {
        self.out.push_str(&"#".repeat(usize::from((*level).clamp(1, 6))));
        if !text.is_empty() {
          self.out.push(' ');
          write_inline::inline(&mut self.out, text, Flow::Heading);
        }
      }
      BlockKind::Paragraph(text) => write_inline::inline(&mut self.out, text, Flow::Lines { prefix: "" }),
      BlockKind::Quote(text) => {
        self.out.push('>');
        if !text.is_empty() {
          self.out.push(' ');
          write_inline::inline(&mut self.out, text, Flow::Lines { prefix: "> " });
        }
      }
      BlockKind::Code { language, text } => self.code(language, text),
      BlockKind::Table(table) => self.table(table),
      BlockKind::Image { alt, target } => {
        self.out.push_str("![");
        write_inline::plain(&mut self.out, alt, Flow::Lines { prefix: "" });
        self.out.push_str("](");
        write_inline::destination(&mut self.out, target);
        self.out.push(')');
      }
      BlockKind::Divider => self.out.push_str("---"),
      BlockKind::ListItem(_) => {}
    }
  }

  fn separate(&mut self, separator: &str) {
    if !self.out.is_empty() {
      self.out.push_str(separator);
    }
  }

  fn item(&mut self, item: &ListItem) {
    let depth = item.depth.min(self.levels.len());
    let parent_has_text = !self.levels.is_empty() && !self.empty_item;
    self.levels.truncate(depth + 1);
    let sibling = self.levels.get(depth);
    let continues = sibling.is_some_and(|level| continues(level.marker, item.marker));
    let delimiter = match sibling {
      Some(level) if continues => level.delimiter,
      Some(level) if matches!(level.marker, ListMarker::Ordered(_)) && level.delimiter == '.' => ')',
      _ => '.',
    };
    // A list item starts a new list after a blank line, unless it is inside an item without text, which a blank
    // line would end. One that cannot break a paragraph needs it after its parent's text too.
    let blank = match sibling {
      _ if self.levels.is_empty() => true,
      Some(_) => !continues,
      None => parent_has_text && !breaks_paragraph(item),
    };
    self.separate(if blank { "\n\n" } else { "\n" });
    let indent = depth.checked_sub(1).map_or(0, |parent| self.levels[parent].content);
    let marker = match item.marker {
      ListMarker::Bullet => "-".to_owned(),
      ListMarker::Ordered(number) => format!("{number}{delimiter}"),
    };
    let content = indent + marker.len() + 1;
    self.out.push_str(&" ".repeat(indent));
    self.out.push_str(&marker);
    match item.task {
      Some(Checkbox::Unchecked) => self.out.push_str(" [ ]"),
      Some(Checkbox::Checked) => self.out.push_str(" [x]"),
      None => {}
    }
    if !item.text.is_empty() {
      self.out.push(' ');
      let prefix = " ".repeat(content);
      write_inline::inline(&mut self.out, &item.text, Flow::Lines { prefix: &prefix });
    }
    self.empty_item = item.text.is_empty() && item.task.is_none();
    let level = Level {
      content,
      marker: item.marker,
      delimiter,
    };
    match self.levels.get_mut(depth) {
      Some(open) => *open = level,
      None => self.levels.push(level),
    }
  }

  fn code(&mut self, language: &str, text: &str) {
    let tick = if language.contains('`') { '~' } else { '`' };
    let longest = text.split(|ch| ch != tick).map(str::len).max().unwrap_or(0);
    let fence = tick.to_string().repeat(longest.max(2) + 1);
    self.out.push_str(&fence);
    self.out.push_str(language);
    self.out.push('\n');
    if !text.is_empty() {
      self.out.push_str(text);
      self.out.push('\n');
    }
    self.out.push_str(&fence);
  }

  fn table(&mut self, table: &Table) {
    let width = table.header.len();
    self.row(&table.header, width);
    self.out.push_str("\n|");
    for column in 0..width {
      self
        .out
        .push_str(match table.columns.get(column).copied().unwrap_or(ColumnAlign::Auto) {
          ColumnAlign::Auto => " --- |",
          ColumnAlign::Left => " :-- |",
          ColumnAlign::Center => " :-: |",
          ColumnAlign::Right => " --: |",
        });
    }
    for row in &table.rows {
      self.out.push('\n');
      self.row(row, width);
    }
  }

  /// A table row, with empty cells up to `width`.
  fn row(&mut self, cells: &[InlineText], width: usize) {
    let empty = InlineText::default();
    self.out.push('|');
    for column in 0..width.max(cells.len()) {
      self.out.push(' ');
      write_inline::inline(&mut self.out, cells.get(column).unwrap_or(&empty), Flow::Cell);
      self.out.push_str(" |");
    }
  }
}

/// Whether an item with `next` continues a list whose last item has `last`.
fn continues(last: ListMarker, next: ListMarker) -> bool {
  match (last, next) {
    (ListMarker::Bullet, ListMarker::Bullet) => true,
    (ListMarker::Ordered(last), ListMarker::Ordered(next)) => last.checked_add(1).is_some_and(|after| after == next),
    _ => false,
  }
}

/// Whether `item` can start a list right after a paragraph's line: not empty, and an ordered one only from 1.
fn breaks_paragraph(item: &ListItem) -> bool {
  let empty = item.text.is_empty() && item.task.is_none();
  !empty && matches!(item.marker, ListMarker::Bullet | ListMarker::Ordered(1))
}
