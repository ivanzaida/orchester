//! A person's message with @-mentions as a chat shows it (ORC-312, `ui::chat::said`): the words as written, whatever
//! Markdown would make of them, each mention a link to its task, or to its run's task's Conversation; and the chips a
//! composer draws over the same words (`ui::mention::MentionField::tokens`).

use lurq::markdown::{MarkdownBlock, MarkdownInline, parse_markdown};
use orchester_app::ui::{chat::said::linked, mention::MentionField};
use orchester_core::id::ProjectId;
use orchester_workspace::mentions::find;

/// What `inlines` show, a link as `[words](destination)` and a hard break as a line break.
fn shown(inlines: &[MarkdownInline]) -> String {
  inlines
    .iter()
    .map(|inline| match inline {
      MarkdownInline::Text(text) => text.clone(),
      MarkdownInline::Link {
        destination, children, ..
      } => format!("[{}]({destination})", shown(children)),
      MarkdownInline::HardBreak => "\n".to_owned(),
      other => panic!("only text, links and line breaks: {other:?}"),
    })
    .collect()
}

#[test]
fn mention_links_show_the_words_as_written_and_link_each_mention() {
  let project = ProjectId::generate();
  let words = "Compare @KON-12 with @KON-12#9, not *this*\n  # or 1. that\n\nNext: (@KON-3)";
  let document = parse_markdown(&linked(words, &find(words), project));
  let paragraphs: Vec<String> = document
    .blocks
    .iter()
    .map(|block| match block {
      MarkdownBlock::Paragraph(inlines) => shown(inlines),
      other => panic!("only paragraphs: {other:?}"),
    })
    .collect();
  let task = |key: &str, tab: &str| format!("orchester:/project/{project}/tasks/{key}/{tab}");
  assert_eq!(
    paragraphs,
    [
      format!(
        "Compare [KON-12]({}) with [KON-12 · #9]({}), not *this*\n\u{a0}\u{a0}# or 1. that",
        task("KON-12", "summary"),
        task("KON-12", "conversation")
      ),
      format!("Next: ([KON-3]({}))", task("KON-3", "summary")),
    ],
    "a task opens its page and a run its Conversation; the rest shows as written, line by line"
  );
}

#[test]
fn mention_links_draw_each_mention_of_a_draft_as_a_chip() {
  let draft = "Ask @KON-1 about @KON-2#4 and mail@KON-3";
  let tokens = MentionField::tokens(draft);
  assert_eq!(
    format!("{tokens:?}"),
    "InputTokens { .. }",
    "the chips never show the draft in a debug line"
  );
  let ranges: Vec<&str> = find(draft).into_iter().map(|found| &draft[found.range]).collect();
  assert_eq!(ranges, ["@KON-1", "@KON-2#4"], "an @ inside a word is no chip");
}
