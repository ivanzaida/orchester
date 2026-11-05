//! The Merge row of a task's properties (ORC-364, `Review · redesign` 01h `fUHdZ` and `Cjprb` › *Task page ·
//! Properties › Merge row*) in the app's real window, on the QA seed's `task-merge-row` scenario (`examples/seed`):
//! under Branch in each of its eight states once the run waits for review or its merge was attempted, none before; it
//! stays beside the Summary's top card once a merge did not land (01j `k4C0Es`, ORC-731); the row opens the run's
//! attempt page. Needs `git`, as the seed does. `cargo test -p orchester-app --test task_merge_row task_merge_row`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::time::Duration;

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{
  id::ProjectId,
  project::TaskKey,
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's attempts with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);

struct Page {
  window: Window,
  _stand_ins: StandIns,
  project: ProjectId,
  db: ProjectDb,
}

impl Page {
  fn new() -> Self {
    let data = Data::new();
    let repo = data.dir.path().join("repo-task-merge-row");
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: repo.clone(),
      scenario: scenarios::named("task-merge-row").expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
    window.tree.resize(1440, 1400);
    Self {
      window,
      _stand_ins: stand_ins,
      project: seeded.project,
      db: ProjectDb::open(&repo).expect("the project database"),
    }
  }

  /// The key of the task titled `title`.
  fn key(&self, title: &str) -> TaskKey {
    self
      .db
      .tasks()
      .expect("the tasks")
      .into_iter()
      .find(|task| task.details().title.as_str() == title)
      .unwrap_or_else(|| panic!("a task titled {title}"))
      .key()
      .clone()
  }

  /// Opens the Summary of the task titled `title`, until its properties are shown; its key.
  fn open(&mut self, title: &str) -> TaskKey {
    let key = self.key(title);
    let page = ProjectPage::Task(key.clone(), TaskView::of(TaskTab::Summary));
    self.window.navigation().go(Route::Project(self.project, page));
    self.window.until_within(GIT_WORK, "the task's properties", |window| {
      window.texts("task-property-created-row").is_some()
    });
    key
  }

  /// Until the Merge row of the open task says `words`.
  fn until_row(&mut self, words: &str) {
    self.window.until_within(GIT_WORK, words, |window| {
      window.tree.tick_timers();
      window
        .texts("task-property-merge-label")
        .is_some_and(|texts| texts.iter().any(|text| text == words))
    });
  }

  /// The words of the open task's Merge row, once it is shown.
  fn words(&mut self) -> String {
    self.window.until_within(GIT_WORK, "the Merge row", |window| {
      window.texts("task-property-merge-label").is_some()
    });
    self.window.text("task-property-merge-label")
  }
}

#[test]
fn task_merge_row_shows_each_state_under_branch() {
  let mut page = Page::new();
  // Today's attempts and merges say their clock time after the state.
  for (title, state) in [
    ("Add retry to webhook sender", "Not merged · "),
    ("Rate limit per endpoint", "Git stopped · "),
    ("Delivery dashboard", "Not recorded · "),
    ("Delivery log per endpoint", "Merged · "),
  ] {
    page.open(title);
    let words = page.words();
    assert!(words.starts_with(state) && words.contains(':'), "{title}: {words}");
    assert_eq!(page.window.text("task-property-merge-key"), "Merge");
  }
  for (title, words) in [
    ("Webhook replay endpoint", "Waits for your review"),
    ("Signed delivery receipts", "Retrying · 1 of 3"),
    ("Payload size guard", "Files left changed"),
    ("Per-endpoint secrets", "Conflicts · 2 files"),
  ] {
    page.open(title);
    page.until_row(words);
  }
}

#[test]
fn task_merge_row_sits_under_branch_and_is_absent_before_a_merge() {
  let mut page = Page::new();
  page.open("Add retry to webhook sender");
  page.words();
  let top = |page: &mut Page, id: &str| {
    let bounds = page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    (bounds.y, bounds.y + bounds.height)
  };
  let (_, branch) = top(&mut page, "task-property-branch-row");
  let (merge, merge_end) = top(&mut page, "task-property-merge-row");
  let (base, _) = top(&mut page, "task-property-base-row");
  assert!(
    branch <= merge && merge_end <= base,
    "Merge between Branch ({branch}) and Base ({base}): {merge}..{merge_end}"
  );
  page.open("Export webhooks as CSV");
  assert!(
    page.window.texts("task-merge-row").is_none(),
    "a task that never ran has no Merge row"
  );
}

#[test]
fn task_merge_row_keeps_the_open_arrow_apart_from_long_words() {
  let mut page = Page::new();
  page.open("Webhook replay endpoint");
  page.until_row("Waits for your review");
  // `Value` in `Cjprb`: the words, a spacer that takes the room left, and the arrow, 6 apart: 12 at the least. The
  // narrow window gives the words less room than they take.
  for width in [1440, 900] {
    page.window.tree.resize(width, 1400);
    page.window.pass();
    let mut bounds = |id: &str| {
      page
        .window
        .tree
        .get_element_by_id_mut(id)
        .and_then(|element| element.bounds())
        .unwrap_or_else(|| panic!("{id} is laid out"))
    };
    let (label, arrow) = (bounds("task-property-merge-label"), bounds("task-property-merge-open"));
    let gap = arrow.x - (label.x + label.width);
    assert!(gap >= 12.0 - 0.5, "{width}: the arrow is {gap} after the words");
  }
}

#[test]
fn task_merge_row_stays_beside_the_not_merged_card_and_opens_the_attempt_page() {
  let mut page = Page::new();
  // The top card turns to the outcome once a merge did not land (01j `k4C0Es`, ORC-731); the row stays.
  for (title, outcome) in [
    ("Add retry to webhook sender", "Not merged into main"),
    ("Per-endpoint secrets", "Conflicts with main"),
    ("Delivery dashboard", "Merged by Git, not recorded"),
  ] {
    page.open(title);
    page.words();
    page.window.until_within(GIT_WORK, outcome, |window| {
      window
        .texts("task-notice-title")
        .is_some_and(|texts| texts.join(" ") == outcome)
    });
    for gone in ["task-merge-conflicts", "task-resolve", "task-open-local-integration"] {
      assert!(
        page.window.texts(gone).is_none(),
        "{title}: {gone} is not on the Summary"
      );
    }
  }
  let key = page.open("Add retry to webhook sender");
  page.words();
  page.window.press("task-merge-row");
  page.window.until_within(GIT_WORK, "the run's attempt page", |window| {
    window.texts("merge-attempt-status").is_some()
  });
  let run = page
    .db
    .runs_of_task(
      page
        .db
        .tasks()
        .expect("the tasks")
        .into_iter()
        .find(|task| *task.key() == key)
        .expect("the task")
        .id(),
    )
    .expect("the runs")
    .into_iter()
    .find_map(Result::ok)
    .expect("its run");
  assert_eq!(
    page.window.navigation().route_now(),
    Route::Project(page.project, ProjectPage::MergeAttempt(key, run.number()))
  );
}
