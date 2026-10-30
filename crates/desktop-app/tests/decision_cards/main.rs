//! The decision cards of a task's conversation, headless at 1440 × 900 on the QA seed's `cards` scenario: a
//! question its step waits on, questions with suggested answers (waiting, several answers, answered by the
//! person and by automation, ended before an answer), a command waiting for approval and the ways one ends
//! (approved once, denied, approved by automation, ended without an answer), and plans; on `cards-auto-mode`, a
//! command as auto mode's review leaves it ([`auto_mode`]); on `cards-approval-modes`, requests as the run's approval
//! mode answered them ([`approval_modes`]). The window's runner program does not exist: an answer
//! reaches no runner, and the card says why ([`answering`]); a run recorded at work is interrupted, so what its
//! agent asked in its turn no longer waits, unless the seed holds its step waiting for the person. Needs `git`.

mod answering;
mod approval_modes;
mod auto_mode;
mod deny_message;
#[allow(
  dead_code,
  reason = "the seed's report and its database path are the QA tool's; these tests read its tasks"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
mod project;
#[path = "../support/mod.rs"]
mod support;
mod theme;

use kontur::{SeedRequest, Seeded, scenarios};
use lurq::{app::events::MouseButton, core::ElementRect, node::element::ElementRef};
use orchester_app::{
  routes::{ProjectPage, Route, TaskView},
  ui::icon::Icon,
};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};
use support::{Data, Window};

/// A window on `scenario`, seeded in the window's own data folder.
fn seeded(scenario: &str) -> (Window, Seeded) {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join("repo-cards"),
    scenario: scenarios::named(scenario).expect("a known scenario"),
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
  let (window, id, _) = opened(title, prefix);
  (window, id)
}

/// [`card_of`], with the seeded project and the task's key.
fn opened(title: &str, prefix: &str) -> (Window, String, (ProjectId, TaskKey)) {
  opened_in("cards", title, prefix)
}

/// [`opened`] on `scenario`.
fn opened_in(scenario: &str, title: &str, prefix: &str) -> (Window, String, (ProjectId, TaskKey)) {
  let (mut window, seeded) = seeded(scenario);
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
  (window, id, (seeded.project, key))
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
  let bounds = laid_out(window, id);
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
  window.pass();
}

/// Where `id` is, once a layout pass placed it. The chat's list builds a row again after the pass that measured
/// it, so a part can stand in the tree a frame before any layout holds it; a person's press comes after that frame.
fn laid_out(window: &mut Window, id: &str) -> ElementRect {
  window.until(&format!("{id} laid out"), |window| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .is_some()
  });
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

/// The id of the card of plan version `version`: `cards-plan-<execution>-<version>`, not one of its parts.
fn version(window: &mut Window, version: &str) -> Option<String> {
  cards(window, "cards-plan-")
    .into_iter()
    .find(|id| id.rsplit('-').next() == Some(version) && id.matches('-').count() == 7)
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
  // The person left the question's mode for an ordinary message: Answer brings it back, focus in the box.
  click(&mut window, "composer-leave");
  window.until("an ordinary message", |window| {
    window.tree.get_element_by_id("composer-context").is_none()
  });
  click(&mut window, &format!("{card}-answer"));
  window.until("the question's mode", |window| {
    window
      .texts("composer-context-text")
      .is_some_and(|texts| texts.join(" ").starts_with("Answering Builder’s question from "))
  });
  window.until("the focus in the composer", |window| {
    window
      .tree
      .focused_element()
      .and_then(|element| element.id().map(str::to_owned))
      .as_deref()
      == Some("composer-input")
  });
}

/// The chat follows its end (`ui::chat::follow`), but until the person scrolls it opens on the card that waits for
/// them, its top in view.
#[test]
fn card_that_waits_opens_the_chat_on_it() {
  let (mut window, card) = card_of("Sync contacts with HubSpot", "cards-question-");
  for _ in 0..10 {
    window.tree.tick_futures();
    window.pass();
  }
  let (chat, shown) = (laid_out(&mut window, "conversation-chat"), laid_out(&mut window, &card));
  assert!(
    shown.y >= chat.y - 0.5 && shown.y < chat.y + chat.height,
    "the waiting card's top is in view: {shown:?} in {chat:?}"
  );
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_none(),
    "the chat still follows"
  );
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
    "Starts 03 Implement with Builder. To ask for changes, write in the composer."
  );
  assert_eq!(text(&mut window, &format!("{card}-approve")), "Approve plan & continue");
  click(&mut window, &format!("{card}-approve"));
  window.until("why the approval did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
  // The hint leads to the composer, in the plan's mode, after the person left it.
  click(&mut window, "composer-leave");
  window.until("an ordinary message", |window| {
    window.tree.get_element_by_id("composer-context").is_none()
  });
  click(&mut window, &format!("{card}-changes"));
  window.until("the plan's mode", |window| {
    window
      .texts("composer-context-text")
      .is_some_and(|texts| texts.join(" ").starts_with("Changes to plan v1"))
  });
}

#[test]
fn card_plan_versions_say_which_waits_and_what_was_asked() {
  let (mut window, _) = card_of("Back off when GitHub rate-limits the sync", "cards-plan-");
  window.until("both versions", |window| {
    version(window, "1").is_some() && version(window, "2").is_some()
  });
  let first = version(&mut window, "1").expect("plan v1");
  let first = text(&mut window, &first);
  assert!(first.contains("Changes requested"), "{first}");
  assert!(
    first.contains("You asked: Don't drop messages while we wait: queue them and show how many wait."),
    "{first}"
  );
  let second = version(&mut window, "2").expect("plan v2");
  let second = text(&mut window, &second);
  assert!(second.contains("Waiting for approval"), "{second}");
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

#[test]
fn card_requests_of_a_run_that_lost_its_runner_no_longer_wait() {
  // The window has no runner: a run recorded at work is interrupted, and so is what its agent asked in its
  // turn. Nothing reads as waiting or can be answered.
  for (title, prefix, gone) in [
    ("Pick the retry header", "cards-choice-", "-send"),
    ("Add jitter to the retries", "cards-approval-", "-approve"),
  ] {
    let (mut window, card) = card_of(title, prefix);
    let shown = text(&mut window, &card);
    assert!(shown.contains("Interrupted · "), "{title}: {shown}");
    assert!(!shown.contains("Waiting for"), "{title}: {shown}");
    assert!(
      window.tree.get_element_by_id(&format!("{card}{gone}")).is_none(),
      "{title} cannot be answered"
    );
  }
  let (mut window, card) = card_of("Pick the retry header", "cards-choice-");
  let context = text(&mut window, &format!("{card}-context"));
  assert!(
    context.contains("the run was interrupted before an answer reached Builder"),
    "{context}"
  );
  let (mut window, card) = card_of("Add jitter to the retries", "cards-approval-");
  assert_eq!(
    text(&mut window, &format!("{card}-decided")),
    "Interrupted · Builder no longer waits for an answer"
  );
  assert_eq!(text(&mut window, &format!("{card}-asked")), "cargo add rand");
}
