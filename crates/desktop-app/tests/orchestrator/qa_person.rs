//! The QA gate (ORC-899, `orchester_app::mcp::qa_person`): in a QA copy, which turns it on, the MCP client counts as
//! the person for the Orchestrator's person-only actions, so a synthetic Enter sends the message with a /watch badge's
//! watch (ORC-890) and a synthetic press of *Take it here* moves the subscription. A session that does not turn it on,
//! as the installed app's never does, refuses both; only `examples/qa_app` turns it on. Synthetic Shift+Enter and
//! Ctrl+Home reach the composer's field.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::orchestrator::WatchTarget;

use super::{
  harness::{Orchestrator, Setup},
  led::{card, leader, led},
  watch_module::{BADGE, Kontur, kontur_with, messages, webhooks_chosen},
};

const INPUT: &str = "orchestrator-input";

/// Turns the QA gate on for the screen's session, as `examples/qa_app` does before its window opens.
fn as_qa_copy(screen: &Orchestrator) {
  screen
    .window
    .launch
    .services()
    .expect("the session started")
    .qa_person
    .count_mcp_client_as_person();
}

fn draft(screen: &mut Orchestrator) -> String {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .and_then(|element| element.as_text_input())
    .expect("the message field")
    .value()
}

/// A synthetic key with `shift` and `ctrl` where the focus is, as `lurq_interact` delivers it.
fn synthetic_key(screen: &mut Orchestrator, key: &str, (shift, ctrl): (bool, bool)) {
  with_synthetic_input(|| {
    let tree = &mut screen.window.tree;
    tree.key_down(key.to_owned(), key.to_owned(), shift, ctrl, false);
    tree.key_up(key.to_owned(), key.to_owned(), shift, ctrl, false);
  });
  screen.window.pass();
}

#[test]
fn orchestrator_qa_person_applies_a_synthetic_enter_watch_in_a_qa_copy() {
  let Kontur {
    mut screen,
    here,
    webhooks,
    ..
  } = kontur_with(|_| {});
  as_qa_copy(&screen);
  webhooks_chosen(&mut screen, webhooks);
  screen.set(INPUT, "Ask before merging.");

  with_synthetic_input(|| screen.key(("Enter", "Enter"), false));
  screen.until("the subscription", |screen| {
    screen
      .db()
      .active_subscriptions()
      .expect("read")
      .of(WatchTarget::Module(webhooks))
      .is_some()
  });
  let subscriptions = screen.db().active_subscriptions().expect("read");
  let led = subscriptions.of(WatchTarget::Module(webhooks)).expect("led");
  assert_eq!(led.chat(), here, "this conversation leads it");
  assert_eq!(led.order(), Some("Ask before merging."));
  assert!(
    !screen.exists("orchestrator-watch-problem"),
    "no person-only notice in a QA copy"
  );
  screen.until("the message sent and the badge gone", |screen| {
    !screen.exists(BADGE) && messages(screen, here).iter().any(|text| text == "Ask before merging.")
  });
}

#[test]
fn orchestrator_qa_person_takes_a_synthetic_take_it_here_in_a_qa_copy() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  as_qa_copy(&screen);
  let (_, mine, billing) = led(&screen);
  screen.go(OrchestratorChat::Chat(mine.id()));
  let card = card(&screen, mine.id());
  let take = format!("{card}-take");
  screen.until("the card", |screen| screen.exists(&take));

  with_synthetic_input(|| screen.click(&take));
  let words = format!("{card}-words");
  screen.until("the line in the card's place", |screen| screen.exists(&words));
  assert_eq!(leader(&screen, billing), Some(mine.id()), "the subscription moved here");
  assert!(
    !screen.exists(&format!("{card}-person-only")),
    "no refusal in a QA copy"
  );
}

#[test]
fn orchestrator_qa_person_is_off_outside_a_qa_copy_and_refuses_a_synthetic_take_it_here() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let services = screen.window.launch.services().expect("the session started");
  assert!(!services.qa_person.counts(), "a session starts with the gate off");
  let (theirs, mine, billing) = led(&screen);
  screen.go(OrchestratorChat::Chat(mine.id()));
  let card = card(&screen, mine.id());
  let take = format!("{card}-take");
  screen.until("the card", |screen| screen.exists(&take));

  with_synthetic_input(|| screen.click(&take));
  screen.until("the refusal", |screen| screen.exists(&format!("{card}-person-only")));
  assert_eq!(
    leader(&screen, billing),
    Some(theirs.id()),
    "automation's press moves nothing"
  );
}

/// Every `.rs` file under `dir`, recursively.
fn sources(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
  for entry in std::fs::read_dir(dir).unwrap_or_else(|_| panic!("{}", dir.display())) {
    let path = entry.expect("an entry").path();
    if path.is_dir() {
      sources(&path, found);
    } else if path.extension().is_some_and(|extension| extension == "rs") {
      found.push(path);
    }
  }
}

#[test]
fn orchestrator_qa_person_is_turned_on_by_the_qa_copy_only() {
  let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  let read = |path: &std::path::Path| std::fs::read_to_string(path).unwrap_or_else(|_| panic!("{}", path.display()));
  let turn_on = ".count_mcp_client_as_person()";
  assert!(
    read(&crate_dir.join("examples/qa_app/main.rs")).contains(turn_on),
    "the QA copy turns it on"
  );
  // The installed app and a normal start run only the crate's own code: no file of it turns the gate on.
  let mut found = Vec::new();
  sources(&crate_dir.join("src"), &mut found);
  assert!(found.len() > 100, "the crate's sources were read");
  for program in found {
    assert!(
      !read(&program).contains(turn_on),
      "{}, which the installed app runs, never turns it on",
      program.display()
    );
  }
}

#[test]
fn orchestrator_qa_person_synthetic_shift_enter_and_ctrl_home_reach_the_composer() {
  let Kontur { mut screen, .. } = kontur_with(|_| {});
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  screen.set(INPUT, "first");
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  synthetic_key(&mut screen, "End", (false, false));

  synthetic_key(&mut screen, "Enter", (true, false));
  assert_eq!(draft(&mut screen), "first\n", "Shift+Enter adds a line");
  synthetic_key(&mut screen, "s", (false, false));
  synthetic_key(&mut screen, "Home", (false, true));
  synthetic_key(&mut screen, "X", (false, false));
  assert_eq!(draft(&mut screen), "Xfirst\ns", "Ctrl+Home puts the caret at the start");
}
