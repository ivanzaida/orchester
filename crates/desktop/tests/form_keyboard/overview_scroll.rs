//! The Overview scrolls as one column when its sections do not fit the window (02.08 `Twtqc`):
//! the newest tasks, the footer with the way to the full task list and Insights stay reachable
//! with the wheel, and the scrollbar sits in a gutter of its own beside them.

use lurq::{app::events::ScrollPhase, core::ElementRect};
use orchester_core::{id::IdempotencyKey, name::Name, store::TaskStore, task::TaskDetails, time::Timestamp};
use orchester_desktop::theme::gutter_width;
use orchester_store::ProjectDb;

use crate::harness::Desktop;

const TASKS: usize = 30;
/// The overview lists the six newest tasks; the oldest of them is its last row.
const FIRST_ROW: &str = "task-row-KON-30";
const LAST_ROW: &str = "task-row-KON-25";
/// The overview's sections end 32 from the window's right edge (`Twtqc`).
const EDGE: f32 = 32.0;

fn seed(repo: &std::path::Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  let details = (1..=TASKS)
    .map(|number| TaskDetails {
      title: Name::new(&format!("Task {number}")).expect("title"),
      brief: String::new(),
      acceptance_criteria: vec![],
      pipeline: None,
      limits: None,
    })
    .collect();
  db.create_tasks(IdempotencyKey::generate(), details, Timestamp::now())
    .expect("tasks");
}

fn bottom(rect: ElementRect) -> f32 {
  rect.y + rect.height
}

fn right(rect: ElementRect) -> f32 {
  rect.x + rect.width
}

fn assert_near(found: f32, expected: f32) {
  assert!((found - expected).abs() < 0.5, "{found} is not {expected}");
}

/// A wheel turn towards the end of the overview, over its first task row.
fn wheel_down(desktop: &mut Desktop) {
  let row = desktop.bounds(FIRST_ROW);
  let (x, y) = (row.x + row.width / 2.0, row.y + row.height / 2.0);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  desktop.tree.scroll(x, y, 0.0, -5000.0, ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  desktop.pass();
}

#[test]
fn overview_scroll_reaches_the_last_task_row() {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for(FIRST_ROW);
  assert!(desktop.tree.get_element_by_id("task-row-KON-24").is_none());
  let viewport = desktop.bounds("overview-scroll");
  let last = desktop.bounds(LAST_ROW);
  assert!(
    bottom(last) > bottom(viewport),
    "the last row starts out of view: {last:?} in {viewport:?}"
  );
  // The scrollbar has a gutter of its own beside the column, and the rows still end 32 from the
  // edge, as drawn.
  let body = desktop.bounds("overview-body");
  assert_near(viewport.width - body.width, gutter_width());
  let row = desktop.bounds(FIRST_ROW);
  assert_near(right(viewport) - right(row), EDGE);

  wheel_down(&mut desktop);
  let last = desktop.bounds(LAST_ROW);
  assert!(
    last.y >= viewport.y && bottom(last) <= bottom(viewport),
    "the last row is in view: {last:?} in {viewport:?}"
  );
  let all = desktop.bounds("view-all-tasks");
  assert!(bottom(all) <= bottom(viewport), "View all tasks is in view: {all:?}");
  let insights = desktop.bounds("overview-insights");
  assert!(
    bottom(insights) <= bottom(viewport),
    "Insights is in view: {insights:?}"
  );
  // The project bar stays above the scrolled column.
  let bar = desktop.bounds("overview-project-bar");
  assert!(bottom(bar) <= viewport.y + 0.5, "{bar:?} above {viewport:?}");
}
