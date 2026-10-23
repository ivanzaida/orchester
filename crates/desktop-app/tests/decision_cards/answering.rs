//! Answering on the cards that wait (the seed holds their steps waiting for the person, so they wait without a
//! runner): a command's card with the request whole, why auto mode left it to the person, and every answer, one
//! answer at a time, and why it did not go through; a question's options and own answer, *Send answer* and why it
//! waits; the focus, once; an MCP client's answer, refused until the person lets automation answer.

use lurq::app::events::MouseButton;
use orchester_app::{
  routes::{ProjectPage, Route, TaskView},
  ui::icon::Icon,
};

use super::{card_of, click, glyphs, laid_out, locked, opened, text};
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
  // The seed's auto mode left it to the person by a hard rule: the card says which.
  assert_eq!(
    text(&mut window, &format!("{card}-hold-cause")),
    "Auto mode didn’t review this: the step’s network is off, and this command may reach it or install."
  );
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
  assert_eq!(
    text(&mut window, &format!("{card}-deny-message")),
    "Deny with a message"
  );
  assert!(!locked(&mut window, &format!("{card}-deny-message")));
  assert!(
    window
      .tree
      .get_element_by_id(&format!("{card}-deny-message-reason"))
      .is_none(),
    "a CLI's denial carries words"
  );
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

/// Presses each of `ids` in turn with no frame between them, as a quick double click or Enter held does; the
/// frames before the first press only wait for every one to be laid out.
fn press_together(window: &mut Window, ids: &[&str]) {
  let places: Vec<_> = ids.iter().map(|id| laid_out(window, id)).collect();
  for bounds in places {
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
    window.projects().feed(project).states.update(|count| *count += 1);
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
  let title = window
    .tree
    .get_element_by_id_mut(&format!("{card}-title"))
    .and_then(|element| element.bounds())
    .expect("the title is laid out");
  // One line of 13/1.35, as the command card's title.
  assert!(title.height < 20.0, "the title takes one line: {}", title.height);
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
  assert!(
    window.tree.get_element_by_id(&format!("{card}-hold-cause")).is_none(),
    "auto mode did not look at it, so the card says nothing of it"
  );
}

#[test]
fn card_question_own_answer_set_by_a_tool_counts_and_is_kept() {
  let (mut window, card, (project, key)) = opened("Pick where the docs button goes", "cards-choice-");
  let own = format!("{card}-own-0");
  // As a paste or an accessibility tool sets it: the value only, no typing.
  window
    .tree
    .get_element_by_id_mut(&own)
    .and_then(|element| element.as_text_input())
    .expect("the own answer's field")
    .set_value("Under the search box");
  window.pass();
  window.pass();
  let context = format!("{card}-context");
  assert!(
    text(&mut window, &context).starts_with("Asked at "),
    "the answer counts: {}",
    text(&mut window, &context)
  );
  for view in [TaskView::Summary, TaskView::Conversation] {
    window
      .navigation()
      .go(Route::Project(project, ProjectPage::Task(key.clone(), view)));
    window.tree.tick_futures();
    window.pass();
  }
  window.until("the card again", |window| window.tree.get_element_by_id(&own).is_some());
  assert_eq!(
    window.texts(&own).unwrap_or_default().join(""),
    "Under the search box",
    "the own answer is kept"
  );
}

/// What the card says when input from an MCP client gives an answer while automation may not answer.
const PERSON_ONLY: &str = "Only a person answers this here: input from an MCP client answers only while you let \
                           automation answer approvals.";

#[test]
fn card_approval_takes_an_mcp_clients_answer_only_as_automation_under_consent() {
  let (mut window, card) = card_of("Add serde to the webhook payloads", "cards-approval-");
  let (approve, refused) = (format!("{card}-approve"), format!("{card}-refused"));
  lurq::app::events::with_synthetic_input(|| click(&mut window, &approve));
  window.until("why the client's answer was not taken", |window| {
    window.texts(&refused).is_some()
  });
  assert_eq!(text(&mut window, &refused), PERSON_ONLY);
  let workspace = window.launch.workspace().expect("the session started");
  workspace
    .set_automation_answers_approvals(true)
    .expect("the approvals opt-in");
  assert!(
    !workspace.automation_permission_consent(),
    "permission setup is a separate consent"
  );
  lurq::app::events::with_synthetic_input(|| click(&mut window, &approve));
  // No runner here: automation's answer reaches none, and the card says so instead.
  window.until("the client's answer sent", |window| {
    window
      .texts(&refused)
      .is_some_and(|texts| texts.join(" ") != PERSON_ONLY)
  });
}

/// What the card says when input from an MCP client gives a wider answer.
const WIDER_PERSON_ONLY: &str = "Only a person gives a wider answer: automation approves a request once or denies it.";

#[test]
fn card_qa_start_offers_approval_for_every_run_of_the_project_to_a_person_only() {
  let (mut window, card) = card_of("Check the board in a QA copy", "cards-approval-");
  let (project, question) = (format!("{card}-project"), format!("{card}-widen"));
  window.until("the request's choices", |window| {
    window.tree.get_element_by_id(&project).is_some()
  });
  assert_eq!(
    text(&mut window, &project),
    "Approve for every run of this project Orchester start_qa_app in every run of this project while \
     tools/qa-app.ps1 is unchanged (git blob 3f2a1b9). A changed script asks again; revoke the rule in Project \
     settings › Auto mode. Approves this one too."
  );
  assert!(!locked(&mut window, &project));
  // An MCP client's press does not make the rule, even with both opt-ins on.
  let workspace = window.launch.workspace().expect("the session started");
  workspace
    .set_automation_answers_approvals(true)
    .expect("the approvals opt-in");
  workspace
    .set_automation_changes_permissions(true)
    .expect("the permissions opt-in");
  click(&mut window, &project);
  window.until("the question", |window| window.texts(&question).is_some());
  assert_eq!(
    text(&mut window, &format!("{question}-title")),
    "Approve start_qa_app for every run of this project?"
  );
  let confirm = format!("{question}-confirm");
  // The question takes no press before it lived through one pass.
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
  }
  // Below the window's fold: pressed where it is, as an MCP client's `lurq_interact` does.
  lurq::app::events::with_synthetic_input(|| {
    window
      .tree
      .get_element_by_id_mut(&confirm)
      .expect("the question's confirm")
      .click();
    window.pass();
  });
  let refused = format!("{card}-refused");
  window.until("why the client's answer was not taken", |window| {
    window.texts(&refused).is_some()
  });
  assert_eq!(text(&mut window, &refused), WIDER_PERSON_ONLY);
}

#[test]
fn card_command_offers_no_approval_for_every_run_of_the_project() {
  let (mut window, card) = card_of("Add serde to the webhook payloads", "cards-approval-");
  window.until("the request's choices", |window| {
    window.tree.get_element_by_id(&format!("{card}-similar")).is_some()
  });
  assert!(window.tree.get_element_by_id(&format!("{card}-project")).is_none());
}
