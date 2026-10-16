//! Links in what an agent writes in a chat (`ui::chat::links`): which ones the chat follows; and which lists of
//! links a reply draws as labelled links (`ui::chat::labelled`).

use lurq::{
  app::{App, theme::TypographyStyle},
  components::{MarkdownLinkPresentation, MarkdownRun},
  markdown::{MarkdownBlock, MarkdownInline, parse_markdown},
};
use orchester_app::{
  i18n,
  routes::Route,
  theme::{self, ChatLinkLooks, Mode},
  ui::chat::{
    labelled::labelled_links,
    links::ChatLink,
    presentation::{LinkWords, presentation},
  },
};
use orchester_core::id::ProjectId;

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
  let ChatLink::Web { url, host } = ChatLink::of("HTTPS://GitHub.com/acme/kontur/pull/42") else {
    panic!("an https address is a web page");
  };
  assert_eq!(url.as_str(), "https://github.com/acme/kontur/pull/42");
  assert_eq!(host, "github.com", "the host the browser goes to, as the link shows it");
}

#[test]
fn chat_links_show_a_web_link_without_words_as_its_address() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app).expect("bundled locales load");
  let looks = ChatLinkLooks::of(app.theme(), &app.theme().typography_style(TypographyStyle::Body));
  let words = LinkWords {
    i18n: app.i18n(),
    place: &|_| None,
  };
  for said in ["", "   "] {
    assert_eq!(
      presentation("https://evil.example/login", said, &looks, &words),
      MarkdownLinkPresentation::text()
        .trailing([MarkdownRun::new("https://evil.example/login", looks.address.clone())]),
      "{said:?}: no host can be placed without the words it belongs to"
    );
  }
  let shown = presentation("https://evil.example/login", "github.com", &looks, &words);
  assert_ne!(shown, MarkdownLinkPresentation::text(), "with words it is a link");
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
}

/// The labels, link destinations and plain details of the first list in `source`, when it reads as labelled links.
fn labelled(source: &str) -> Option<Vec<(String, String, String)>> {
  let document = parse_markdown(source);
  let items = document.blocks.iter().find_map(|block| match block {
    MarkdownBlock::List { items, .. } => Some(items),
    _ => None,
  })?;
  let rows = labelled_links(items)?;
  Some(
    rows
      .into_iter()
      .map(|row| {
        let MarkdownInline::Link { destination, .. } = row.link else {
          panic!("a row's link is a link");
        };
        let detail = row
          .detail
          .iter()
          .map(|inline| match inline {
            MarkdownInline::Text(text) => text.clone(),
            other => format!("{other:?}"),
          })
          .collect();
        (row.label, destination.clone(), detail)
      })
      .collect(),
  )
}

#[test]
fn chat_links_read_a_list_of_labelled_links_as_one() {
  let rows = labelled(
    "- Task: [KON-1 · Delivery log](orchester:/project/p/tasks/KON-1) — ready for review\n\
     - Run: [Run #3](orchester:/project/p/runs/3) – 7 steps\n\
     - Pull request: [#42](https://github.com/acme/kontur/pull/42)\n\
     - Usage:[1–15 Oct](orchester:/project/p/usage) - 6.1M tokens",
  );
  assert_eq!(
    rows,
    Some(vec![
      (
        "Task".to_owned(),
        "orchester:/project/p/tasks/KON-1".to_owned(),
        "ready for review".to_owned()
      ),
      (
        "Run".to_owned(),
        "orchester:/project/p/runs/3".to_owned(),
        "7 steps".to_owned()
      ),
      (
        "Pull request".to_owned(),
        "https://github.com/acme/kontur/pull/42".to_owned(),
        String::new()
      ),
      (
        "Usage".to_owned(),
        "orchester:/project/p/usage".to_owned(),
        "6.1M tokens".to_owned()
      ),
    ])
  );
}

#[test]
fn chat_links_keep_any_other_list_a_list() {
  for source in [
    "- Task: [KON-1](orchester:/a) — ready\n- [KON-2](orchester:/b) waits",
    "- Task [KON-1](orchester:/a) — no colon",
    "- Task: [KON-1](orchester:/a) and [KON-2](orchester:/b)",
    "- Task: [KON-1](orchester:/a) — then [KON-2](orchester:/b)",
    "- Task: [KON-1](orchester:/a), no dash",
    "- Task: no link at all",
    "- : [KON-1](orchester:/a)",
    "- Task: [KON-1](orchester:/a)\n\n  A second paragraph.",
    "- Task: **[KON-1](orchester:/a)**",
  ] {
    assert_eq!(labelled(source), None, "{source:?} stays a list");
  }
}
