//! The decisions a task's Summary waits for, headless at 1440 × 900 on the QA seed's `task-states` and
//! `decisions` scenarios: the plan review with its versions and the note Request changes needs, raising
//! the limit with its read-back, a daily budget's stop that only Resume continues, the decision after a
//! step's retries, and Ready for review with Merge into main merging, refused at a conflict, unavailable
//! without a branch to merge into, and merged. No runner is connected here (its program does not exist):
//! each decision the runner carries out comes back refused, in so many words. Needs `git`.

#[path = "task_decisions/accounts.rs"]
mod accounts;
#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod support;

use std::path::{Path, PathBuf};

use orchester_app::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::icon::Icon,
};
use orchester_core::{id::ProjectId, time::Timestamp};
use support::{Data, Window};
use tokio::runtime::Runtime;

/// Why a decision waits: no runner is connected to carry it out (the header's own words).
const OFFLINE: &str = "The runner is not connected; the run's controls return once it is online.";

/// The window on a seeded scenario's project, and the seeded tasks: key and state.
struct Page {
  window: Window,
  project: ProjectId,
  tasks: Vec<(String, &'static str)>,
}

impl Page {
  fn seeded(scenario: &str) -> Self {
    Self::seeded_with(scenario, |_, _| None)
  }

  /// Seeds `scenario`; `prepare` is called with the temporary folder before seeding (no repository yet), and
  /// returns the fake Codex program to seed with, then again with the repository once seeded.
  fn seeded_with(scenario: &str, prepare: impl Fn(&Path, Option<&Path>) -> Option<PathBuf>) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
    std::fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::tempdir_in(scratch).expect("a temporary folder");
    let request = kontur::SeedRequest {
      data_dir: dir.path().to_path_buf(),
      repo: dir.path().join(format!("repo-{scenario}")),
      scenario: kontur::scenarios::named(scenario).expect("a known scenario"),
      fake_codex: prepare(dir.path(), None),
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    prepare(dir.path(), Some(&seeded.repo));
    let data = Data {
      dir,
      runtime: Runtime::new().expect("tokio runtime"),
    };
    let launch = data.launch();
    let window = Window::of(data, launch);
    let tasks = seeded.tasks.into_iter().map(|task| (task.key, task.state)).collect();
    Self {
      window,
      project: seeded.project,
      tasks,
    }
  }

  /// Opens the Summary of the task left in `state` and waits until `ready` is shown.
  fn open(&mut self, state: &str, ready: &str) {
    let (key, _) = self
      .tasks
      .iter()
      .find(|(_, seeded)| *seeded == state)
      .unwrap_or_else(|| panic!("a task {state}"))
      .clone();
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(TaskTab::Summary));
    self.window.navigation().go(Route::Project(self.project, page));
    self
      .window
      .until(&format!("{ready} on {key}"), |window| window.texts(ready).is_some());
  }

  fn text(&mut self, id: &str) -> String {
    self
      .window
      .texts(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .join(" ")
  }

  fn bounds(&mut self, id: &str) -> lurq::core::ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  fn has(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .click();
    self.window.pass();
  }

  fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .set_value(value);
    self.window.pass();
  }

  fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .value()
  }

  /// The labels of the select `id`.
  fn options(&mut self, id: &str) -> Vec<String> {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"))
      .labels()
      .iter()
      .map(ToString::to_string)
      .collect()
  }

  fn choose(&mut self, id: &str, label: &str) {
    let mut select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"));
    let index = select
      .labels()
      .iter()
      .position(|option| &**option == label)
      .unwrap_or_else(|| panic!("an option {label}"));
    select.commit(index);
    self.window.pass();
  }

  /// Whether the button `id` shows the lock of an unavailable action in place of its icon.
  fn locked(&mut self, id: &str) -> bool {
    self
      .window
      .tree
      .get_element_by_id(&format!("{id}-icon"))
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph == Icon::Lock.glyph().to_string())
  }

  /// Waits until the element `id` says `words`.
  fn until_says(&mut self, id: &str, words: &str) {
    self.window.until(&format!("{id} to say {words}"), |window| {
      window.texts(id).is_some_and(|texts| texts.join(" ").contains(words))
    });
  }
}

#[test]
fn decision_plan_review_lists_versions_and_request_changes_needs_a_note() {
  let mut page = Page::seeded("task-states");
  page.open("plan v2 waiting", "plan-review");
  assert_eq!(
    page.text("plan-review-title"),
    "Plan v2 · Back off when GitHub rate-limits the sync"
  );
  assert_eq!(
    page.options("plan-review-version"),
    ["v2 · latest", "v1 · changes requested"]
  );
  let changed = page.text("plan-review-changed");
  assert!(
    changed.starts_with("Changed since v1, from your note “Don't drop messages while we wait"),
    "{changed}"
  );
  let key = key_of(&page, "plan v2 waiting");
  assert_eq!(
    page.text("plan-review-criteria"),
    format!("Acceptance criteria of {key} 1 The change is covered by tests. 2 Nothing changes for existing users.")
  );
  assert!(page.has("task-stages"), "the run's stages in place of its step rows");
  assert!(!page.has("task-steps"), "no step rows");
  assert!(!page.has("task-brief"), "the plan's card holds what the task asks for");
  page.choose("plan-review-version", "v1 · changes requested");
  page.until_says("plan-review-title", "Plan v1");
  assert_eq!(
    page.text("plan-review-changed"),
    "This is v1, an earlier plan. Plan v2 waits for your decision."
  );
  assert_eq!(
    page.text("plan-review-approve-hint"),
    "Switch to v2 to approve.",
    "approving decides the version that waits"
  );
  page.choose("plan-review-version", "v2 · latest");
  page.until_says("plan-review-title", "Plan v2");
  page.set("plan-review-note", "Show how many messages wait.");
  assert_eq!(
    page.text("plan-review-approve-hint"),
    "Clear the note to approve, or request changes."
  );
  // Deciding needs the runner, which is not connected here: both wait, and say why.
  assert!(page.locked("plan-approve"));
  assert!(page.locked("plan-request-changes"));
  assert_eq!(page.text("plan-review-offline"), OFFLINE);
  page.open("plan v1 waiting", "plan-review");
  assert_eq!(page.options("plan-review-version"), ["v1 · latest"]);
  assert!(!page.has("plan-review-changed"));
  assert_eq!(
    page.text("plan-review-approve-hint"),
    "Approving starts 03 Implement with Builder."
  );
}

#[test]
fn decision_raise_reads_back_the_new_limit_and_asks_the_runner() {
  let mut page = Page::seeded("task-states");
  page.open("stopped at its token limit", "raise-limit");
  assert_eq!(
    page.text("raise-limit-tokens"),
    "Used 500k of 500k · Implement stopped mid-step"
  );
  assert!(page.has("raise-limit-meter"));
  assert_eq!(page.value("raise-limit-input"), "800,000");
  assert_eq!(
    page.text("raise-limit-field-hint"),
    "= 800k tokens · 300k more than now"
  );
  assert_eq!(page.text("raise-limit-continue-label"), "Continue with 800k tokens");
  page.set("raise-limit-input", "400,000");
  page.until_says(
    "raise-limit-input-note",
    "Enter more than the 500k tokens this run already used.",
  );
  // No maximum: far above any project's maximum is read as it is, up to what the project database holds.
  page.set("raise-limit-input", "90,000,000");
  page.until_says("raise-limit-field-hint", "= 90M tokens · 89.5M more than now");
  page.set("raise-limit-input", "10,000,000,000,000,000,000");
  page.until_says(
    "raise-limit-input-note",
    "Enter a smaller number: a budget can be at most 9,223,372,036,854,775,807 tokens.",
  );
  assert!(page.locked("raise-limit-continue"), "continuing waits for a runner");
  assert_eq!(page.text("raise-limit-offline"), OFFLINE);
  // Removing the limit continues the run without it, which needs the runner too.
  assert!(page.locked("raise-limit-remove"), "removing waits for a runner");
  assert_eq!(
    page.text("raise-limit-remove-meaning"),
    "Remove the limit continues the run with no token limit."
  );
  // An empty field names no new limit: Remove the limit is how the run goes on without one.
  page.set("raise-limit-input", "");
  page.until_says("raise-limit-input-note", "Enter a token budget.");
  assert!(page.has("raise-limit-tokens-by-step"));
  assert_eq!(page.text("raise-limit-step-research"), "40k tokens · Explorer");
}

#[test]
fn decision_daily_budget_allows_only_resume() {
  let mut page = Page::seeded("task-states");
  page.open("stopped by the daily budget", "task-notice");
  page
    .window
    .until("the run controls", |window| window.texts("task-resume-run").is_some());
  assert!(
    !page.has("raise-limit"),
    "raising the run's own limit does not lift the day's"
  );
  assert!(!page.has("raise-limit-continue"));
  assert!(page.has("task-steps"), "the usual Summary");
  assert!(page.locked("task-resume-run"), "Resume waits for a runner");
  assert_eq!(page.text("task-action-reasons"), "Runner not connected");
}

#[test]
fn decision_after_retries_allows_a_retry_returns_with_a_note_and_stops() {
  let mut page = Page::seeded("decisions");
  page.open("waits after Review's retries", "task-retry-decision");
  assert_eq!(page.text("task-retry-decision-title"), "Decide how the run continues");
  assert_eq!(page.text("task-retry-reported-head"), "What Review reported");
  assert!(page.has("task-retry-report-text"));
  assert!(page.has("task-retry-attempt"), "the latest attempt's steps");
  let options = page.options("task-return-step");
  assert!(options.contains(&"03 Implement · default".to_owned()), "{options:?}");
  let title = page.text("task-notice-title");
  assert!(
    title.starts_with("Review still fails after ") && (title.ends_with(" retries") || title.ends_with(" retry")),
    "{title}"
  );
  assert!(page.text("task-notice").contains("sent the work back to Implement"));
  // Each decision goes to the runner, which is not connected here: they wait, and say why.
  page.set("task-return-note", "Sample more than ten delays.");
  for button in ["task-allow-retry", "task-return-with-note", "task-stop-run"] {
    assert!(page.locked(button), "{button}");
  }
  assert_eq!(page.text("task-retry-offline"), OFFLINE);
}

#[test]
fn decision_ready_for_review_opens_local_integration_before_any_merge() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, merges cleanly", "task-merge");
  assert_eq!(page.text("task-notice-title"), "Ready for your review");
  assert!(page.locked("task-review-pull-request"));
  assert_eq!(
    page.text("task-review-pull-request-reason"),
    "Review PR needs a pull request, which no pipeline step opens yet."
  );
  assert!(page.locked("task-ask-agent"), "Ask agent waits for the composer");
  page.until_says("task-ready-changes-figure", "1 file");
  assert!(page.has("task-ready-check-review"), "Review verified the work");
  assert!(page.has("task-run-details"));
  page.click("task-merge");
  page.window.until("local checkout review", |window| {
    window.tree.get_element_by_id("task-local-integration").is_some()
  });
  assert!(
    page
      .window
      .navigation()
      .route_now()
      .path()
      .ends_with("/local-integration")
  );
  assert!(!page.has("local-confirm"), "no merge approval before a fresh Check");
}

#[test]
fn decision_merge_opens_local_review_for_a_task_that_will_conflict() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, conflicts with main", "task-merge");
  page.click("task-merge");
  page.window.until("local checkout review", |window| {
    window.tree.get_element_by_id("task-local-integration").is_some()
  });
  assert!(
    page
      .window
      .navigation()
      .route_now()
      .path()
      .ends_with("/local-integration")
  );
  assert!(
    !page.has("local-result-editor"),
    "sync has not begun from an entry click"
  );
}

#[test]
fn decision_merge_is_unavailable_without_a_branch_and_merged_says_where() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, no branch to merge into", "task-merge");
  assert!(page.locked("task-merge"));
  assert_eq!(
    page.text("task-merge-reason"),
    "Merge needs the branch your checkout had when the run started, which this run didn't record."
  );
  let mut page = Page::seeded("task-states");
  page.open("merged", "task-notice");
  assert_eq!(page.text("task-notice-title"), "Merged into main");
  assert!(page.has("task-stages"));
  assert!(!page.has("task-merge"));
  assert!(!page.has("task-review-pull-request"));
}

#[test]
fn decision_external_git_evidence_replaces_ready_without_a_historical_merge_record() {
  use orchester_core::{
    integration::local::LocalIntegrationStore,
    store::{MergeStore, RunStore, TaskStore},
  };
  use orchester_store::ProjectDb;

  let mut page = Page::seeded("decisions");
  let root = page.window.data.dir.path().join("repo-decisions");
  let db = ProjectDb::open(&root).expect("project");
  let key = key_of(&page, "ready for review, merges cleanly");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .max_by_key(|run| run.number().get())
    .expect("run");
  let output = std::process::Command::new("git")
    .args([
      "merge",
      "--no-ff",
      "-m",
      "External QA integration",
      run.branch().as_str(),
    ])
    .current_dir(&root)
    .output()
    .expect("git");
  assert!(output.status.success(), "external merge completes");
  page.open("ready for review, merges cleanly", "task-external-integration");
  assert!(!page.has("task-merge"));
  assert!(!page.has("task-notice-title"));
  assert!(
    page
      .text("task-external-integration")
      .contains("Integration result needs reconciliation")
  );
  assert!(db.run_merge(run.id()).expect("record").is_none());
  assert!(db.local_operation(run.id()).expect("operation").is_none());
  page.click("task-inspect-external-integration");
  page.window.until("current external Git evidence", |window| {
    window.tree.get_element_by_id("local-external-integration").is_some()
  });
  assert!(!page.has("local-sync"));
  assert!(!page.has("local-confirm"));
}

/// The key of the task seeded in `state`.
fn key_of(page: &Page, state: &str) -> String {
  page
    .tasks
    .iter()
    .find(|(_, seeded)| *seeded == state)
    .map(|(key, _)| key.clone())
    .unwrap_or_else(|| panic!("a task {state}"))
}
