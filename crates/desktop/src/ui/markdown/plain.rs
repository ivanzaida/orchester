//! What Markdown reads as once rendered, as plain text: for a search, which should find what the chat
//! shows, and for a line that shows a message on one line.

use lurq::markdown::{MarkdownBlock, MarkdownInline, parse_markdown};

/// The characters that make Markdown read differently from its source; text without any of them reads
/// as written.
const MARKUP: &[char] = &['*', '_', '`', '~', '[', '#', '<', '&', '\\', '|', '>'];

/// The text `source` shows once rendered: each block and each line of a paragraph on a line of its
/// own, without the markup.
pub fn plain_text(source: &str) -> String {
  let mut text = String::new();
  for block in &parse_markdown(source).blocks {
    push_block(&mut text, block);
  }
  text.truncate(text.trim_end().len());
  text
}

/// Whether `source` may read differently once rendered; when not, [`plain_text`] would give it back.
pub fn has_markup(source: &str) -> bool {
  source.contains(MARKUP)
}

/// What `source` reads as once rendered, when that differs from it: what a search of the chat finds
/// besides the source.
pub fn rendered(source: &str) -> Option<String> {
  has_markup(source).then(|| plain_text(source))
}

fn push_block(text: &mut String, block: &MarkdownBlock) {
  match block {
    MarkdownBlock::Paragraph(inlines) | MarkdownBlock::Heading { children: inlines, .. } => {
      push_inlines(text, inlines);
    }
    MarkdownBlock::BlockQuote(blocks) | MarkdownBlock::FootnoteDefinition { blocks, .. } => {
      blocks.iter().for_each(|block| push_block(text, block));
      return;
    }
    MarkdownBlock::List { items, .. } => {
      items
        .iter()
        .flat_map(|item| &item.blocks)
        .for_each(|block| push_block(text, block));
      return;
    }
    MarkdownBlock::Table { rows, .. } => {
      for row in rows {
        for cell in &row.cells {
          push_inlines(text, cell);
          text.push(' ');
        }
        text.push('\n');
      }
      return;
    }
    MarkdownBlock::CodeBlock { text: code, .. } | MarkdownBlock::Math { text: code } => text.push_str(code.trim_end()),
    MarkdownBlock::Html(html) => text.push_str(html.trim_end()),
    MarkdownBlock::ThematicBreak => return,
  }
  text.push('\n');
}

fn push_inlines(text: &mut String, inlines: &[MarkdownInline]) {
  for inline in inlines {
    match inline {
      MarkdownInline::Text(value)
      | MarkdownInline::Code(value)
      | MarkdownInline::Math(value)
      | MarkdownInline::Html(value)
      | MarkdownInline::FootnoteReference(value) => text.push_str(value),
      MarkdownInline::Emphasis(children)
      | MarkdownInline::Strong(children)
      | MarkdownInline::Strikethrough(children)
      | MarkdownInline::Link { children, .. }
      | MarkdownInline::Image { alt: children, .. } => push_inlines(text, children),
      // The chat keeps the line breaks a message was written with (`super::segments::keep_line_breaks`).
      MarkdownInline::SoftBreak | MarkdownInline::HardBreak => text.push('\n'),
      MarkdownInline::TaskListMarker(_) => {}
    }
  }
}
