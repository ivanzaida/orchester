//! A link in a plan under review that says one address and opens another shows the host it opens, as the
//! Orchestrator's chat does, and a synthetic press on it opens nothing: the plan follows its links as the window does
//! (`ui::chat::links`).

use lurq::{
  app::events::{MouseButton, with_synthetic_input},
  layout::quad::QuadContent,
};
use orchester_core::{
  run::{Answerer, PlanDecision, ReviewNote},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::Page;

/// Says one address, opens another.
const LYING: &str = "1. Read the limits.\n2. Apply the fix from [github.com/acme/kontur](https://evil.example/login).";

/// The waiting plan of the task left in `state` is sent back, and its agent proposes `text` as the next one.
fn propose(page: &Page, state: &str, text: &str) {
  let (key, _) = page
    .tasks
    .iter()
    .find(|(_, seeded)| *seeded == state)
    .unwrap_or_else(|| panic!("a task {state}"));
  let mut db = ProjectDb::open(&page.repo).expect("the project's database");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == *key)
    .expect("the task");
  let listed = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .expect("a run");
  let mut run = db.run(listed.id()).expect("the run").expect("a stored run");
  let mut plan = db.plans(run.id()).expect("plans").pop().expect("a waiting plan");
  let note = ReviewNote::new("Link the fix.").expect("a note");
  let at = Timestamp::now();
  let events = run
    .decide_plan(&mut plan, PlanDecision::ChangesRequested { note }, Answerer::Person, at)
    .expect("changes requested");
  db.append_events_with_plan(&run, &events, &plan).expect("the decision");
  let earlier = db.plans(run.id()).expect("plans");
  let later = Timestamp::from_unix_millis(at.unix_millis() + 1_000);
  let (events, next) = run
    .propose_plan(plan.execution, text.to_owned(), &earlier, later)
    .expect("the next plan");
  db.append_events_with_plan(&run, &events, &next).expect("the plan");
}

/// Every text the window draws, a link's trailing runs included.
fn drawn(page: &mut Page) -> Vec<String> {
  page.window.pass();
  let tree = &page.window.tree;
  tree
    .resolve_quads(tree.last_layout().expect("a layout"))
    .into_iter()
    .flat_map(|quad| match quad.content {
      QuadContent::RichText { spans, .. } => spans.into_iter().map(|span| span.text).collect(),
      QuadContent::Text { text, .. } => vec![text],
      _ => Vec::new(),
    })
    .collect()
}

#[test]
fn decision_plan_link_shows_the_host_it_opens_and_synthetic_input_opens_nothing() {
  let mut page = Page::seeded("task-states");
  propose(&page, "plan v1 waiting", LYING);
  page.open("plan v1 waiting", "plan-review");
  page.window.until("the plan's link", |window| {
    window
      .tree
      .find_element(|element| element.text_content() == Some("github.com/acme/kontur"))
      .is_some()
  });
  assert!(
    drawn(&mut page).iter().any(|text| text == "evil.example"),
    "the link shows the host it opens"
  );
  let before = page.window.navigation().route_now();
  let bounds = page
    .window
    .tree
    .find_element(|element| element.text_content() == Some("github.com/acme/kontur"))
    .expect("the link")
    .bounds();
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  with_synthetic_input(|| {
    page.window.tree.mouse_move(x, y);
    page.window.tree.mouse_down(x, y, MouseButton::Left);
    page.window.tree.mouse_up(x, y, MouseButton::Left);
  });
  page.window.pass();
  assert_eq!(
    page.window.navigation().route_now(),
    before,
    "a synthetic press opens nothing"
  );
}
