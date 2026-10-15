//! Links in what an agent writes in a chat (`ui::chat::links`): which ones the chat follows, and what the others
//! read as once rendered. The words are the shipped English ones, `orchestrator.link.*`.

use std::borrow::Cow;

use lurq::markdown::{MarkdownBlock, MarkdownInline, parse_markdown};
use orchester_app::{
  routes::Route,
  ui::{
    chat::links::{ChatLink, Shown, present},
    markdown::plain::plain_text,
  },
};
use orchester_core::id::ProjectId;

fn words(shown: Shown<'_>) -> String {
  match shown {
    Shown::Web { link, host } => format!("{link} ({host})"),
    Shown::Address { text, address } => format!("{text} ({address})"),
  }
}

fn shown(source: &str) -> Cow<'_, str> {
  present(source, &words)
}

/// The destinations of the links `markdown` renders.
fn destinations(markdown: &str) -> Vec<String> {
  fn inlines(found: &mut Vec<String>, all: &[MarkdownInline]) {
    for inline in all {
      match inline {
        MarkdownInline::Link { destination, .. } => found.push(destination.clone()),
        MarkdownInline::Emphasis(children) | MarkdownInline::Strong(children) => inlines(found, children),
        _ => {}
      }
    }
  }
  fn blocks(found: &mut Vec<String>, all: &[MarkdownBlock]) {
    for block in all {
      match block {
        MarkdownBlock::Paragraph(children) | MarkdownBlock::Heading { children, .. } => inlines(found, children),
        MarkdownBlock::List { items, .. } => items.iter().for_each(|item| blocks(found, &item.blocks)),
        MarkdownBlock::Table { rows, .. } => rows
          .iter()
          .flat_map(|row| &row.cells)
          .for_each(|cell| inlines(found, cell)),
        _ => {}
      }
    }
  }
  let mut found = Vec::new();
  blocks(&mut found, &parse_markdown(markdown).blocks);
  found
}

#[test]
fn chat_links_name_a_place_in_the_app_a_web_page_or_nothing() {
  let project = ProjectId::generate();
  let task = format!("/project/{project}/tasks/KON-12/summary");
  assert_eq!(
    ChatLink::of(&format!("orchester:{task}")),
    ChatLink::App(Route::parse(&task).expect("a task's route"))
  );
  assert_eq!(
    ChatLink::of(&format!("Orchester:{task}")),
    ChatLink::App(Route::parse(&task).expect("a task's route")),
    "a scheme is read ignoring case"
  );
  for text in [
    format!("orchester:/project/{project}/nowhere"),
    "orchester:/project/not-an-id/tasks".to_owned(),
    format!("orchester://project/{project}/tasks"),
    "orchester:".to_owned(),
    "http://github.com/acme".to_owned(),
    "mailto:someone@example.com".to_owned(),
    "file:///C:/Windows/System32/cmd.exe".to_owned(),
    "javascript:alert(1)".to_owned(),
    "/project/x/tasks".to_owned(),
    "https://".to_owned(),
    String::new(),
  ] {
    assert_eq!(ChatLink::of(&text), ChatLink::Text, "{text} is not followed");
  }
  let ChatLink::Web(url) = ChatLink::of("HTTPS://github.com/acme/kontur/pull/42") else {
    panic!("an https address is a web page");
  };
  assert_eq!(url.as_str(), "https://github.com/acme/kontur/pull/42");
}

#[test]
fn chat_links_keep_a_reply_without_links_or_with_places_in_the_app_as_written() {
  let project = ProjectId::generate();
  for source in [
    "No links here.".to_owned(),
    "A code span stays as written: `[KON-1](mailto:x)`.".to_owned(),
    format!("See [KON-12](orchester:/project/{project}/tasks/KON-12/summary) and <orchester:/project/{project}>."),
  ] {
    assert!(
      matches!(shown(&source), Cow::Borrowed(kept) if kept == source),
      "{source} is shown as written"
    );
  }
}

#[test]
fn chat_links_show_a_web_page_with_its_host() {
  let source = "Opened [PR 42](https://github.com/acme/kontur/pull/42).";
  let rendered = shown(source);
  assert_eq!(plain_text(&rendered), "Opened PR 42 (github.com).");
  assert_eq!(destinations(&rendered), ["https://github.com/acme/kontur/pull/42"]);
  let tricked = shown("[github.com](https://github.com@evil.example/acme)");
  assert_eq!(
    plain_text(&tricked),
    "github.com (evil.example)",
    "the host is the one the browser goes to"
  );
}

#[test]
fn chat_links_show_what_they_do_not_follow_as_text_with_its_address() {
  let project = ProjectId::generate();
  let cases = [
    (
      format!("Look at [KON-99](orchester:/project/{project}/nowhere) now."),
      format!("Look at KON-99 (orchester:/project/{project}/nowhere) now."),
    ),
    (
      "[site](http://a.example/*b*_c_)".to_owned(),
      "site (http://a.example/*b*_c_)".to_owned(),
    ),
    (
      "[**KON-1**](mailto:a@b.example)".to_owned(),
      "KON-1 (mailto:a@b.example)".to_owned(),
    ),
    ("[](javascript:alert(1))".to_owned(), "javascript:alert(1)".to_owned()),
    ("<http://a.example>".to_owned(), "http://a.example".to_owned()),
    (
      "[x][1]\n\n[1]: file:///etc/passwd".to_owned(),
      "x (file:///etc/passwd)".to_owned(),
    ),
  ];
  for (source, reads) in cases {
    let rendered = shown(&source);
    assert_eq!(plain_text(&rendered), reads, "{source}");
    assert!(
      destinations(&rendered).is_empty(),
      "{source} has no link left: {rendered}"
    );
  }
}

#[test]
fn chat_links_keep_the_reply_around_them_as_it_reads() {
  let project = ProjectId::generate();
  let source = format!(
    "| Task | Pull request |\n|---|---|\n| [KON-1](orchester:/project/{project}/tasks/KON-1/summary) | \
     [#42](https://github.com/acme/kontur/pull/42) |\n| [KON-2](orchester:/project/{project}/no) | — |\n\n\
     - *[done](mailto:x@y.example)* first\n"
  );
  let rendered = shown(&source);
  assert_eq!(
    plain_text(&rendered),
    format!(
      "Task Pull request \nKON-1 #42 (github.com) \nKON-2 (orchester:/project/{project}/no) — \ndone (mailto:x@y.example) first"
    )
  );
  assert_eq!(
    destinations(&rendered),
    [
      format!("orchester:/project/{project}/tasks/KON-1/summary"),
      "https://github.com/acme/kontur/pull/42".to_owned()
    ]
  );
}
