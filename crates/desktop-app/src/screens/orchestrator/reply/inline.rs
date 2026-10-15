//! A paragraph, heading or list of a reply that holds a link, drawn as lurq's Markdown lays one out (a wrapping row
//! of text runs per line, list markers at the theme's widths and gaps) with the reply's links drawn by `link`. lurq
//! keeps no public rich text, so each run of one style is a text of its own; a selection stays within one, as it
//! stays within one of lurq's around a link.

use lurq::{
  app::{ctx::Ctx, theme::ThemeMarkdown},
  components::{Column, Row, Text},
  layout::{Alignment, text_style::TextStyle},
  markdown::{MarkdownBlock, MarkdownHeadingLevel, MarkdownInline},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use super::{
  super::data::ProjectNames,
  holds_link,
  link::{self, Target},
};
use crate::{
  theme::{OrchestratorText, markdown_heading, markdown_run},
  ui::{chat::links::without_bidi_controls, markdown::LinkHandler},
};

/// A block drawn here: a paragraph or heading's text, or a list whose items hold only those and lists.
pub(super) enum Drawn<'a> {
  Text {
    inlines: &'a [MarkdownInline],
    heading: Option<MarkdownHeadingLevel>,
  },
  List {
    ordered: bool,
    start: u64,
    items: Vec<Vec<Drawn<'a>>>,
  },
}

impl<'a> Drawn<'a> {
  /// `block` as drawn here, or `None` when lurq's Markdown draws it.
  pub(super) fn of(block: &'a MarkdownBlock) -> Option<Self> {
    match block {
      MarkdownBlock::Paragraph(inlines) => Some(Self::Text { inlines, heading: None }),
      MarkdownBlock::Heading { level, children } => Some(Self::Text {
        inlines: children,
        heading: Some(*level),
      }),
      MarkdownBlock::List { ordered, start, items } => Some(Self::List {
        ordered: *ordered,
        start: start.unwrap_or(1),
        items: items
          .iter()
          .map(|item| item.blocks.iter().map(Self::of).collect::<Option<Vec<_>>>())
          .collect::<Option<Vec<_>>>()?,
      }),
      _ => None,
    }
  }
}

/// What every link of a reply needs: the reply's id, which numbers its links, the conversation's project, the
/// projects' names, what a press does, and the theme's Markdown roles.
pub(super) struct Env<'a> {
  id: &'a str,
  pub(super) project: ProjectId,
  pub(super) names: &'a ProjectNames,
  pub(super) links: &'a LinkHandler,
  theme: &'a ThemeMarkdown,
  next: usize,
}

impl<'a> Env<'a> {
  pub(super) const fn new(
    id: &'a str,
    (project, names): (ProjectId, &'a ProjectNames),
    links: &'a LinkHandler,
    theme: &'a ThemeMarkdown,
  ) -> Self {
    Self {
      id,
      project,
      names,
      links,
      theme,
      next: 0,
    }
  }

  /// The element id of the reply's next link: `<reply id>-link-<n>`, numbered from 0 in reading order.
  fn link_id(&mut self) -> String {
    let id = format!("{}-link-{}", self.id, self.next);
    self.next += 1;
    id
  }
}

/// `drawn` in `base`, the reply's text style.
pub(super) fn draw(ctx: &mut Ctx, env: &mut Env<'_>, drawn: &Drawn<'_>, base: &TextStyle) -> Element {
  match drawn {
    Drawn::Text { inlines, heading } => {
      let style = heading.map_or_else(|| base.clone(), |level| markdown_heading(env.theme, level, base));
      lines(ctx, env, inlines, &style)
    }
    Drawn::List { ordered, start, items } => list(ctx, env, (*ordered, *start, items), base),
  }
}

/// A list's items, each its marker and then its blocks, as lurq's Markdown lays out a list.
fn list(
  ctx: &mut Ctx,
  env: &mut Env<'_>,
  (ordered, start, items): (bool, u64, &[Vec<Drawn<'_>>]),
  base: &TextStyle,
) -> Element {
  let theme = env.theme;
  let marker_style = theme.list_marker.apply(base);
  let marker_width = if ordered {
    theme.ordered_list_marker_width
  } else {
    theme.unordered_list_marker_width
  };
  let mut list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(theme.list_item_spacing);
  for (number, item) in (start..).zip(items) {
    let marker = if ordered { format!("{number}.") } else { "-".to_owned() };
    let mut body = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(theme.nested_block_spacing);
    for block in item {
      body = body.child(draw(ctx, env, block, base));
    }
    list = list.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(theme.list_marker_gap)
        .align_items(Alignment::Start)
        .child(markdown_run(&marker, marker_style.clone()).width(Dimension::Px(marker_width)))
        .child(body),
    );
  }
  list.into()
}

/// `inlines` in `style`: a line of its own after each hard break, each line a row that wraps.
fn lines(ctx: &mut Ctx, env: &mut Env<'_>, inlines: &[MarkdownInline], style: &TextStyle) -> Element {
  let mut column = Column::new().width(Dimension::Pct(100.0)).min_width(Dimension::Px(0.0));
  for line in inlines.split(|inline| matches!(inline, MarkdownInline::HardBreak)) {
    let mut runs = Runs::default();
    runs.walk(ctx, env, line, style);
    column = column.child(
      Row::new()
        .wrap()
        .width(Dimension::Pct(100.0))
        .min_width(Dimension::Px(0.0))
        .align_items(Alignment::Center)
        .with_children(runs.finish()),
    );
  }
  column.into()
}

/// The elements of a line: runs of text, one per style, and links.
#[derive(Default)]
struct Runs {
  elements: Vec<Element>,
  text: String,
  style: Option<TextStyle>,
}

impl Runs {
  fn push(&mut self, text: &str, style: &TextStyle) {
    if self.style.as_ref() != Some(style) {
      self.flush();
      self.style = Some(style.clone());
    }
    self.text.push_str(text);
  }

  fn element(&mut self, element: impl Into<Element>) {
    self.flush();
    self.elements.push(element.into());
  }

  fn flush(&mut self) {
    let text = std::mem::take(&mut self.text);
    if let (false, Some(style)) = (text.is_empty(), self.style.take()) {
      self.elements.push(
        markdown_run(&text, style)
          .selectable(true)
          .max_width(Dimension::Pct(100.0))
          .min_width(Dimension::Px(0.0))
          .into(),
      );
    }
  }

  fn finish(mut self) -> Vec<Element> {
    self.flush();
    self.elements
  }

  fn walk(&mut self, ctx: &mut Ctx, env: &mut Env<'_>, inlines: &[MarkdownInline], style: &TextStyle) {
    let theme = env.theme;
    for inline in inlines {
      match inline {
        MarkdownInline::Text(text) | MarkdownInline::Html(text) | MarkdownInline::FootnoteReference(text) => {
          self.push(text, style);
        }
        MarkdownInline::Code(text) | MarkdownInline::Math(text) => self.push(text, &theme.inline_code.apply(style)),
        MarkdownInline::Emphasis(children) => self.walk(ctx, env, children, &theme.emphasis.apply(style)),
        MarkdownInline::Strong(children) => self.walk(ctx, env, children, &theme.strong.apply(style)),
        MarkdownInline::Strikethrough(children) => self.walk(ctx, env, children, &theme.strikethrough.apply(style)),
        MarkdownInline::Image { alt, .. } => self.walk(ctx, env, alt, style),
        MarkdownInline::SoftBreak | MarkdownInline::HardBreak => self.push(" ", style),
        MarkdownInline::TaskListMarker(done) => self.push(if *done { "[x] " } else { "[ ] " }, style),
        MarkdownInline::Link {
          destination, children, ..
        } => self.link(ctx, env, (destination, children), style),
      }
    }
  }

  /// A link the chat follows, drawn by `link`; any other is its words, then its address.
  fn link(
    &mut self,
    ctx: &mut Ctx,
    env: &mut Env<'_>,
    (destination, children): (&str, &[MarkdownInline]),
    style: &TextStyle,
  ) {
    let label = without_bidi_controls(&plain(children));
    // A link holding another is text: its words would read as the inner one while a press went here.
    let target = if holds_link(children) {
      None
    } else {
      Target::of(destination)
    };
    if let Some(target) = target {
      let id = env.link_id();
      let link_style = env.theme.link.apply(style);
      self.element(link::link(
        ctx,
        env,
        link::Look {
          id,
          label,
          style: link_style,
          destination,
          target,
        },
      ));
      return;
    }
    let shown = without_bidi_controls(destination);
    if label.trim().is_empty() || label == destination {
      self.element(address(&shown));
      return;
    }
    self.push(&label, style);
    self.push(" ", style);
    let note = ctx.t_args("orchestrator.link.addressNote", [("address", shown.as_str())]);
    self.element(address(&note));
  }
}

/// An address the chat does not follow, as text.
fn address(text: &str) -> Text {
  Text::new(text)
    .variant(OrchestratorText::LinkAddress.role())
    .selectable(true)
    .max_width(Dimension::Pct(100.0))
}

/// What `inlines` read as, as plain text.
fn plain(inlines: &[MarkdownInline]) -> String {
  let mut text = String::new();
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
      | MarkdownInline::Image { alt: children, .. } => text.push_str(&plain(children)),
      MarkdownInline::SoftBreak | MarkdownInline::HardBreak => text.push(' '),
      MarkdownInline::TaskListMarker(_) => {}
    }
  }
  text
}
