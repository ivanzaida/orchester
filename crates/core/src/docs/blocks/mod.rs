//! A doc's Markdown body as a list of blocks, the unit the WYSIWYG editor and the block-by-block merge work on
//! (`docs/design/docs.md`, *The editor* and *Conflicts*).
//!
//! The body is CommonMark with GitHub tables, task lists and strikethrough, read with the parser lurq's Markdown
//! renders with. Each block keeps its inline content as text with marks over it ([`InlineText`]).
//!
//! # Round trip
//!
//! `Body::parse(&body.to_markdown()) == body` for every parsed body, and `body.to_markdown()` of a parsed body is a
//! normalized form of the Markdown it was parsed from, which parses to the same blocks. The normalized form:
//!
//! - Blocks are separated by one blank line; list items of one list by none. Lines end with `\n`, and the body ends
//!   with one.
//! - Headings are ATX (`## Title`); setext headings become ATX. A line break in a heading or a table cell is a space.
//! - Paragraphs are one line: soft breaks are spaces, a hard break is written as `\` at the end of the line.
//! - Bullets are `-`, ordered items `1.` (switching to `1)` when a new list must start right after another ordered
//!   list), task items `- [ ]` and `- [x]`; nested items are indented to their parent's text, lists are tight. A list
//!   item holds the text of its first paragraph; further paragraphs, code and quotes inside it become blocks of their
//!   own after it, and a list item is at most one level deeper than the list item right before it (a list item after
//!   any other block is at level 0).
//! - A quote holds one paragraph: a quote of several paragraphs is several quotes, and nested quotes, headings and list
//!   items inside a quote are quotes of their text. Code, tables, images and dividers inside a quote are blocks of
//!   their own.
//! - Code blocks are fenced with backticks (tildes when the language holds a backtick), indented code becomes fenced.
//! - Emphasis is `*italic*` and `**bold**`, strikethrough `~~text~~`; links are inline `[text](target)`, reference
//!   links and autolinks included, and link and image titles are dropped. A mark crossing the edge of another is closed
//!   there and reopened, with `_` when it reopens right after a closing `*`.
//! - Tables are written `| a | b |` with one space around each cell and `---` delimiters with their alignment colons;
//!   columns are not padded to one width.
//! - A paragraph that holds only images is one image block per image; an image among text is its alt text linked to the
//!   image's target. Raw HTML is kept as literal text.
//! - Text is escaped where Markdown would read it as syntax: `\`, `` ` ``, `*`, `[`, `]`, `<`, `~` and `|` always, `_`
//!   unless inside a word, `&` before a character reference, `!` before a link, block markers at the start of a line
//!   (`#`, `>`, `-`, `+`, `=`, `:`, `1.`), `#` at the end of a heading, and spaces at the edges of a line (as `&#32;`).
//!
//! Blocks a caller builds round-trip when they hold what a parse could give: no empty paragraph, heading levels 1
//! to 6, consecutive numbers in one ordered list, table rows no wider than the header, emphasis that does not start or
//! end with a space, no mark starting or ending inside a code mark, and links that do not overlap.

mod inline;
mod parse;
mod write;
mod write_inline;

use thiserror::Error;

pub use self::inline::{InlineText, Mark, MarkKind};

/// Identifies a block within one body. A parse numbers the blocks from 0 in order; edits keep the ids of the blocks
/// they leave and give new blocks ids not used before in the body, so focus and merge follow a block across edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(u64);

impl BlockId {
  pub const fn get(self) -> u64 {
    self.0
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
  id: BlockId,
  kind: BlockKind,
}

impl Block {
  pub const fn id(&self) -> BlockId {
    self.id
  }

  pub const fn kind(&self) -> &BlockKind {
    &self.kind
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
  /// A heading of `level` 1 to 6.
  Heading {
    level: u8,
    text: InlineText,
  },
  Paragraph(InlineText),
  ListItem(ListItem),
  Quote(InlineText),
  /// A code block; `language` is its info string, empty when it has none. `text` has no final line break.
  Code {
    language: String,
    text: String,
  },
  Table(Table),
  Image {
    alt: String,
    target: String,
  },
  Divider,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
  pub marker: ListMarker,
  pub task: Option<Checkbox>,
  /// 0 for an item of a top-level list, 1 for an item of a list inside it, and so on.
  pub depth: usize,
  pub text: InlineText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMarker {
  Bullet,
  /// An ordered item with the number it shows.
  Ordered(u64),
}

/// The check box of a task list item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checkbox {
  Unchecked,
  Checked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
  pub columns: Vec<ColumnAlign>,
  pub header: Vec<InlineText>,
  pub rows: Vec<Vec<InlineText>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnAlign {
  /// No alignment given (`---`).
  Auto,
  Left,
  Center,
  Right,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BlocksError {
  #[error("no block {0:?} in this body")]
  UnknownBlock(BlockId),
  #[error("position {index} is past the body's {len} blocks")]
  OutOfRange { index: usize, len: usize },
  #[error("mark {start}..{end} is not on characters of the {len} bytes of its text")]
  MarkOutsideText { start: usize, end: usize, len: usize },
}

/// A doc's body as blocks with ids stable within it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Body {
  blocks: Vec<Block>,
  next_id: u64,
}

impl Body {
  /// The blocks of the Markdown `markdown`. Every input parses; what the blocks cannot hold is normalized as the
  /// module documentation says.
  pub fn parse(markdown: &str) -> Self {
    parse::blocks(markdown).into_iter().collect()
  }

  /// The body as normalized Markdown.
  pub fn to_markdown(&self) -> String {
    write::markdown(&self.blocks)
  }

  pub fn blocks(&self) -> &[Block] {
    &self.blocks
  }

  pub fn block(&self, id: BlockId) -> Option<&Block> {
    self.blocks.iter().find(|block| block.id == id)
  }

  /// Puts a new block at `index`, before the block there, and returns its id.
  pub fn insert(&mut self, index: usize, kind: BlockKind) -> Result<BlockId, BlocksError> {
    if index > self.blocks.len() {
      return Err(BlocksError::OutOfRange {
        index,
        len: self.blocks.len(),
      });
    }
    let id = self.new_id();
    self.blocks.insert(index, Block { id, kind });
    Ok(id)
  }

  /// Gives the block `id` new content; it keeps its id and place.
  pub fn replace(&mut self, id: BlockId, kind: BlockKind) -> Result<BlockKind, BlocksError> {
    let block = self.position(id).map(|index| &mut self.blocks[index])?;
    Ok(std::mem::replace(&mut block.kind, kind))
  }

  pub fn remove(&mut self, id: BlockId) -> Result<BlockKind, BlocksError> {
    let index = self.position(id)?;
    Ok(self.blocks.remove(index).kind)
  }

  fn position(&self, id: BlockId) -> Result<usize, BlocksError> {
    self
      .blocks
      .iter()
      .position(|block| block.id == id)
      .ok_or(BlocksError::UnknownBlock(id))
  }

  fn new_id(&mut self) -> BlockId {
    let id = BlockId(self.next_id);
    self.next_id += 1;
    id
  }
}

/// A body of `kinds` in order, numbered as a parse numbers them.
impl FromIterator<BlockKind> for Body {
  fn from_iter<I: IntoIterator<Item = BlockKind>>(kinds: I) -> Self {
    let mut body = Self::default();
    for kind in kinds {
      let id = body.new_id();
      body.blocks.push(Block { id, kind });
    }
    body
  }
}
