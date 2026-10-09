//! The decision cards of a task's conversation, headless at 1440 × 900 on the QA seed's `cards` scenario: a
//! question its step waits on, questions with suggested answers (waiting, several answers, answered by the
//! person and by automation, ended before an answer), a command waiting for approval and the ways one ends
//! (approved once, denied, approved by automation, ended without an answer), and plans. The window's runner
//! program does not exist: an answer reaches no runner, and the card says why. Needs `git`.

#[allow(
  dead_code,
  reason = "the seed's report and its database path are the QA tool's; these tests read its tasks"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
#[path = "../support/mod.rs"]
mod support;
mod theme;

use kontur::{SeedRequest, Seeded, scenarios};
use lurq::{app::events::MouseButton, node::element::ElementRef};
use orchester_app::{
  routes::{ProjectPage, Route, TaskView},
  ui::icon::Icon,
};
use orchester_core::{project::TaskKey, time::Timestamp};
use support::{Data, Window};

/// A window on the `cards` scenario, seeded in the window's own data folder.
fn seeded() -> (Window, Seeded) {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join("repo-cards"),
    scenario: scenarios::named("cards").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let launch = data.launch();
  (Window::of(data, launch), seeded)
}

/// The conversation of the seeded task titled `title`, once a card with an id starting with `prefix` shows;
/// that card's id.
fn card_of(title: &str, prefix: &str) -> (Window, String) {
  let (mut window, seeded) = seeded();
  let task = seeded
    .tasks
    .iter()
    .find(|task| task.title == title)
    .unwrap_or_else(|| panic!("no seeded task {title}"));
  let key: TaskKey = task.key.parse().expect("a task key");
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::Task(key.clone(), TaskView::Conversation),
  ));
  window.until(&format!("a card {prefix}… of {key}"), |window| {
    cards(window, prefix).into_iter().any(|id| id.matches('-').count() >= 2)
  });
  let id = card_id(&mut window, prefix);
  (window, id)
}

/// The id of the one card whose id starts with `prefix`, without its parts' ids.
fn card_id(window: &mut Window, prefix: &str) -> String {
  let mut found: Vec<String> = cards(window, prefix);
  found.sort_by_key(String::len);
  found.into_iter().next().unwrap_or_else(|| panic!("no card {prefix}…"))
}

/// Every element id under the conversation that starts with `prefix`.
fn cards(window: &mut Window, prefix: &str) -> Vec<String> {
  let Some(tab) = window.tree.get_element_by_id("conversation") else {
    return Vec::new();
  };
  let mut found = Vec::new();
  walk(tab, &mut |element| {
    if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
      found.push(id.to_owned());
    }
  });
  found
}

fn walk<'a>(element: ElementRef<'a>, visit: &mut impl FnMut(ElementRef<'a>)) {
  visit(element);
  for child in element.children() {
    walk(child, visit);
  }
}

/// The texts inside `id` joined by spaces, icon glyphs left out.
fn text(window: &mut Window, id: &str) -> String {
  window.texts(id).unwrap_or_else(|| panic!("{id} is shown")).join(" ")
}

/// Every text inside `id`, icon glyphs included.
fn glyphs(window: &mut Window, id: &str) -> Vec<String> {
  let mut all = Vec::new();
  if let Some(element) = window.tree.get_element_by_id(id) {
    walk(element, &mut |element| {
      all.extend(element.text_content().map(str::to_owned));
    });
  }
  all
}

/// Clicks the middle of `id`.
fn click(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
  window.pass();
}

fn locked(window: &mut Window, id: &str) -> bool {
  glyphs(window, id).contains(&Icon::Lock.glyph().to_string())
}

#[test]
fn card_question_waits_and_sends_the_answer_to_the_composer() {
  let (mut window, card) = card_of("Sync contacts with HubSpot", "cards-question-");
  let shown = text(&mut window, &card);
  assert!(shown.contains("Builder asks you"), "{shown}");
  assert!(shown.contains("Waiting for your answer"), "{shown}");
  assert!(
    shown.contains("Should a contact that exists in both keep the HubSpot owner or ours?"),
    "{shown}"
  );
  let context = text(&mut window, &format!("{card}-context"));
  assert!(context.starts_with("Asked at "), "{context}");
  assert!(
    context.contains("Implement · Builder's active time is stopped until you answer."),
    "{context}"
  );
  // This build has no composer yet: Answer says so instead of doing nothing.
  assert!(locked(&mut window, &format!("{card}-answer")));
  assert!(text(&mut window, &format!("{card}-answer-reason")).contains("the composer comes with a later build"));
}

#[test]
fn card_question_with_options_waits_until_answered_and_says_why_it_did_not_go() {
  let (mut window, card) = card_of("Pick where the docs button goes", "cards-choice-");
  let shown = text(&mut window, &card);
  for words in [
    "Builder asks you",
    "Waiting for your answer",
    "Where should the docs button go?",
    "Builder's options",
    "In the top bar",
    "Beside the account menu, where people look for help.",
    "In the sidebar footer",
    "In the command palette only",
  ] {
    assert!(shown.contains(words), "{words:?} in {shown}");
  }
  assert!(
    window.tree.get_element_by_id(&format!("{card}-own-0")).is_some(),
    "own answer field"
  );
  let send = format!("{card}-send");
  click(&mut window, &send);
  assert!(
    window.tree.get_element_by_id(&format!("{card}-refused")).is_none(),
    "Send answer waits for an answer"
  );
  click(&mut window, &format!("{card}-option-0-1"));
  click(&mut window, &send);
  window.until("why the answer did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
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
fn card_question_with_options_answered_says_what_by_whom_and_when() {
  let (mut window, card) = card_of("Name the retry setting", "cards-choice-");
  let shown = text(&mut window, &card);
  assert!(shown.contains("Answered · "), "{shown}");
  assert!(text(&mut window, &format!("{card}-answer-0-0")).contains("In the top bar"));
  let context = text(&mut window, &format!("{card}-context"));
  assert!(context.contains("answered by you at"), "{context}");
  assert!(window.tree.get_element_by_id(&format!("{card}-send")).is_none());

  let (mut window, card) = card_of("Pick the cache store", "cards-choice-");
  let context = text(&mut window, &format!("{card}-context"));
  assert!(
    context.contains("answered by automation through Orchester's MCP server at"),
    "{context}"
  );

  let (mut window, card) = card_of("Pick the log level", "cards-choice-");
  let shown = text(&mut window, &card);
  assert!(shown.contains("Ended without an answer"), "{shown}");
  assert!(text(&mut window, &format!("{card}-context")).contains("Builder stopped waiting at"));
}

#[test]
fn card_plan_shows_its_first_lines_the_whole_plan_and_approve() {
  let (mut window, card) = card_of("Add retry to webhook sender", "cards-plan-");
  let shown = text(&mut window, &card);
  assert!(shown.contains("Plan v1 · Add retry to webhook sender"), "{shown}");
  assert!(shown.contains("Waiting for approval"), "{shown}");
  assert!(
    !shown.contains("Document the setting."),
    "the first lines only: {shown}"
  );
  let whole = format!("{card}-whole");
  assert_eq!(text(&mut window, &whole), "Show the whole plan · 9 lines");
  click(&mut window, &whole);
  window.until("the whole plan", |window| {
    window
      .texts(&card)
      .is_some_and(|texts| texts.join(" ").contains("Document the setting."))
  });
  assert!(window.tree.get_element_by_id(&whole).is_none());
  let hint = text(&mut window, &format!("{card}-hint"));
  assert_eq!(
    hint,
    "Starts Implement with Builder. Asking for changes is not available here yet."
  );
  assert_eq!(text(&mut window, &format!("{card}-approve")), "Approve plan & continue");
  click(&mut window, &format!("{card}-approve"));
  window.until("why the approval did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
}

#[test]
fn card_plan_versions_say_which_waits_and_what_was_asked() {
  let (mut window, _) = card_of("Back off when GitHub rate-limits the sync", "cards-plan-");
  window.until("both versions", |window| cards(window, "cards-plan-plan-").len() >= 2);
  let first = text(&mut window, "cards-plan-plan-1");
  assert!(first.contains("Changes requested"), "{first}");
  assert!(
    first.contains("You asked: Don't drop messages while we wait: queue them and show how many wait."),
    "{first}"
  );
  let second = text(&mut window, "cards-plan-plan-2");
  assert!(second.contains("Waiting for approval"), "{second}");
}

#[test]
fn card_approval_shows_the_request_whole_and_every_answer() {
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
  assert!(agent.starts_with("Builder · Implement · Run "), "{agent}");
  assert!(text(&mut window, &format!("{card}-note")).contains("Adds the dependency"));
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
  assert!(similar.starts_with("Approve similar in this run"), "{similar}");
  let everything = text(&mut window, &format!("{card}-everything"));
  assert!(everything.starts_with("Approve everything in this run"), "{everything}");
  assert!(locked(&mut window, &format!("{card}-always")));
  assert_eq!(
    text(&mut window, &format!("{card}-always")),
    "Always allow… Not available yet: adding a command to a profile from an approval is planned."
  );
  let automation = text(&mut window, &format!("{card}-automation"));
  assert!(
    automation.starts_with("Automation through Orchester's MCP server may"),
    "{automation}"
  );
}

#[test]
fn card_approval_answer_without_a_runner_says_why() {
  let (mut window, card) = card_of("Add serde to the webhook payloads", "cards-approval-");
  click(&mut window, &format!("{card}-approve"));
  window.until("why the answer did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
  assert!(
    window.tree.get_element_by_id(&format!("{card}-approve")).is_some(),
    "the request still waits"
  );
}

#[test]
fn card_approval_decided_reads_how_by_whom_and_when() {
  for (title, decided, asked) in [
    ("Add rand to the jitter", "Approved once · by you at ", "cargo add rand"),
    (
      "Publish the webhook crate",
      "Denied · by you at ",
      "cargo publish --dry-run",
    ),
    (
      "Fetch the latest schema",
      "Approved once · by automation at ",
      "git fetch origin",
    ),
    (
      "Push the release tag",
      "Ended without an answer · Builder stopped waiting at ",
      "git push origin v1.2.0",
    ),
  ] {
    let (mut window, card) = card_of(title, "cards-approval-");
    let said = text(&mut window, &format!("{card}-decided"));
    assert!(said.starts_with(decided), "{title}: {said}");
    assert_eq!(text(&mut window, &format!("{card}-asked")), asked, "{title}");
    assert!(
      window.tree.get_element_by_id(&format!("{card}-approve")).is_none(),
      "{title}"
    );
  }
}
