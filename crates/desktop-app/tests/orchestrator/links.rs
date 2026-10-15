//! Links in the Orchestrator's replies (`docs/design/orchestrator-toolset.md`, "Links"; `w262B`, `agv8r`): an
//! `orchester:` link goes to its place in the active tab, for a person and for synthetic input alike, and says where
//! in its tooltip, the project first when it is another; a link to the web shows its host, says what it opens, and
//! synthetic input never opens it; a route the app does not know, or another scheme, is its words and then its
//! address as text. A link is a Tab stop that Enter presses. The fake Claude Code answers `Noted: <message>`, so the
//! message carries the reply's links. A person's press on a link to the web opens the default browser and is not
//! made here.

use std::{
  fs,
  time::{Duration, Instant},
};

use lurq::{
  app::events::{MouseButton, with_synthetic_input},
  node::element::ElementRef,
};
use orchester_app::{
  routes::{OrchestratorChat, Route},
  ui::{
    chat::links::{Followed, follow},
    tooltip::TOOLTIP_DELAY,
  },
};
use orchester_core::id::ProjectId;

use super::{
  harness::{Orchestrator, Setup, git},
  support::add_project,
};

const PULL_REQUEST: &str = "https://github.com/acme/kontur/pull/42";

/// The middle of the text `label` in the window, which only a link's words are on their own.
fn center_of(screen: &mut Orchestrator, label: &str) -> (f32, f32) {
  let bounds = screen
    .window
    .tree
    .find_element(|element| element.text_content() == Some(label))
    .unwrap_or_else(|| panic!("{label} is shown on its own"))
    .bounds();
  (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0)
}

/// Presses the middle of the text `label`, as a person's pointer does.
fn press_text(screen: &mut Orchestrator, label: &str) {
  let (x, y) = center_of(screen, label);
  let tree = &mut screen.window.tree;
  tree.mouse_move(x, y);
  tree.mouse_down(x, y, MouseButton::Left);
  tree.mouse_up(x, y, MouseButton::Left);
  screen.window.pass();
}

/// The element id of the link whose words are `label`: its words' id is `<link id>-label`.
fn link_id(screen: &mut Orchestrator, label: &str) -> String {
  fn find(element: ElementRef<'_>, label: &str) -> Option<String> {
    if element.text_content() == Some(label)
      && let Some(id) = element.id().and_then(|id| id.strip_suffix("-label"))
    {
      return Some(id.to_owned());
    }
    element.children().iter().find_map(|child| find(child, label))
  }
  screen
    .window
    .tree
    .root()
    .and_then(|root| find(root, label))
    .unwrap_or_else(|| panic!("{label} is a link's words"))
}

/// What the tooltip of the link whose words are `label` says once the pointer rests on it.
fn tooltip(screen: &mut Orchestrator, label: &str) -> String {
  let id = link_id(screen, label);
  let (x, y) = center_of(screen, label);
  screen.window.tree.mouse_move(x, y);
  screen.window.pass();
  // The pointer rests on the link past the tooltip's delay.
  screen
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  screen.window.pass();
  let panel = format!("{id}-tooltip");
  let words = screen.text(&panel);
  screen.window.tree.mouse_move(0.0, 0.0);
  screen.window.pass();
  words
}

/// Back to the conversation, its reply shown.
fn back_to_the_chat(screen: &mut Orchestrator) {
  screen.window.navigation().back();
  screen.until("the conversation again", |screen| {
    screen.text("orchestrator-conversation").contains("Noted:")
  });
}

fn is_shown_on_its_own(screen: &mut Orchestrator, text: &str) -> bool {
  screen
    .window
    .tree
    .find_element(|element| element.text_content() == Some(text))
    .is_some()
}

#[test]
fn orchestrator_reply_links_go_in_the_app_and_show_the_web_and_unknown_routes_as_they_are() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let project = screen.project;
  let tasks = format!("/project/{project}/tasks");
  let settings = format!("/project/{project}/orchestrator/settings");
  screen.go(OrchestratorChat::New);
  screen.send(&format!(
    "Look at [the tasks](orchester:{tasks}), [its settings](orchester:{settings}), [PR 42]({PULL_REQUEST}), \
     [KON-99](orchester:/project/{project}/nowhere) and [mail](mailto:team@example.com)."
  ));
  screen.until("the answer", |screen| {
    screen.text("orchestrator-conversation").contains("Noted:")
  });
  // The harness joins the texts with a space: a link's words and its host are texts of their own.
  let reply = screen.text("orchestrator-conversation");
  assert!(
    reply.contains("PR 42 github.com"),
    "a link to the web shows its host: {reply}"
  );
  for address in [
    format!("(orchester:/project/{project}/nowhere)"),
    "(mailto:team@example.com)".to_owned(),
  ] {
    assert!(
      is_shown_on_its_own(&mut screen, &address),
      "a link the chat does not follow is its words, then its address: {reply}"
    );
  }
  for text in ["KON-99", "mail"] {
    assert!(!is_shown_on_its_own(&mut screen, text), "{text} is no link of its own");
  }

  press_text(&mut screen, "the tasks");
  assert_eq!(
    screen.route(),
    Route::parse(&tasks).expect("the Tasks route"),
    "a person's press goes there"
  );

  back_to_the_chat(&mut screen);
  with_synthetic_input(|| press_text(&mut screen, "its settings"));
  assert_eq!(
    screen.route(),
    Route::parse(&settings).expect("the settings route"),
    "synthetic input may follow a link in the app"
  );

  back_to_the_chat(&mut screen);
  let chat = screen.route();
  let followed = with_synthetic_input(|| follow(&screen.window.navigation(), PULL_REQUEST));
  assert!(
    matches!(followed, Followed::NotOpened),
    "synthetic input never opens the browser: {followed:?}"
  );
  with_synthetic_input(|| press_text(&mut screen, "PR 42"));
  assert_eq!(screen.route(), chat, "a link to the web leaves the tab where it is");

  // Display math before a link makes lurq read that paragraph as two blocks; it is the reply's only link.
  screen.send("Last:\n\n$$x$$ [github.com](https://evil.example/login)");
  let reply = answer(&mut screen, 2);
  assert!(
    reply.contains("(evil.example)") || reply.contains("github.com evil.example"),
    "a link however its reply splits shows the host it opens: {reply}"
  );

  // A quoted definition spans `>` marks; a label hides behind a bidirectional override.
  screen.send(&format!(
    "Then:\n\n> [repo]:\n> https://github.com/acme/kontur\n\n[the repo][repo] beside [\u{202E}sksat](orchester:{tasks})"
  ));
  let reply = answer(&mut screen, 3);
  assert!(
    is_shown_on_its_own(&mut screen, "the repo") && reply.contains("the repo github.com"),
    "a reference link finds a definition written over two quoted lines: {reply}"
  );
  assert!(
    is_shown_on_its_own(&mut screen, "sksat"),
    "a link's words without the override that turned them around: {reply}"
  );
}

/// The `count`th answer once it ended, as the conversation shows it.
fn answer(screen: &mut Orchestrator, count: usize) -> String {
  screen.until("the answer", |screen| {
    !screen.exists("orchestrator-thinking")
      && screen.text("orchestrator-conversation").matches("Noted:").count() == count
  });
  let shown = screen.text("orchestrator-conversation");
  shown.rsplit("Noted:").next().unwrap_or_default().to_owned()
}

#[test]
fn orchestrator_reply_links_say_where_they_go_and_take_the_keyboard() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let project = screen.project;
  let atlas = ProjectId::generate();
  let repo = screen.root.with_file_name("atlas");
  fs::create_dir_all(&repo).expect("Atlas's folder");
  git(&repo, &["init", "--quiet", "--initial-branch=main"]);
  let workspace = screen.window.launch.workspace().expect("the session started");
  add_project(workspace, (atlas, "ATL", "Atlas"), &repo);
  let conversation = format!("/project/{project}/tasks/KON-1/conversation");
  screen.go(OrchestratorChat::New);
  screen.send(&format!(
    "The notes are in [KON-1 notes](orchester:{conversation}); Atlas runs \
     [ATL-9 import](orchester:/project/{atlas}/tasks/ATL-9/summary); see [PR 42]({PULL_REQUEST})."
  ));
  screen.until("the answer", |screen| {
    screen.text("orchestrator-conversation").contains("Noted:")
  });

  assert_eq!(
    tooltip(&mut screen, "KON-1 notes"),
    "KON-1 › Conversation · opens in this tab"
  );
  assert_eq!(
    tooltip(&mut screen, "ATL-9 import"),
    "Atlas › ATL-9 · opens in this tab",
    "a place in another project names the project first"
  );
  assert_eq!(
    tooltip(&mut screen, "PR 42"),
    format!("Opens {PULL_REQUEST} in your browser")
  );

  // From the message field back through the links, last first, as Shift+Tab goes.
  let links = ["PR 42", "ATL-9 import", "KON-1 notes"].map(|label| link_id(&mut screen, label));
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
  screen.window.pass();
  for id in &links {
    screen
      .window
      .tree
      .key_down("Tab".to_owned(), "Tab".to_owned(), true, false, false);
    screen
      .window
      .tree
      .key_up("Tab".to_owned(), "Tab".to_owned(), true, false, false);
    screen.window.pass();
    let focused = screen
      .window
      .tree
      .focused_element()
      .and_then(|element| element.id().map(str::to_owned));
    assert_eq!(focused.as_deref(), Some(id.as_str()), "Shift+Tab reaches each link");
  }
  screen.key(("Enter", "Enter"), false);
  assert_eq!(
    screen.route(),
    Route::parse(&conversation).expect("the task's Conversation route"),
    "Enter on a focused link opens its place"
  );
}
