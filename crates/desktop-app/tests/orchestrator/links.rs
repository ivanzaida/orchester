//! Links in the Orchestrator's replies (`docs/design/orchestrator-toolset.md`, "Links"; `w262B`, `agv8r`): an
//! `orchester:` link goes to its place in the active tab, for a person and for synthetic input alike, and says where
//! in its tooltip, the project first when it is another; a link to the web shows its host, says what it opens, and
//! synthetic input never opens it; a route outside a project, a route the app does not know, or another scheme is
//! its words and then its address as text. A link is a Tab stop that Enter presses, with the design's ring. A list
//! of labelled links is drawn as their grid. The fake Claude Code answers `Noted: <message>`, so the message carries
//! the reply's links. A person's press on a link to the web opens the default browser and is not made here.

use std::{
  fs,
  time::{Duration, Instant},
};

use lurq::{
  app::events::{MouseButton, ScrollPhase, with_synthetic_input},
  layout::quad::{QuadContent, RichTextSpan},
  node::color::Color,
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
/// `text_muted`, `text_secondary` and `border_focus` in the dark mode the harness shows.
const MUTED: &str = "#8A8A92";
const SECONDARY: &str = "#B3B3BA";
const FOCUS: &str = "#A3A3AB";

/// The middle of the first line of the element whose text is `label`: a link's words.
fn center_of(screen: &mut Orchestrator, label: &str) -> (f32, f32) {
  let bounds = screen
    .window
    .tree
    .find_element(|element| element.text_content() == Some(label))
    .unwrap_or_else(|| panic!("{label} is a link's words"))
    .bounds();
  (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0)
}

/// Presses the middle of the link `label`, as a person's pointer does.
fn press_text(screen: &mut Orchestrator, label: &str) {
  let (x, y) = center_of(screen, label);
  let tree = &mut screen.window.tree;
  tree.mouse_move(x, y);
  tree.mouse_down(x, y, MouseButton::Left);
  tree.mouse_up(x, y, MouseButton::Left);
  screen.window.pass();
}

/// Whether an element's text is `text`: a link's words, or a text of their own.
fn is_shown_on_its_own(screen: &mut Orchestrator, text: &str) -> bool {
  screen
    .window
    .tree
    .find_element(|element| element.text_content() == Some(text))
    .is_some()
}

/// What the tooltip of the link `label` says once the pointer rests on it: the text that shows then.
fn tooltip_shows(screen: &mut Orchestrator, label: &str, words: &str) -> bool {
  let (x, y) = center_of(screen, label);
  screen.window.tree.mouse_move(x, y);
  screen.window.pass();
  screen
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  screen.window.pass();
  let shown = is_shown_on_its_own(screen, words);
  screen.window.tree.mouse_move(0.0, 0.0);
  screen.window.pass();
  shown
}

/// Every run of text the window draws, a plain text as one span, without the invisible isolate marks around a
/// link's words and runs.
fn spans(screen: &mut Orchestrator) -> Vec<RichTextSpan> {
  screen.window.pass();
  let tree = &screen.window.tree;
  let quads = tree.resolve_quads(tree.last_layout().expect("a layout"));
  quads
    .into_iter()
    .filter_map(|quad| match quad.content {
      QuadContent::RichText { spans, .. } => Some(spans),
      QuadContent::Text { text, style, .. } => Some(vec![RichTextSpan { text, style }]),
      _ => None,
    })
    .flatten()
    .filter(|span| !matches!(span.text.as_str(), "\u{2068}" | "\u{2069}"))
    .collect()
}

fn span<'a>(spans: &'a [RichTextSpan], text: &str) -> &'a RichTextSpan {
  spans.iter().find(|span| span.text == text).unwrap_or_else(|| {
    panic!(
      "no span {text:?} in {:?}",
      spans.iter().map(|span| &span.text).collect::<Vec<_>>()
    )
  })
}

/// Back to the conversation, its reply shown.
fn back_to_the_chat(screen: &mut Orchestrator) {
  screen.window.navigation().back();
  screen.until("the conversation again", |screen| {
    screen.text("orchestrator-conversation").contains("Noted:")
  });
}

/// Waits for the answer that starts with `start` to end. A list that keeps only the turns in view may have let the
/// earlier ones go, so the answer is found by its words, not counted.
fn answer(screen: &mut Orchestrator, start: &str) {
  screen.until("the answer", |screen| {
    !screen.exists("orchestrator-thinking") && screen.text("orchestrator-conversation").contains(start)
  });
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
     [KON-99](orchester:/project/{project}/nowhere), [add it](orchester:/add-project?folder=x) and \
     [mail](mailto:team@example.com)."
  ));
  answer(&mut screen, "Noted: Look at");
  let drawn = spans(&mut screen);
  let pull = span(&drawn, "PR 42");
  assert_eq!(
    pull.style.underline.and_then(|underline| underline.color),
    Some(Color::from_hex(MUTED)),
    "a link's words over a `text_muted` underline"
  );
  let host = span(&drawn, "github.com");
  assert_eq!(
    (host.style.font_size, host.style.color),
    (11.5, Color::from_hex(MUTED)),
    "a link to the web shows its host"
  );
  for address in [
    format!("(orchester:/project/{project}/nowhere)"),
    "(orchester:/add-project?folder=x)".to_owned(),
    "(mailto:team@example.com)".to_owned(),
  ] {
    let shown = span(&drawn, &address);
    assert_eq!(&*shown.style.font_family, "Geist Mono", "{address} in mono");
  }
  for text in ["KON-99", "add it", "mail"] {
    assert!(!is_shown_on_its_own(&mut screen, text), "{text} is no link");
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

  // Display math before a link; a quoted definition over two lines; a label behind a bidirectional override.
  screen.send(&format!(
    "Last:\n\n$$x$$ [github.com](https://evil.example/login)\n\n> [repo]:\n> https://github.com/acme/kontur\n\n\
     [the repo][repo] beside [\u{202E}sksat](orchester:{tasks})"
  ));
  answer(&mut screen, "Noted: Last:");
  // The answer's end, below the fold: only what is on screen is drawn.
  let list = screen.bounds("orchestrator-conversation");
  let (x, y) = (list.x + list.width / 2.0, list.y + list.height / 2.0);
  screen.window.tree.mouse_move(x, y);
  screen.window.tree.scroll(x, y, 0.0, 5000.0, ScrollPhase::Scroll);
  screen.window.pass();
  screen.window.pass();
  let drawn = spans(&mut screen);
  span(&drawn, "evil.example");
  assert!(
    is_shown_on_its_own(&mut screen, "the repo") && is_shown_on_its_own(&mut screen, "sksat"),
    "a reference link finds a quoted definition; a link's words lose the override"
  );
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
  answer(&mut screen, "Noted: The notes");

  assert!(tooltip_shows(
    &mut screen,
    "KON-1 notes",
    "KON-1 › Conversation · opens in this tab"
  ));
  assert!(
    tooltip_shows(&mut screen, "ATL-9 import", "Atlas › ATL-9 · opens in this tab"),
    "a place in another project names the project first"
  );
  assert!(tooltip_shows(
    &mut screen,
    "PR 42",
    &format!("Opens {PULL_REQUEST} in your browser")
  ));

  // From the message field back through the links, last first, as Shift+Tab goes; the focused one has the ring.
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
  screen.window.pass();
  for words in ["PR 42", "ATL-9 import", "KON-1 notes"] {
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
      .and_then(|element| element.text_content().map(str::to_owned));
    assert_eq!(focused.as_deref(), Some(words), "Shift+Tab reaches each link");
  }
  let tree = &screen.window.tree;
  let rings = tree
    .resolve_quads(tree.last_layout().expect("a layout"))
    .into_iter()
    .filter_map(|quad| quad.border.and_then(|border| border.top))
    .filter(|top| top.color == Color::from_hex(FOCUS) && top.width == 2.0)
    .count();
  assert!(rings > 0, "the focused link shows a 2 px `border_focus` ring");
  screen.key(("Enter", "Enter"), false);
  assert_eq!(
    screen.route(),
    Route::parse(&conversation).expect("the task's Conversation route"),
    "Enter on a focused link opens its place"
  );
}

#[test]
fn orchestrator_reply_draws_labelled_links_as_a_grid_and_any_other_list_as_a_list() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let project = screen.project;
  screen.go(OrchestratorChat::New);
  screen.send(&format!(
    "Status:\n\n- Task: [KON-1 · Delivery log](orchester:/project/{project}/tasks/KON-1) — ready for review\n\
     - Pull request: [#42 Delivery log]({PULL_REQUEST})\n\nAnd:\n\n\
     - [KON-2](orchester:/project/{project}/tasks/KON-2) waits\n- Task: no link here"
  ));
  answer(&mut screen, "Noted: Status:");
  let grids = screen
    .window
    .tree
    .get_elements_by_class_name("orchestrator-link-grid")
    .len();
  assert_eq!(grids, 1, "only the list of labelled links is a grid");
  for label in ["Task", "Pull request"] {
    assert!(is_shown_on_its_own(&mut screen, label), "{label} is a label of its own");
  }
  let drawn = spans(&mut screen);
  let detail = span(&drawn, "ready for review");
  assert_eq!(
    (detail.style.font_size, detail.style.color),
    (12.5, Color::from_hex(SECONDARY)),
    "a detail in 12.5 `text_secondary`"
  );
  let link = screen
    .window
    .tree
    .find_element(|element| element.text_content() == Some("KON-1 · Delivery log"))
    .expect("the grid's link")
    .bounds();
  let after = screen
    .window
    .tree
    .find_element(|element| element.text_content() == Some("ready for review"))
    .expect("the grid's detail")
    .bounds();
  let gap = after.x - (link.x + link.width);
  assert!(
    (10.0..=14.0).contains(&gap),
    "the detail follows the link's words, 12 after them: {gap}"
  );
}
