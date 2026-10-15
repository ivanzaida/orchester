//! Links in the Orchestrator's replies (`docs/design/orchestrator-toolset.md`, "Links"): an `orchester:` link goes to
//! its place in the active tab, for a person and for synthetic input alike; a link to the web shows its host and
//! synthetic input never opens it; a route the app does not know is text with its address. The fake Claude Code
//! answers `Noted: <message>`, so the message carries the reply's links. A person's press on a link to the web
//! opens the default browser and is not made here.

use lurq::app::events::{MouseButton, with_synthetic_input};
use orchester_app::{
  routes::Route,
  ui::chat::links::{Followed, follow},
};

use super::harness::{Orchestrator, Setup};

const PULL_REQUEST: &str = "https://github.com/acme/kontur/pull/42";

/// Presses the middle of the text `label` in the chat, as a person's pointer does.
fn press_text(screen: &mut Orchestrator, label: &str) {
  let bounds = screen
    .window
    .tree
    .find_element(|element| element.text_content() == Some(label))
    .unwrap_or_else(|| panic!("{label} is shown on its own"))
    .bounds();
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  let tree = &mut screen.window.tree;
  tree.mouse_move(x, y);
  tree.mouse_down(x, y, MouseButton::Left);
  tree.mouse_up(x, y, MouseButton::Left);
  screen.window.pass();
}

/// Back to the conversation, its reply shown.
fn back_to_the_chat(screen: &mut Orchestrator) {
  screen.window.navigation().back();
  screen.until("the conversation again", |screen| {
    screen.text("orchestrator-conversation").contains("Noted:")
  });
}

#[test]
fn orchestrator_reply_links_go_in_the_app_and_show_the_web_and_unknown_routes_as_they_are() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let project = screen.project;
  let tasks = format!("/project/{project}/tasks");
  let settings = format!("/project/{project}/orchestrator/settings");
  screen.go(orchester_app::routes::OrchestratorChat::New);
  screen.send(&format!(
    "Look at [the tasks](orchester:{tasks}), [its settings](orchester:{settings}), [PR 42]({PULL_REQUEST}), \
     [KON-99](orchester:/project/{project}/nowhere) and [mail](mailto:team@example.com)."
  ));
  screen.until("the answer", |screen| {
    screen.text("orchestrator-conversation").contains("Noted:")
  });
  // The harness joins the texts with a space: a link is a text of its own.
  let reply = screen.text("orchestrator-conversation");
  assert!(
    reply.contains("PR 42  (github.com)"),
    "a link to the web shows its host: {reply}"
  );
  assert!(
    reply.contains(&format!("KON-99 (orchester:/project/{project}/nowhere)")),
    "an unknown route is text with its address: {reply}"
  );
  assert!(
    reply.contains("mail (mailto:team@example.com)"),
    "another scheme is text: {reply}"
  );
  for text in ["KON-99", "mail"] {
    assert!(
      screen
        .window
        .tree
        .find_element(|element| element.text_content() == Some(text))
        .is_none(),
      "{text} is no link of its own"
    );
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
}
