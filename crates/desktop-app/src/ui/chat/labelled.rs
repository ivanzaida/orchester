//! A reply's labelled links (`Orchestrator · reply with links` `w262B`): a list whose every item is a label ending
//! in a colon, exactly one link, and optionally a dash and a detail (`- Task: [KON-12](…) — Ready for review`) reads
//! as a grid of the label, the link and the detail. Any other list stays a list.

use lurq::markdown::{MarkdownBlock, MarkdownInline, MarkdownListItem};

/// One row of a labelled-links grid.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelledLink<'a> {
  /// The label without its colon.
  pub label: String,
  /// The link, as the list holds it.
  pub link: &'a MarkdownInline,
  /// What follows the dash, without it; empty when the item ends with the link.
  pub detail: Vec<MarkdownInline>,
}

/// The dashes that may part a labelled link from its detail.
const DASHES: [char; 3] = ['—', '–', '-'];

/// `items` as labelled links when every one is a labelled link; `None` when any is not, or there are none.
pub fn labelled_links(items: &[MarkdownListItem]) -> Option<Vec<LabelledLink<'_>>> {
  if items.is_empty() {
    return None;
  }
  items.iter().map(labelled).collect()
}

fn labelled(item: &MarkdownListItem) -> Option<LabelledLink<'_>> {
  let [MarkdownBlock::Paragraph(inlines)] = item.blocks.as_slice() else {
    return None;
  };
  let (label, rest) = leading_text(inlines);
  let (link, rest) = rest.split_first()?;
  if !matches!(link, MarkdownInline::Link { .. }) {
    return None;
  }
  let label = label.trim().strip_suffix(':')?.trim();
  if label.is_empty() {
    return None;
  }
  Some(LabelledLink {
    label: label.to_owned(),
    link,
    detail: detail(rest)?,
  })
}

/// What follows the link: nothing, or a dash and then a detail that holds no link.
fn detail(rest: &[MarkdownInline]) -> Option<Vec<MarkdownInline>> {
  let (text, after) = leading_text(rest);
  if text.trim().is_empty() && after.is_empty() {
    return Some(Vec::new());
  }
  let words = text.trim_start().strip_prefix(DASHES)?.trim_start();
  if holds_link(after) {
    return None;
  }
  let mut detail = Vec::with_capacity(after.len() + 1);
  if !words.is_empty() {
    detail.push(MarkdownInline::Text(words.to_owned()));
  }
  detail.extend(after.iter().cloned());
  Some(detail)
}

/// The plain text `inlines` start with, joined, and the inlines after it.
fn leading_text(inlines: &[MarkdownInline]) -> (String, &[MarkdownInline]) {
  let count = inlines
    .iter()
    .take_while(|inline| matches!(inline, MarkdownInline::Text(_)))
    .count();
  let text = inlines[..count]
    .iter()
    .filter_map(|inline| match inline {
      MarkdownInline::Text(text) => Some(text.as_str()),
      _ => None,
    })
    .collect();
  (text, &inlines[count..])
}

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
