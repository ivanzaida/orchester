//! Answering on the cards that wait (the seed holds their steps waiting for the person, so they wait without a
//! runner): a command's card with the request whole and every answer, one answer at a time, and why it did
//! not go through; a question's options and own answer, *Send answer* and why it waits; the focus, once.

use lurq::app::events::MouseButton;
use orchester_app::{
  routes::{ProjectPage, Route, TaskView},
  ui::icon::Icon,
};

use super::{card_of, click, glyphs, locked, opened, text};
use crate::support::Window;

/// Types `keys` into the element that has the focus, one key at a time.
fn type_keys(window: &mut Window, keys: &[&str]) {
  for key in keys {
    let code = match *key {
      "Backspace" => "Backspace".to_owned(),
      letter => format!("Key{}", letter.to_uppercase()),
    };
    window.tree.key_down((*key).to_owned(), code, false, false, false);
    window.pass();
  }
}

fn focused(window: &Window) -> Option<String> {
  window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
}

fn loading(window: &mut Window, id: &str) -> bool {
  glyphs(window, id).contains(&Icon::LoaderCircle.glyph().to_string())
}

#[test]
fn card_approval_waiting_shows_the_request_whole_and_every_answer() {
  let (mut window, card) = card_of("Add serde to the webhook payloads", "cards-approval-");
  window.until("the request's choices", |window| {
    window.tree.get_element_by_id(&format!("{card}-reason")).is_some()
  });
  assert_eq!(text(&mut window, &format!("{card}-title")), "Approve command?");
  assert!(text(&mut window, &format!("{card}-since")).starts_with("waiting since "));
  assert_eq!(text(&mut window, &format!("{card}-category")), "Network");
  assert_eq!(
    text(&mut window, &format!("{card}-reason")),
    "Installs a package from the network: crates.io · serde"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-value")),
    "cargo add serde --features derive",
    "the command whole"
  );
  assert_eq!(text(&mut window, &format!("{card}-value-copy")), "Copy");
  let agent = text(&mut window, &format!("{card}-agent"));
  assert!(agent.starts_with("Builder · 03 Implement · Run "), "{agent}");
  assert!(text(&mut window, &format!("{card}-note")).contains("The step needs it to go on."));
  // The static seed has no agent connection, so the words are Orchester's, not Codex's own.
  assert_eq!(
    text(&mut window, &format!("{card}-deny-tells")),
    "Deny tells Builder: “Denied by the person reviewing this run. Continue without it.”",
    "what the agent gets, word for word"
  );
  assert_eq!(text(&mut window, &format!("{card}-deny")), "Deny");
  assert_eq!(text(&mut window, &format!("{card}-approve")), "Approve once");
  assert!(locked(&mut window, &format!("{card}-deny-message")));
  assert!(text(&mut window, &format!("{card}-deny-message-reason")).contains("Deny, then write to Builder"));
  let similar = text(&mut window, &format!("{card}-similar"));
  assert!(
    similar.starts_with("Approve similar in this run cargo add commands in Network"),
    "{similar}"
  );
  assert!(!locked(&mut window, &format!("{card}-similar")));
  let everything = text(&mut window, &format!("{card}-everything"));
  assert!(
    everything.starts_with("Approve everything in this run Approves this one"),
    "{everything}"
  );
  assert!(locked(&mut window, &format!("{card}-always")));
  assert_eq!(
    text(&mut window, &format!("{card}-always")),
    "Always allow… Not available yet: adding a command to a profile from an approval is planned."
  );
  assert_eq!(
    text(&mut window, &format!("{card}-automation")),
    "Automation through Orchester's MCP server may only deny it: only a person approves a request like this."
  );
}

#[test]
fn card_approval_takes_one_answer_at_a_time_and_says_why_it_did_not_go() {
  let (mut window, card) = card_of("Add serde to the webhook payloads", "cards-approval-");
  let (approve, deny) = (format!("{card}-approve"), format!("{card}-deny"));
  // Approve once pressed twice and Deny right after, before a frame: only the first counts. A second press
  // that cancelled it would leave "answered already" or "no longer waits" beside live buttons.
  press_together(&mut window, &[&approve, &approve, &deny]);
  window.until("why the answer did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
  let refused = text(&mut window, &format!("{card}-refused"));
  assert!(!refused.contains("answered already"), "{refused}");
  assert!(!refused.contains("no longer waits"), "{refused}");
  assert!(!loading(&mut window, &approve), "the request still waits");
  assert_eq!(text(&mut window, &approve), "Approve once");
  assert!(window.tree.get_element_by_id(&deny).is_some());
}

/// Presses each of `ids` in turn with no frame between them, as a quick double click or Enter held does.
fn press_together(window: &mut Window, ids: &[&str]) {
  for id in ids {
    let bounds = window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    window.tree.mouse_move(x, y);
    window.tree.mouse_down(x, y, MouseButton::Left);
    window.tree.mouse_up(x, y, MouseButton::Left);
  }
  window.pass();
}

#[test]
fn card_question_with_options_waits_for_an_answer_then_sends_it() {
  let (mut window, card) = card_of("Pick where the docs button goes", "cards-choice-");
  let shown = text(&mut window, &card);
  for words in [
    "Builder asks you",
    "Waiting for your answer",
    "Where should the docs button go?",
    "Builder's options",
    "In the top bar",
    "Beside the account menu, where people look for help.",
  ] {
    assert!(shown.contains(words), "{words:?} in {shown}");
  }
  let (send, context) = (format!("{card}-send"), format!("{card}-context"));
  assert_eq!(
    text(&mut window, &context),
    "Pick one of Builder's options or write your own answer"
  );
  click(&mut window, &send);
  assert!(
    window.tree.get_element_by_id(&format!("{card}-refused")).is_none(),
    "Send answer waits for an answer"
  );
  click(&mut window, &format!("{card}-option-0-1"));
  assert!(text(&mut window, &context).starts_with("Asked at "));
  click(&mut window, &send);
  window.until("why the answer did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
}

#[test]
fn card_question_own_answer_replaces_the_pick_and_erasing_it_brings_none_back() {
  let (mut window, card) = card_of("Pick where the docs button goes", "cards-choice-");
  let radio = format!("{card}-option-0-1-mark");
  // A picked radio draws its center inside its ring.
  let check = |window: &mut Window| {
    window
      .tree
      .get_element_by_id(&radio)
      .map_or(0, |mark| mark.children().len())
  };
  let unpicked = check(&mut window);
  click(&mut window, &format!("{card}-option-0-1"));
  let picked = check(&mut window);
  click(&mut window, &format!("{card}-own-0"));
  type_keys(&mut window, &["x"]);
  type_keys(&mut window, &["Backspace"]);
  assert_eq!(
    check(&mut window),
    unpicked,
    "the own answer replaced the pick, which stays gone"
  );
  assert_ne!(picked, unpicked, "the pick showed");
  assert_eq!(
    text(&mut window, &format!("{card}-context")),
    "Pick one of Builder's options or write your own answer"
  );
}

#[test]
fn card_question_with_several_answers_checks_each() {
  let (mut window, card) = card_of("Choose the export formats", "cards-choice-");
  assert!(text(&mut window, &card).contains("Builder's options · pick one or more"));
  for option in ["0-0", "0-2"] {
    click(&mut window, &format!("{card}-option-{option}"));
  }
  let check = Icon::Check.glyph().to_string();
  for (option, on) in [("0-0", true), ("0-1", false), ("0-2", true)] {
    let marked = glyphs(&mut window, &format!("{card}-option-{option}-mark")).contains(&check);
    assert_eq!(marked, on, "option {option}");
  }
}

#[test]
fn card_that_waits_when_the_chat_opens_takes_the_focus_once() {
  let (mut window, card, (project, _)) = opened("Pick where the docs button goes", "cards-choice-");
  let first = format!("{card}-option-0-0");
  window.until("the focus on the first option", |window| {
    focused(window).as_deref() == Some(first.as_str())
  });
  let own = format!("{card}-own-0");
  click(&mut window, &own);
  // The run changes and the chat renders again: the focus stays where the person put it.
  for _ in 0..3 {
    window.projects().feed(project).changes.update(|count| *count += 1);
    window.tree.tick_futures();
    window.pass();
  }
  assert_ne!(focused(&window).as_deref(), Some(first.as_str()));
}

#[test]
fn card_question_keeps_what_was_picked_and_typed_as_the_tabs_switch() {
  let (mut window, card, (project, key)) = opened("Choose the export formats", "cards-choice-");
  click(&mut window, &format!("{card}-option-0-2"));
  click(&mut window, &format!("{card}-own-0"));
  type_keys(&mut window, &["x", "m", "l"]);
  for view in [TaskView::Summary, TaskView::Conversation] {
    window
      .navigation()
      .go(Route::Project(project, ProjectPage::Task(key.clone(), view)));
    window.tree.tick_futures();
    window.pass();
  }
  let mark = format!("{card}-option-0-2-mark");
  window.until("the card again", |window| {
    window.tree.get_element_by_id(&mark).is_some()
  });
  let check = Icon::Check.glyph().to_string();
  assert!(glyphs(&mut window, &mark).contains(&check), "the pick is kept");
  assert_eq!(
    window.texts(&format!("{card}-own-0")).unwrap_or_default().join(""),
    "xml",
    "the own answer is kept"
  );
}

#[test]
fn card_orchesters_own_read_outside_says_why_it_asks() {
  let (mut window, card) = card_of("Read the docs of the sibling project", "cards-approval-");
  window.until("the request's choices", |window| {
    window.tree.get_element_by_id(&format!("{card}-similar")).is_some()
  });
  assert_eq!(text(&mut window, &format!("{card}-title")), "Approve tool action?");
  let explain = text(&mut window, &format!("{card}-explain"));
  assert!(
    explain.starts_with("Builder wants to read outside the worktree"),
    "{explain}"
  );
  let folder = std::path::Path::new("kontur-docs").join("specs").display().to_string();
  let reason = text(&mut window, &format!("{card}-reason"));
  assert!(
    reason.starts_with("Reads outside the worktree: ") && reason.ends_with(&folder),
    "{reason}"
  );
  let subject = text(&mut window, &format!("{card}-subject"));
  assert!(!subject.contains("Asks before MCP actions"), "{subject}");
  let similar = text(&mut window, &format!("{card}-similar"));
  assert!(
    similar.starts_with("Approve similar in this run Reads and searches outside the worktree in "),
    "{similar}"
  );
}
