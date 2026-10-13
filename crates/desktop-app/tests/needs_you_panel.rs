//! The Needs you panel the title bar's pill opens (layer `Needs you` `eBhFg`), headless at 1440 × 900 on the
//! QA seed's scenarios: what waits, grouped by project and oldest first, with the same count as the pill; a
//! plan that opens its task and closes the panel; an approval and a question with suggested answers answered in
//! place through the conversation's own cards, with the same rules (no runner here, so an answer says why it did
//! not go); a wider answer that synthetic input does not take; Esc and the close. Needs `git`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed reports what it wrote; these tests read the window instead"
)]
mod kontur;
mod support;

use kontur::{SeedRequest, Seeded, scenarios};
use lurq::{
  app::events::{MouseButton, with_synthetic_input},
  node::element::ElementRef,
};
use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::time::Timestamp;
use support::{Data, Window};

/// The window on `scenario`, landed on its project once its tasks are read.
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
  let mut window = Window::of(data, launch);
  window.until("the project's tasks to be read", |window| {
    window.texts("needs-you-pill").is_some()
  });
  (window, seeded)
}

/// Scrolls `id` into view, as a person would to reach it, and clicks its middle.
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
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
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
  assert!(text(&mut window, &format!("{merge}-title")).ends_with("passed every step; its work waits to be merged"));
  assert_eq!(text(&mut window, &format!("{merge}-action")), "Review & merge");
  let limit = item_of(&mut window, "Explain an expired GitHub token in the status bar");
  assert!(text(&mut window, &format!("{limit}-title")).ends_with("stopped at its limit"));
  assert_eq!(text(&mut window, &format!("{limit}-action")), "Raise the limit");
  assert_eq!(text(&mut window, "needs-you-show-all"), "Show all in Kontur's Overview");
}

#[test]
fn needs_you_panel_opens_a_plan_on_its_task_and_closes() {
  let (mut window, seeded) = seeded("tasks");
  open(&mut window);
  let plan = item_of(&mut window, "Plan the GitHub app permissions");
  assert_eq!(text(&mut window, &format!("{plan}-kind")), "Plan review");
  assert_eq!(text(&mut window, &format!("{plan}-action")), "Review plan");
  click(&mut window, &format!("{plan}-action"));
  let key = seeded
    .tasks
    .iter()
    .find(|task| task.title == "Plan the GitHub app permissions")
    .map(|task| task.key.parse().expect("a task key"))
    .expect("the seeded task");
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary)))
  );
  window.until("the panel to close", |window| window.texts("needs-you-panel").is_none());
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
    window.link.input_gate().protecting(),
    "while the question shows, MCP clients' window input is refused"
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
  assert!(
    !window.link.input_gate().protecting(),
    "the lock lifts with the question"
  );
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
