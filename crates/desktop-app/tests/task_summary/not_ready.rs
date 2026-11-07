//! A Run on a pipeline that is not ready on this computer starts nothing (`Run state · Not ready`, L9 `f5CEX`,
//! ORC-840): the task stays a draft with no run, and its Summary lists each part that is missing with its fix, which
//! opens the place that part is set. On the QA seed's `picker-readiness` scenario: KON-1 is a draft with every detail
//! on Design pass, which runs a profile whose two MCP servers start programs this computer does not have. Needs `git`.

use orchester_app::{
  routes::{ProjectPage, Route},
  ui::icon::Icon,
};
use orchester_core::store::{RunStore, TaskStore};
use orchester_store::ProjectDb;

use crate::Page;

const STATE: &str = "draft on a pipeline that is not ready here";

/// The window on KON-1 after Run was pressed and the card showed.
fn refused() -> Page {
  let mut page = Page::seeded_with_runner("picker-readiness");
  page.open(STATE);
  // Run is locked until the runner is connected.
  page.wait("the runner", |window| {
    window
      .tree
      .get_element_by_id("task-run-icon")
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
  page.click("task-run");
  page.wait("the not-ready card", |window| window.texts("task-not-ready").is_some());
  page
}

#[test]
fn task_not_ready_run_lists_each_missing_part_with_its_fix_and_starts_nothing() {
  let mut page = refused();
  assert_eq!(page.text("task-not-ready-title"), "Not ready · nothing started");
  assert_eq!(page.text("task-not-ready-meta"), "Design pass v1 · 2 things missing");
  assert_eq!(
    page.text("task-not-ready-body"),
    "Fix these and press Run again. Nothing is queued until then."
  );
  assert_eq!(page.text("task-not-ready-row-0-what"), "orchester-qa-missing-docs");
  assert_eq!(
    page.text("task-not-ready-row-0-need"),
    "MCP server · set its program path"
  );
  assert_eq!(page.text("task-not-ready-row-0-fix"), "Set path →");
  assert_eq!(page.text("task-not-ready-row-1-what"), "orchester-qa-missing-search");
  assert_eq!(page.text("task-not-ready-open-agents"), "Open Agents · Needs you");
  assert_eq!(page.text("task-not-ready-note"), "Each fix opens its field in Agents");
  let words = page.text("task-not-ready");
  assert!(
    !words.contains("team") && !words.contains("computer"),
    "a local project's words: {words}"
  );
  // No notice repeats it, and nothing was queued, claimed or run.
  assert!(!page.has("task-control-refused"), "the card is the whole refusal");
  let db = ProjectDb::open(&page.window.data.dir.path().join("repo-picker-readiness")).expect("the project");
  let tasks = db.tasks().expect("tasks");
  assert_eq!(tasks.len(), 1);
  assert_eq!(tasks[0].status(), orchester_core::task::TaskStatus::Draft, "not queued");
  assert!(
    db.runs_of_task(tasks[0].id()).expect("runs").is_empty(),
    "no run, no attempt"
  );
}

#[test]
fn task_not_ready_a_fix_opens_the_place_that_part_is_set() {
  let mut page = refused();
  page.click("task-not-ready-row-0-fix");
  let route = page.window.navigation().route_now();
  assert!(
    matches!(route, Route::Project(_, ProjectPage::EditConnection(_))),
    "a local MCP server's program is set on its own page: {route:?}"
  );
}

#[test]
fn task_not_ready_open_agents_opens_the_agents_page() {
  let mut page = refused();
  page.click("task-not-ready-open-agents");
  let route = page.window.navigation().route_now();
  assert!(matches!(route, Route::Project(_, ProjectPage::Agents)), "{route:?}");
}
