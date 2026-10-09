//! A task's Conversation tab, headless at 1440 × 900 on the QA seed's projects: the `conversation`
//! scenario's long transcript (KON-12: chips and their summaries, a failed command, an open group's first
//! seven rows and *Show N more*, folded steps, a return, a result card, a problem row, a person's message,
//! following the end and *Jump to latest*) and `task-states` for what the tab shows instead of a timeline
//! ([`states`]). The window's runner program does not exist, so a run recorded as running reads as
//! interrupted: the working line and *Following live* need a live run (QA's `live` scenario). Needs `git`.

mod denied;
#[allow(
  dead_code,
  reason = "the seed's report and its database path are the QA tool's; these tests read its tasks"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
mod long;
mod states;
#[path = "../support/mod.rs"]
mod support;
mod theme;

use kontur::{SeedRequest, Seeded, scenarios};
use lurq::{
  app::events::{MouseButton, ScrollPhase},
  node::element::ElementRef,
};
use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::{project::TaskKey, time::Timestamp};
use support::{Data, Window};

/// A window on `scenario`, seeded in the window's own data folder.
fn seeded(scenario: &str) -> (Window, Seeded) {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join(format!("repo-{scenario}")),
    scenario: scenarios::named(scenario).expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let launch = data.launch();
  (Window::of(data, launch), seeded)
}

/// The key of the seeded task titled `title`.
fn key_of(seeded: &Seeded, title: &str) -> TaskKey {
  let task = seeded
    .tasks
    .iter()
    .find(|task| task.title == title)
    .unwrap_or_else(|| panic!("no seeded task {title}"));
  task.key.parse().expect("a task key")
}

/// Opens the Conversation tab of `key` and waits for `shown`, an element of the tab.
fn open(window: &mut Window, seeded: &Seeded, key: &TaskKey, shown: &str) {
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::Task(key.clone(), TaskView::Conversation),
  ));
  window.until(&format!("{shown} of {key}"), |window| {
    window.tree.get_element_by_id(shown).is_some()
  });
}

/// KON-12 of the `conversation` scenario, its chat shown.
fn retry_webhooks() -> (Window, Seeded) {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the timeline's rows", |window| {
    !ids(window, "conversation-row:").is_empty()
  });
  (window, seeded)
}

/// Every element id under the chat that starts with `prefix`.
fn ids(window: &mut Window, prefix: &str) -> Vec<String> {
  let Some(chat) = window.tree.get_element_by_id("conversation") else {
    return Vec::new();
  };
  let mut found = Vec::new();
  walk(chat, &mut |element| {
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

/// The texts of the whole tab, joined.
fn all_text(window: &mut Window) -> String {
  window.texts("conversation").unwrap_or_default().join("\n")
}

/// The id of the element under the chat with `prefix` whose texts contain `text`.
fn with_text(window: &mut Window, prefix: &str, text: &str) -> String {
  ids(window, prefix)
    .into_iter()
    .find(|id| window.texts(id).is_some_and(|texts| texts.join(" ").contains(text)))
    .unwrap_or_else(|| panic!("no {prefix}… holds {text:?}: {}", all_text(window)))
}

/// Clicks the middle of the element `id`.
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

/// Turns the wheel over the chat: up (`delta` above 0) scrolls toward its start.
fn wheel(window: &mut Window, delta: f32) {
  let bounds = window
    .tree
    .get_element_by_id_mut("conversation-chat")
    .and_then(|element| element.bounds())
    .expect("the chat is laid out");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.scroll(x, y, 0.0, delta, ScrollPhase::Scroll);
  window.pass();
  window.pass();
}

/// Scrolls the chat to its start, where the timeline begins.
fn to_start(window: &mut Window) {
  for _ in 0..10 {
    wheel(window, 5000.0);
  }
}

#[test]
fn timeline_chips_summarize_tool_groups_and_mark_a_failed_command() {
  let (mut window, _) = retry_webhooks();
  let text = all_text(&mut window);
  assert!(
    text.contains("Searched once · ran cargo test -p webhooks · edited 2 files"),
    "{text}"
  );
  let failed = with_text(&mut window, "conversation-group-", "ran cargo test -p webhooks ·");
  assert_eq!(
    window.texts("conversation-group-failure"),
    Some(vec!["exit 101".to_owned()]),
    "the first failed command's exit, in {failed}"
  );
  // No agent works (the window has no runner): the last command has no end, and never reads as running.
  assert!(text.contains("cargo test -p webhooks jitter without an end"), "{text}");
  assert!(!text.contains("running"), "{text}");
}

#[test]
fn timeline_an_open_group_lists_seven_rows_then_shows_more() {
  let (mut window, _) = retry_webhooks();
  let chip = with_text(&mut window, "conversation-group-tools-", "Read 12 files");
  let key = chip.trim_start_matches("conversation-group-");
  click(&mut window, &chip);
  let rows = format!("conversation-group-rows-{key}");
  window.until("the group's rows", |window| window.texts(&rows).is_some());
  let listed = ids(&mut window, "conversation-entry-")
    .into_iter()
    .filter(|id| id.ends_with("-title"))
    .count();
  assert_eq!(listed, 7, "an open group lists its first seven rows");
  let more = format!("conversation-group-more-{key}");
  let label = window.texts(&more).expect("Show N more").join(" ");
  assert!(label.starts_with("Show "), "{label}");
  assert!(label.contains("reads"), "what it leaves out, by kind: {label}");
  // The opened group stays where it was; its rows run on under the view.
  wheel(&mut window, -5000.0);
  click(&mut window, &more);
  window.until("every row", |window| window.texts(&more).is_none());
  let listed = ids(&mut window, "conversation-entry-")
    .into_iter()
    .filter(|id| id.ends_with("-title"))
    .count();
  assert!(
    listed > 7,
    "all of the group's rows once Show more was pressed: {listed}"
  );
  wheel(&mut window, -5000.0);
  // A command's row opens to its output: the group's last command, which the chat shows.
  let row = ids(&mut window, "conversation-entry-")
    .into_iter()
    .filter(|id| id.ends_with("-title"))
    .rfind(|id| {
      window
        .texts(id)
        .is_some_and(|texts| texts.concat().starts_with("sed -n"))
    })
    .expect("the group's last command");
  let row = row.trim_end_matches("-title").to_owned();
  click(&mut window, &format!("{row}-toggle"));
  window.until("the command's output", |window| {
    window.texts(&format!("{row}-output")).is_some()
  });
}

#[test]
fn timeline_steps_fold_returns_divide_and_results_close_a_step() {
  let (mut window, _) = retry_webhooks();
  to_start(&mut window);
  let text = all_text(&mut window);
  assert!(
    text.contains("01–04 · 4 steps done"),
    "finished steps in a row fold: {text}"
  );
  assert!(text.contains("Show them"), "{text}");
  assert!(
    text.contains("Review returned the work to 03 Implement · attempt 2"),
    "a return divides the attempts: {text}"
  );
  let result = with_text(&mut window, "conversation-result-", "Review failed");
  let card = window.texts(&result).expect("the result card").join(" ");
  assert!(card.contains("05 Review"), "{card}");
  assert!(card.contains("Changes ↗") && card.contains("Summary ↗"), "{card}");
  // Show them opens the folded steps, each with its divider.
  let fold = with_text(&mut window, "conversation-fold-", "4 steps done");
  click(&mut window, &format!("{fold}-link"));
  window.until("the folded steps' dividers", |window| {
    all_text(window).contains("01 Research")
  });
}

#[test]
fn timeline_problem_rows_stand_alone_and_show_what_happened() {
  let (mut window, _) = retry_webhooks();
  let problem = with_text(&mut window, "conversation-problem-", "The agent CLI reported an error");
  let texts = window.texts(&problem).expect("the problem row").join(" ");
  assert!(
    texts.contains("stream disconnected before completion; reconnecting (1/5)"),
    "what happened, never folded into a group: {texts}"
  );
}

#[test]
fn timeline_shows_the_persons_message_with_its_delivery_line() {
  let (mut window, _) = retry_webhooks();
  let message = with_text(
    &mut window,
    "conversation-message-",
    "Please keep the retry budget configurable",
  );
  let texts = window.texts(&message).expect("the message").join(" ");
  assert!(texts.contains("You"), "{texts}");
  assert!(texts.contains("Delivered mid-turn to Builder"), "{texts}");
}

#[test]
fn timeline_any_scroll_up_stops_following_and_jump_to_latest_counts_arrivals_only() {
  let (mut window, _) = retry_webhooks();
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_none(),
    "the chat starts at its end, following"
  );
  wheel(&mut window, 2000.0);
  window.until("Jump to latest", |window| {
    window.tree.get_element_by_id("conversation-jump").is_some()
  });
  assert_eq!(
    window.texts("conversation-jump"),
    Some(vec!["Jump to latest".to_owned()]),
    "no row arrived since it stopped following"
  );
  // Steps opened while scrolled up are no arrivals.
  to_start(&mut window);
  let fold = with_text(&mut window, "conversation-fold-", "4 steps done");
  click(&mut window, &format!("{fold}-link"));
  window.until("the folded steps' dividers", |window| {
    all_text(window).contains("01 Research")
  });
  assert_eq!(
    window.texts("conversation-jump"),
    Some(vec!["Jump to latest".to_owned()]),
    "rows the person opened are not new"
  );
  click(&mut window, "conversation-jump");
  window.until("following again", |window| {
    window.tree.get_element_by_id("conversation-jump").is_none()
  });
}

#[test]
fn timeline_a_touchpad_nudge_up_stops_following_and_stays() {
  let (mut window, _) = retry_webhooks();
  wheel(&mut window, 5.0);
  window.until("Jump to latest", |window| {
    window.tree.get_element_by_id("conversation-jump").is_some()
  });
  for _ in 0..5 {
    window.pass();
  }
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_some(),
    "the chat stays where the nudge left it"
  );
  assert!(
    window
      .tree
      .get_element_by_id("conversation-row:conversation-jump-room")
      .is_some(),
    "the last row can scroll clear of the floating pill"
  );
}

#[test]
fn timeline_a_command_without_an_end_is_not_one_the_group_ran() {
  let (mut window, _) = retry_webhooks();
  let text = all_text(&mut window);
  assert!(
    text.contains("Read 12 files · edited 2 files · cargo test -p webhooks jitter without an end"),
    "{text}"
  );
  assert!(!text.contains("ran cargo test -p webhooks jitter"), "{text}");
}

#[test]
fn timeline_the_persons_bubble_hugs_its_words() {
  let (mut window, _) = retry_webhooks();
  let message = with_text(
    &mut window,
    "conversation-message-",
    "Please keep the retry budget configurable",
  );
  let width = |window: &mut Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .map(|bounds| bounds.width)
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (bubble, row) = (
    width(&mut window, &format!("{message}-bubble")),
    width(&mut window, &message),
  );
  assert!(
    bubble < row * 0.6,
    "a short message's bubble is as wide as its words: {bubble} of {row}"
  );
}

#[test]
fn timeline_opening_a_group_while_following_keeps_it_in_place() {
  let (mut window, _) = retry_webhooks();
  let chip = with_text(&mut window, "conversation-group-tools-", "Read 12 files");
  let top = |window: &mut Window| {
    window
      .tree
      .get_element_by_id_mut(&chip)
      .and_then(|element| element.bounds())
      .map(|bounds| bounds.y)
      .expect("the chip is laid out")
  };
  let before = top(&mut window);
  click(&mut window, &chip);
  for _ in 0..5 {
    window.pass();
  }
  let after = top(&mut window);
  assert!(
    (after - before).abs() < 1.0,
    "the chip stays under the pointer: {before} → {after}"
  );
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_some(),
    "the chat no longer follows"
  );
  // The chip stays in view, and Jump to latest floats over the chat without covering it.
  let bounds = |window: &mut Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (chat, chip, jump) = (
    bounds(&mut window, "conversation-chat"),
    bounds(&mut window, &chip),
    bounds(&mut window, "conversation-jump"),
  );
  assert!(
    chip.y >= chat.y && chip.y + chip.height <= chat.y + chat.height,
    "the opened chip is in view: {chip:?} in {chat:?}"
  );
  let overlaps = chip.x < jump.x + jump.width
    && jump.x < chip.x + chip.width
    && chip.y < jump.y + jump.height
    && jump.y < chip.y + chip.height;
  assert!(
    !overlaps,
    "Jump to latest leaves the chip uncovered: {chip:?}, {jump:?}"
  );
  assert!(
    jump.y + jump.height <= chat.y + chat.height + 0.5,
    "Jump to latest floats inside the chat: {jump:?} in {chat:?}"
  );
}
