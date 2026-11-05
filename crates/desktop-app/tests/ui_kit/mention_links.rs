//! A person's message with @-mentions as a chat shows it (ORC-312, ORC-316, `ui::chat::said`): the words as written,
//! whatever Markdown would make of them, each mention a link to its task, to its run's task's Conversation, or by its
//! name to its file in Files; and the chips a composer draws over the same words
//! (`ui::mention::MentionField::tokens`).

use lurq::markdown::{MarkdownBlock, MarkdownInline, parse_markdown};
use orchester_app::ui::{chat::said::linked, mention::MentionField};
use orchester_core::id::{ProjectId, RunId};
use orchester_workspace::{files::Version, mentions::find};

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

/// The paragraphs `words` show as a chat links them in `project`, its files at `files_at`.
fn paragraphs(words: &str, (project, files_at): (ProjectId, &Version)) -> Vec<String> {
  let document = parse_markdown(&linked(words, &find(words), (project, files_at)));
  document
    .blocks
    .iter()
    .map(|block| match block {
      MarkdownBlock::Paragraph(inlines) => shown(inlines),
      other => panic!("only paragraphs: {other:?}"),
    })
    .collect()
}

#[test]
fn mention_links_show_the_words_as_written_and_link_each_mention() {
  let project = ProjectId::generate();
  let words = "Compare @KON-12 with @KON-12#9, not *this*\n  # or 1. that\n\nNext: (@KON-3)";
  let task = |key: &str, tab: &str| format!("orchester:/project/{project}/tasks/{key}/{tab}");
  assert_eq!(
    paragraphs(words, (project, &Version::Checkout)),
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
fn mention_links_open_a_file_by_its_name_in_files() {
  let project = ProjectId::generate();
  let words = "Check @`crates/webhooks/src/sender.rs` and @`README.md`.";
  assert_eq!(
    paragraphs(words, (project, &Version::Checkout)),
    [format!(
      "Check [sender.rs](orchester:/project/{project}/files/crates/webhooks/src/sender.rs) and \
       [README.md](orchester:/project/{project}/files/README.md)."
    )],
    "the Orchestrator's files are the project's checkout"
  );
  let run = RunId::generate();
  assert_eq!(
    paragraphs("See @`src/a b.rs`", (project, &Version::Run(run))),
    [format!(
      "See [a b.rs](orchester:/project/{project}/files/src/a%20b.rs?at=run%3A{run})"
    )],
    "a run's files are its worktree's"
  );
}

#[test]
fn mention_links_draw_each_mention_of_a_draft_as_a_chip() {
  let draft = "Ask @KON-1 about @KON-2#4 and mail@KON-3, or @`src/lib.rs`, not @`open or @`a`b";
  let tokens = MentionField::tokens(draft);
  assert_eq!(
    format!("{tokens:?}"),
    "InputTokens { .. }",
    "the chips never show the draft in a debug line"
  );
  let ranges: Vec<&str> = find(draft).into_iter().map(|found| &draft[found.range]).collect();
  assert_eq!(
    ranges,
    ["@KON-1", "@KON-2#4", "@`src/lib.rs`"],
    "an @ inside a word is no chip, nor a path without its closing backtick or with a letter after it"
  );
}
