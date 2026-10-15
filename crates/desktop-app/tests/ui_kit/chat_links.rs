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

#[test]
fn chat_links_never_leave_a_link_their_text_made_unchecked() {
  let cases = [
    // Brackets around a link made text pair into a new one: it is read again, and shows its host.
    (
      "[[a](mailto:x)](https://evil.example)",
      "a (mailto:x) (evil.example)",
      vec!["https://evil.example"],
    ),
    (
      "[[](mailto:x)](https://evil.example)",
      "mailto:x (evil.example)",
      vec!["https://evil.example"],
    ),
    // What a link said never becomes a reference definition.
    (
      "[[b]: https://evil.example](mailto:x)\n\nsee [b]",
      "[b]: https://evil.example (mailto:x)\nsee [b]",
      vec![],
    ),
    // A link holding another is text: its words would read as the inner one while a press went to it.
    (
      "[<https://github.com>](https://evil.example/p)",
      "https://github.com (https://evil.example/p)",
      vec![],
    ),
    (
      "[see <mailto:x@y.example>](https://github.com/acme)",
      "see mailto:x@y.example (https://github.com/acme)",
      vec![],
    ),
    // What a link said keeps no structure of its own.
    ("[# head](mailto:x)", "# head (mailto:x)", vec![]),
  ];
  for (source, reads, links) in cases {
    let rendered = shown(source);
    assert_eq!(plain_text(&rendered), reads, "{source} → {rendered}");
    assert_eq!(destinations(&rendered), links, "{source} → {rendered}");
  }
}

#[test]
fn chat_links_show_a_reply_as_text_when_its_wording_would_make_links() {
  let source = "[PR 42](https://github.com/acme/kontur/pull/42) and [mail](mailto:x)";
  let linking = |shown: Shown<'_>| match shown {
    Shown::Web { link, host } => format!("{link} ({host}) [more](mailto:y)"),
    Shown::Address { text, address } => format!("{text} ({address})"),
  };
  let rendered = present(source, &linking);
  assert_eq!(
    plain_text(&rendered),
    source,
    "a wording that adds a link: the source, as text"
  );
  assert!(destinations(&rendered).is_empty());
  let endless = |shown: Shown<'_>| match shown {
    Shown::Web { link, host } => format!("{link} ({host})"),
    Shown::Address { text, address } => format!("[{text}](mailto:{address})"),
  };
  let rendered = present(source, &endless);
  assert_eq!(
    plain_text(&rendered),
    source,
    "a reply that never settles: the source, as text"
  );
  assert!(destinations(&rendered).is_empty());
}

#[test]
fn chat_links_follow_only_places_in_a_project() {
  for path in [
    r"/add-project?folder=\\evil.example\share",
    "/add-project",
    "/settings",
    "/settings?tab=accounts",
    "/",
  ] {
    assert!(Route::parse(path).is_ok(), "{path} is a route of the app");
    assert_eq!(
      ChatLink::of(&format!("orchester:{path}")),
      ChatLink::Text,
      "{path} is outside a project: the chat never opens it"
    );
  }
  let rendered = shown(r"[open it](orchester:/add-project?folder=\\evil.example\share)");
  assert_eq!(
    plain_text(&rendered),
    r"open it (orchester:/add-project?folder=\evil.example\share)",
    "a Markdown destination reads `\\` as one backslash"
  );
  assert!(destinations(&rendered).is_empty());
}

#[test]
fn chat_links_show_no_bidirectional_controls() {
  let source = "[\u{202E}moc.buhtig](https://evil.example) and [\u{2066}KON-1\u{2069}](mailto:x\u{202D}y)";
  let rendered = shown(source);
  assert!(
    !rendered.contains([
      '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}'
    ]),
    "{rendered:?}"
  );
  assert_eq!(plain_text(&rendered), "moc.buhtig (evil.example) and KON-1 (mailto:xy)");
}
