//! The Needs you panel the title bar's pill opens (layer `Needs you` `eBhFg`), headless at 1440 × 900 on the
//! QA seed's scenarios: what waits, grouped by project and oldest first, with the same count as the pill; a
//! plan that opens its task and closes the panel; an approval and a question with suggested answers answered in
//! place through the conversation's own cards, with the same rules (no runner here, so an answer says why it did
//! not go); a wider answer that synthetic input does not take, and an MCP client's answers ([`automation`]); Esc
//! and the close, and the pill opening it again after each way it closes ([`closing`]); an expanded approval's
//! layout and the pinned project header ([`expanded`]); a question's panel layout, the notes of what was answered
//! while the panel is open and an expanded item kept in view ([`in_place`]); an item whose request cannot be read
//! again ([`reread`]); the same count wherever Needs you shows ([`counts`]); two projects that wait, each tab's
//! count and each project's item in its own tab ([`tabs`]). Needs `git`.

mod automation;
mod closing;
mod counts;
mod expanded;
mod in_place;
#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed reports what it wrote; these tests read the window instead"
)]
mod kontur;
mod reread;
#[path = "../support/mod.rs"]
mod support;
mod tabs;

use std::time::{Duration, Instant};

use kontur::{SeedRequest, Seeded, scenarios};
use lurq::{
  app::events::{MouseButton, with_synthetic_input},
  node::element::ElementRef,
};
use orchester_app::routes::{MemoryTab, ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{
  id::ResourceId,
  memory::{
    AgentProvenance, AgentWrite, Confidence, Memory, MemoryContent, MemoryKind, MemoryScope, MemorySettings,
    MemoryStore, SecretScreen,
  },
  resource::StepKey,
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use support::{Data, Window};

/// The window on `scenario`, landed on its project once its tasks are read.
fn seeded(scenario: &str) -> (Window, Seeded) {
  seeded_with(scenario, |_| {})
}

/// The window on `scenario` with what `more` adds to it before the app starts.
fn seeded_with(scenario: &str, more: impl FnOnce(&Seeded)) -> (Window, Seeded) {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join(format!("repo-{scenario}")),
    scenario: scenarios::named(scenario).expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  more(&seeded);
  let launch = data.launch();
  let mut window = Window::of(data, launch);
  window.until("the project's tasks to be read", |window| {
    window.texts("needs-you-pill").is_some()
  });
  (window, seeded)
}

/// Scrolls `id` into view, as a person would to reach it, and clicks its middle. Bounds are logical, the
/// pointer's position in the window's pixels.
fn click(window: &mut Window, id: &str) {
  if let Some(mut element) = window.tree.get_element_by_id_mut(id) {
    element.scroll_into_view();
  }
  window.pass();
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let scale = window.tree.scale_factor();
  let (x, y) = (
    (bounds.x + bounds.width / 2.0) * scale,
    (bounds.y + bounds.height / 2.0) * scale,
  );
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
  window.pass();
}

/// Opens the panel from the pill.
fn open(window: &mut Window) {
  click(window, "needs-you-pill");
  window.until("the panel", |window| window.texts("needs-you-panel").is_some());
}

fn walk<'a>(element: ElementRef<'a>, visit: &mut impl FnMut(ElementRef<'a>)) {
  visit(element);
  for child in element.children() {
    walk(child, visit);
  }
}

/// The panel's items, in the list's order: each item's element id.
fn items(window: &mut Window) -> Vec<String> {
  let Some(panel) = window.tree.get_element_by_id("needs-you-panel") else {
    return Vec::new();
  };
  let mut ids = Vec::new();
  walk(panel, &mut |element| {
    if let Some(id) = element.id().filter(|id| id.starts_with("needs-you-item-")) {
      ids.push(id.to_owned());
    }
  });
  let all = ids.clone();
  ids.retain(|id| all.contains(&format!("{id}-task")));
  ids
}

fn text(window: &mut Window, id: &str) -> String {
  window.texts(id).unwrap_or_else(|| panic!("{id} is shown")).join(" ")
}

/// The item whose task is titled `title`.
fn item_of(window: &mut Window, title: &str) -> String {
  items(window)
    .into_iter()
    .find(|id| {
      window
        .texts(&format!("{id}-task"))
        .is_some_and(|texts| texts.join(" ").ends_with(title))
    })
    .unwrap_or_else(|| panic!("an item of {title}"))
}

/// Every element id under `id` that starts with `prefix`.
fn ids_under(window: &mut Window, id: &str, prefix: &str) -> Vec<String> {
  let Some(root) = window.tree.get_element_by_id(id) else {
    return Vec::new();
  };
  let mut found = Vec::new();
  walk(root, &mut |element| {
    if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
      found.push(id.to_owned());
    }
  });
  found
}

#[test]
fn needs_you_panel_lists_what_waits_by_project_oldest_first() {
  let (mut window, seeded) = seeded("tasks");
  open(&mut window);
  assert_eq!(text(&mut window, "needs-you-count"), "6", "as the pill counts");
  assert_eq!(window.texts("needs-you-pill"), Some(vec!["6 need you".to_owned()]));
  assert_eq!(text(&mut window, "needs-you-sub"), "Kontur · oldest first");
  let group = format!("needs-you-group-{}", seeded.project);
  assert_eq!(text(&mut window, &group), "K Kontur this tab's project 6");
  let items = items(&mut window);
  assert_eq!(items.len(), 6, "{items:?}");
  let kinds: Vec<String> = items
    .iter()
    .map(|id| text(&mut window, &format!("{id}-kind")))
    .collect();
  let mut sorted = kinds.clone();
  sorted.sort();
  assert_eq!(
    sorted,
    [
      "Approval",
      "Decision",
      "Plan review",
      "Question",
      "Ready to merge",
      "Stopped at limit"
    ],
    "{kinds:?}"
  );
  let approval = item_of(&mut window, "Add retry to webhook sender");
  assert_eq!(
    text(&mut window, &format!("{approval}-title")),
    "Builder asks to run cargo add rand"
  );
  assert!(text(&mut window, &format!("{approval}-task")).starts_with("KON-12 · "));
  assert!(text(&mut window, &format!("{approval}-waiting")).starts_with("waiting "));
  assert!(text(&mut window, &format!("{approval}-where")).starts_with("Run "));
  assert_eq!(text(&mut window, &format!("{approval}-action")), "Review");
  let merge = item_of(&mut window, "Show the signed-in account");
  assert!(
    text(&mut window, &format!("{merge}-title")).ends_with("is ready to merge: every step passed"),
    "worded as the Overview's card"
  );
  assert_eq!(text(&mut window, &format!("{merge}-action")), "Review & merge");
  let limit = item_of(&mut window, "Explain an expired GitHub token in the status bar");
  assert!(
    text(&mut window, &format!("{limit}-title")).ends_with("-token limit"),
    "with the limit it stopped at, as the Overview's card says it"
  );
  assert_eq!(text(&mut window, &format!("{limit}-action")), "Raise the limit");
  assert_eq!(text(&mut window, "needs-you-show-all"), "Show all in Kontur's Overview");
}

/// The element `id` is ringed as the card a navigation pointed at.
fn pointed(window: &mut Window, id: &str) -> bool {
  window
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| element.has_class("pointed"))
}

/// The Summary route of the seeded task titled `title`.
fn summary_of(seeded: &Seeded, title: &str) -> Route {
  let key = seeded
    .tasks
    .iter()
    .find(|task| task.title == title)
    .map(|task| task.key.parse().expect("a task key"))
    .expect("the seeded task");
  Route::Project(seeded.project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary)))
}

#[test]
fn needs_you_panel_opens_a_plan_on_its_task_and_closes() {
  let (mut window, seeded) = seeded("tasks");
  open(&mut window);
  let plan = item_of(&mut window, "Plan the GitHub app permissions");
  assert_eq!(text(&mut window, &format!("{plan}-kind")), "Plan review");
  assert_eq!(text(&mut window, &format!("{plan}-action")), "Review plan");
  click(&mut window, &format!("{plan}-action"));
  assert_eq!(
    window.navigation().route_now(),
    summary_of(&seeded, "Plan the GitHub app permissions")
  );
  window.until("the panel to close", |window| window.texts("needs-you-panel").is_none());
  window.until("the plan's card ringed", |window| pointed(window, "plan-review"));
  window
    .tree
    .tick_timers_at(Instant::now() + Duration::from_millis(2_050));
  window.pass();
  assert!(!pointed(&mut window, "plan-review"), "the ring fades after its moment");
  assert!(window.tree.get_element_by_id("plan-review").is_some());
}

#[test]
fn needs_you_panel_opens_a_limit_at_its_card_with_the_new_limit_focused() {
  let (mut window, seeded) = seeded("tasks");
  open(&mut window);
  let title = "Explain an expired GitHub token in the status bar";
  let limit = item_of(&mut window, title);
  click(&mut window, &format!("{limit}-action"));
  assert_eq!(window.navigation().route_now(), summary_of(&seeded, title));
  window.until("the limit's card ringed", |window| pointed(window, "raise-limit"));
  window.until("the new limit focused", |window| {
    window.tree.focused_element().and_then(|element| element.id()) == Some("raise-limit-input")
  });
  // The pointer was taken: going to the same page again by other means rings nothing.
  window
    .navigation()
    .go(Route::Project(seeded.project, ProjectPage::Tasks(Default::default())));
  window.pass();
  window.navigation().go(summary_of(&seeded, title));
  window.until("the card again", |window| {
    window.tree.get_element_by_id("raise-limit").is_some()
  });
  assert!(!pointed(&mut window, "raise-limit"));
}

#[test]
fn needs_you_panel_keeps_under_the_pill_on_a_scaled_window() {
  let (mut window, _) = seeded("tasks");
  // 1440 × 900 at 150 %: the panel's room is the window's logical height, not its pixels.
  window.tree.set_scale_factor(1.5);
  window.tree.resize(2160, 1350);
  window.pass();
  open(&mut window);
  let (item, _) = expand_approval(&mut window, "Add retry to webhook sender");
  settle(&mut window);
  let bounds = |window: &mut Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let pill = bounds(&mut window, "needs-you-pill");
  let panel = bounds(&mut window, "needs-you-panel");
  assert!(
    panel.y >= pill.y + pill.height,
    "the panel opens under the pill: {panel:?} under {pill:?}"
  );
  assert!(
    panel.y + panel.height <= 900.0 - 32.0 + 0.5,
    "and ends above the status bar's room: {panel:?}"
  );
  assert!(window.tree.get_element_by_id(&item).is_some());
}

#[test]
fn needs_you_panel_closes_with_its_close_and_with_the_pill() {
  let (mut window, _) = seeded("tasks");
  open(&mut window);
  click(&mut window, "needs-you-close");
  window.until("the panel to close", |window| window.texts("needs-you-panel").is_none());
  open(&mut window);
  click(&mut window, "needs-you-pill");
  window.until("the panel to close again", |window| {
    window.texts("needs-you-panel").is_none()
  });
}

/// Two records an agent of the project's first run wrote to its memory, which wait for approval.
fn propose_memory(seeded: &Seeded) {
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's database");
  let run = db
    .tasks()
    .expect("the project's tasks")
    .iter()
    .flat_map(|task| db.runs_of_task(task.id()).expect("the task's runs"))
    .find_map(|run| run.ok().filter(|run| run.current_attempt().is_some()))
    .expect("a run");
  let records: Vec<Memory> = ["Webhooks retry three times.", "Docs live in docs/."]
    .into_iter()
    .map(|text| {
      let write = AgentWrite {
        provenance: AgentProvenance {
          profile: ResourceId::generate(),
          run: run.id(),
          step: StepKey::new("implement").expect("a step key"),
          execution: run.current_attempt().expect("an attempt").executions()[0].id(),
        },
        confidence: Confidence::Medium,
        sources: Vec::new(),
      };
      let content = MemoryContent::new(MemoryKind::Fact, text).expect("content");
      let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
      Memory::by_agent(
        MemoryScope::Project,
        write,
        screened,
        &MemorySettings::default(),
        Timestamp::now(),
      )
    })
    .collect();
  db.add_memories(&records).expect("the records wait for approval");
}

#[test]
fn needs_you_panel_opens_the_memory_page_on_the_records_waiting_for_approval() {
  let (mut window, seeded) = seeded_with("tasks", propose_memory);
  open(&mut window);
  assert_eq!(text(&mut window, "needs-you-count"), "7", "the records are one item");
  let item = format!("needs-you-item-memory-{}", seeded.project);
  assert_eq!(text(&mut window, &format!("{item}-kind")), "Memory");
  assert_eq!(
    text(&mut window, &format!("{item}-title")),
    "2 memory records wait for approval",
    "worded as the Overview's card"
  );
  assert_eq!(text(&mut window, &format!("{item}-task")), "Project memory");
  assert_eq!(text(&mut window, &format!("{item}-action")), "Review records");
  assert!(
    window.tree.get_element_by_id(&format!("{item}-reason")).is_none(),
    "the review is built: no reason why not"
  );
  click(&mut window, &format!("{item}-action"));
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::Memory(Some(MemoryTab::Waiting)))
  );
  window.until("the panel to close", |window| window.texts("needs-you-panel").is_none());
  window.until("the records waiting for approval", |window| {
    window.texts("memory-queue").is_some()
  });
  assert_eq!(text(&mut window, "memory-tab-waiting-badge"), "2");
  let queue = window.texts("memory-queue").expect("the queue");
  for record in ["Webhooks retry three times.", "Docs live in docs/."] {
    assert!(queue.iter().any(|said| said == record), "{record:?} in {queue:?}");
  }
}

#[test]
fn needs_you_panel_answers_an_approval_in_place_through_its_card() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let item = item_of(&mut window, "Add serde to the webhook payloads");
  click(&mut window, &format!("{item}-action"));
  window.until("the card in place", |window| {
    !ids_under(window, &item, "cards-approval-").is_empty()
  });
  let mut cards = ids_under(&mut window, &item, "cards-approval-");
  cards.sort_by_key(String::len);
  let card = cards[0].clone();
  window.until("the request's choices", |window| {
    window.tree.get_element_by_id(&format!("{card}-similar")).is_some()
  });
  assert!(
    window.tree.get_element_by_id(&format!("{card}-title")).is_none(),
    "the item says what it is: the card leaves out its head"
  );
  assert!(text(&mut window, &format!("{item}-waiting")).starts_with("waiting since "));
  assert_eq!(
    text(&mut window, &format!("{card}-value")),
    "cargo add serde --features derive"
  );
  assert_eq!(text(&mut window, &format!("{card}-approve")), "Approve once");
  click(&mut window, &format!("{card}-approve"));
  // No runner here: the answer reaches none, and the card says why, as in the conversation.
  window.until("why the answer did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
  click(&mut window, &format!("{item}-collapse"));
  window.until("the item collapsed", |window| {
    ids_under(window, &item, "cards-approval-").is_empty()
  });
}

/// Expands the item of the task titled `title` around its approval card: the item and the card's id.
fn expand_approval(window: &mut Window, title: &str) -> (String, String) {
  let item = item_of(window, title);
  click(window, &format!("{item}-action"));
  window.until("the card in place", |window| {
    !ids_under(window, &item, "cards-approval-").is_empty()
  });
  let mut cards = ids_under(window, &item, "cards-approval-");
  cards.sort_by_key(String::len);
  let card = cards[0].clone();
  (item, card)
}

/// Lets a question that just showed live through its first passes, as a person's eye would.
fn settle(window: &mut Window) {
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
  }
}

#[test]
fn needs_you_panel_asks_a_person_to_confirm_a_wider_answer() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let (_, card) = expand_approval(&mut window, "Add serde to the webhook payloads");
  let (similar, question, refused) = (
    format!("{card}-similar"),
    format!("{card}-widen"),
    format!("{card}-refused"),
  );
  window.until("Approve similar", |window| {
    window.tree.get_element_by_id(&similar).is_some()
  });
  assert!(!window.link.input_gate().protecting());
  click(&mut window, &similar);
  window.until("the question in place", |window| {
    window.tree.get_element_by_id(&format!("{question}-confirm")).is_some()
  });
  let run = run_of(&mut window, &card);
  assert_eq!(
    text(&mut window, &format!("{question}-title")),
    format!("Approve similar requests until Run {run} ends?")
  );
  assert!(
    !window.link.input_gate().protecting(),
    "the question locks no window input: an MCP client may still look, move and take it back"
  );
  settle(&mut window);
  assert!(
    window.tree.get_element_by_id(&refused).is_none(),
    "choosing a wider answer sends nothing before it is confirmed"
  );
  with_synthetic_input(|| click(&mut window, &format!("{question}-confirm")));
  settle(&mut window);
  assert!(
    window.tree.get_element_by_id(&refused).is_none(),
    "an MCP client's confirm is not taken without the person's consent"
  );
  click(&mut window, &format!("{question}-keep"));
  window.until("the question taken back", |window| {
    window.tree.get_element_by_id(&question).is_none()
  });
  assert!(window.tree.get_element_by_id(&refused).is_none());
  click(&mut window, &similar);
  window.until("the question again", |window| {
    window.tree.get_element_by_id(&format!("{question}-confirm")).is_some()
  });
  settle(&mut window);
  click(&mut window, &format!("{question}-confirm"));
  // No runner here: the confirmed answer reaches none, and the card says why, as in the conversation.
  window.until("a person's confirm to go to the runner", |window| {
    window.tree.get_element_by_id(&refused).is_some()
  });
  assert!(window.tree.get_element_by_id(&question).is_none());
  assert!(!window.link.input_gate().protecting());
}

/// The number of the run `card` belongs to, as its request says it.
fn run_of(window: &mut Window, card: &str) -> String {
  let covers = text(window, &format!("{card}-similar-covers"));
  let after = covers.split("Run ").nth(1).expect("what it covers names the run");
  after.chars().take_while(char::is_ascii_digit).collect()
}

#[test]
fn needs_you_panel_shows_a_request_whose_run_lost_its_runner_as_the_conversation_does() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let (item, card) = expand_approval(&mut window, "Add jitter to the retries");
  window.until("the request's state", |window| {
    window.tree.get_element_by_id(&format!("{card}-decided")).is_some()
  });
  assert_eq!(
    text(&mut window, &format!("{card}-decided")),
    "Interrupted · Builder no longer waits for an answer"
  );
  assert!(
    window.tree.get_element_by_id(&format!("{card}-approve")).is_none(),
    "nothing answers a request no agent waits on"
  );
  click(&mut window, &format!("{item}-collapse"));
  let (_, waiting) = expand_approval(&mut window, "Add serde to the webhook payloads");
  window.until("a request its agent waits on to be answerable", |window| {
    window.tree.get_element_by_id(&format!("{waiting}-approve")).is_some()
  });
}

#[test]
fn needs_you_panel_answers_a_question_with_options_in_place() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let item = item_of(&mut window, "Pick where the docs button goes");
  assert_eq!(
    text(&mut window, &format!("{item}-title")),
    "Builder asks: Where should the docs button go?"
  );
  assert_eq!(text(&mut window, &format!("{item}-action")), "Answer");
  click(&mut window, &format!("{item}-action"));
  window.until("the card in place", |window| {
    !ids_under(window, &item, "cards-choice-").is_empty()
  });
  let mut cards = ids_under(&mut window, &item, "cards-choice-");
  cards.sort_by_key(String::len);
  let card = cards[0].clone();
  let shown = text(&mut window, &card);
  assert!(shown.contains("In the top bar"), "{shown}");
  click(&mut window, &format!("{card}-option-0-1"));
  click(&mut window, &format!("{card}-send"));
  window.until("why the answer did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
}
