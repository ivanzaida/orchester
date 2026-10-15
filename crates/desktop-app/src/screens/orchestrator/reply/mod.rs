//! An Orchestrator's reply (`Orchestrator · reply with links` `w262B`, `Orchestrator · links · states` `agv8r`): its
//! Markdown as lurq draws it, except that a paragraph, a heading or a list that holds a link is drawn here
//! (`inline`), so each link looks and acts as its kind (`link`): lurq's Markdown draws every link alike, with no
//! underline, hover, tooltip or focus of its own. A reply without links is lurq's Markdown as a whole; otherwise each
//! top-level block is, unless it is one drawn here. A block that holds a link and is anything else (a table, a
//! quote, a list that holds code) keeps lurq's drawing, its links made safe and shown with their hosts by
//! `ui::chat::links::present`.

mod inline;
mod link;
mod place;

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{FormTextRole, PaletteColor, TypographyStyle},
  },
  components::Column,
  markdown::{MarkdownBlock, MarkdownInline, parse_markdown},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;
use pulldown_cmark::{Event, Parser};

use self::inline::{Drawn, Env};
use super::data::ProjectNames;
use crate::ui::{
  chat::links::{Shown, options, present},
  markdown::{LinkHandler, MarkdownView},
};

/// The reply `text`, its links followed by `links`; `project` is the conversation's, `names` the projects' names.
/// `id` names it.
pub(super) fn reply(
  ctx: &mut Ctx,
  id: &str,
  text: &Arc<str>,
  (project, names): (ProjectId, &ProjectNames),
  links: &LinkHandler,
) -> Element {
  ctx.mount_keyed::<Reply>(
    &format!("reply-{id}"),
    ReplyProps {
      id: Arc::from(id),
      text: text.clone(),
      project,
      names: names.clone(),
      links: links.clone(),
    },
  )
}

#[derive(Clone)]
struct ReplyProps {
  id: Arc<str>,
  text: Arc<str>,
  project: ProjectId,
  names: ProjectNames,
  links: LinkHandler,
}

/// Compared by what it shows; the handler by identity, as the screen keeps one.
impl PartialEq for ReplyProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.text == other.text
      && self.project == other.project
      && self.names == other.names
      && Arc::ptr_eq(&self.links, &other.links)
  }
}

impl DevtoolsInspectable for ReplyProps {}

struct Reply;

impl Component for Reply {
  type Props = ReplyProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReplyProps>().clone();
    let blocks = blocks(&props.text);
    if !blocks.iter().any(Block::holds_link) {
      return lurq_markdown(ctx, &props.id, &props.text, &props.links);
    }
    let (theme, base) = {
      let theme = ctx.theme();
      let base = FormTextRole {
        typography: TypographyStyle::Body,
        color: PaletteColor::TextPrimary,
      }
      .resolve(&theme.typography(), &theme.palette());
      (theme.markdown().clone(), base)
    };
    let mut env = Env::new(&props.id, (props.project, &props.names), &props.links, &theme);
    let mut column = Column::new()
      .id(props.id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(theme.document_spacing);
    for (index, block) in blocks.iter().enumerate() {
      column = column.child(match block.drawn() {
        Some(drawn) => inline::draw(ctx, &mut env, &drawn, &base),
        None => lurq_markdown(ctx, &format!("{}-{index}", props.id), &block.source, &props.links),
      });
    }
    column.into()
  }
}

/// Markdown `source` drawn by lurq, named `id`, always through `present`: a link lurq draws is one the chat follows,
/// a link to the web shows its host, and any other is text. However a reply splits into blocks, none of it reaches
/// lurq as written.
fn lurq_markdown(ctx: &mut Ctx, id: &str, source: &str, links: &LinkHandler) -> Element {
  let shown = present(source, &|shown| link_words(ctx, shown)).into_owned();
  MarkdownView::new(id, shown, links.clone()).render(ctx)
}

/// How a link to the web is shown with its host, and a link the chat cannot follow as text, in a block lurq draws.
fn link_words(ctx: &Ctx, shown: Shown<'_>) -> String {
  match shown {
    Shown::Web { link, host } => ctx.t_args("orchestrator.link.web", [("link", link), ("host", host)]),
    Shown::Address { text, address } => ctx.t_args("orchestrator.link.address", [("text", text), ("address", address)]),
  }
  .to_string()
}

/// A top-level block of a reply: its source with the reply's link definitions after it, so a reference link in it
/// still finds its destination, and the blocks lurq's Markdown reads that source as: usually one, but lurq reads some
/// of what pulldown-cmark keeps in one paragraph as more (display math before text).
struct Block {
  source: String,
  parsed: Vec<MarkdownBlock>,
}

impl Block {
  fn holds_link(&self) -> bool {
    self.parsed.iter().any(holds_link_block)
  }

  /// The block as drawn here: only when lurq reads it as one block that holds a link and is drawn here.
  fn drawn(&self) -> Option<Drawn<'_>> {
    match self.parsed.as_slice() {
      [block] if holds_link_block(block) => Drawn::of(block),
      _ => None,
    }
  }
}

/// The top-level blocks of `source`, in order.
fn blocks(source: &str) -> Vec<Block> {
  let mut parser = Parser::new_ext(source, options()).into_offset_iter();
  let mut ranges: Vec<Range<usize>> = Vec::new();
  let mut depth = 0_usize;
  for (event, range) in parser.by_ref() {
    match event {
      Event::Start(_) => {
        if depth == 0 {
          ranges.push(range);
        }
        depth += 1;
      }
      Event::End(_) => depth = depth.saturating_sub(1),
      _ if depth == 0 => ranges.push(range),
      _ => {}
    }
  }
  // Written again from what was read, not copied from the source: a definition inside a quote spans its `>` marks.
  let definitions: String = parser
    .reference_definitions()
    .iter()
    .map(|(label, definition)| {
      format!(
        "\n[{}]: <{}>",
        escaped_in(label, &['[', ']', '\\']),
        escaped_in(&definition.dest, &['<', '>', '\\'])
      )
    })
    .collect();
  ranges
    .into_iter()
    .map(|range| {
      let source = format!("{}\n{definitions}", &source[range]);
      let parsed = parse_markdown(&source).blocks;
      Block { source, parsed }
    })
    .collect()
}

/// `text` with each of `marks` escaped by a backslash.
fn escaped_in(text: &str, marks: &[char]) -> String {
  let mut out = String::with_capacity(text.len());
  for glyph in text.chars() {
    if marks.contains(&glyph) {
      out.push('\\');
    }
    out.push(glyph);
  }
  out
}

/// Whether `block` holds a link anywhere.
fn holds_link_block(block: &MarkdownBlock) -> bool {
  match block {
    MarkdownBlock::Paragraph(inlines) | MarkdownBlock::Heading { children: inlines, .. } => holds_link(inlines),
    MarkdownBlock::BlockQuote(blocks) | MarkdownBlock::FootnoteDefinition { blocks, .. } => {
      blocks.iter().any(holds_link_block)
    }
    MarkdownBlock::List { items, .. } => items.iter().flat_map(|item| &item.blocks).any(holds_link_block),
    MarkdownBlock::Table { rows, .. } => rows.iter().flat_map(|row| &row.cells).any(|cell| holds_link(cell)),
    MarkdownBlock::CodeBlock { .. }
    | MarkdownBlock::Math { .. }
    | MarkdownBlock::Html(_)
    | MarkdownBlock::ThematicBreak => false,
  }
}

/// Whether `inlines` hold a link anywhere.
fn holds_link(inlines: &[MarkdownInline]) -> bool {
  inlines.iter().any(|inline| match inline {
    MarkdownInline::Link { .. } => true,
    MarkdownInline::Emphasis(children)
    | MarkdownInline::Strong(children)
    | MarkdownInline::Strikethrough(children)
    | MarkdownInline::Image { alt: children, .. } => holds_link(children),
    _ => false,
  })
}
