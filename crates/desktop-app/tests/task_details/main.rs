//! What a task asks for and its properties, on its Summary (ORC-100: the Details tab went into it), headless on the QA
//! seed's `task-states` and `tasks` scenarios: a task opened without a tab opens on its Summary, its tabs Summary,
//! Conversation and Changes; the brief and every criterion of a task in each state show whole and unscored; the
//! brief's version chip offers only the versions kept, falls back honestly for an older task, keeps its choice as the
//! tabs switch and the task is read again; the properties show what the latest run started with and say when the task
//! has changed since. Before the first run the Summary says there are no runs yet over the brief, with Start run
//! unavailable for its real reason; a run that cannot be read, or one a teammate ran of a shared task ([`shared`]), is
//! never taken for none. No runner is connected (its program does not exist). Needs `git`.
//!
//! `cargo test -p orchester-app --test task_details <filter>`

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod run_mode;
mod shared;
#[path = "../support/mod.rs"]
mod support;

use std::path::PathBuf;

use orchester_app::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::markdown::plain::plain_text,
};
use orchester_core::{
  id::ProjectId,
  store::TaskStore,
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::outside::Outside;
use support::{Data, Window};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// The window on a seeded scenario's project, and the seeded tasks: key, title and state.
struct Page {
  window: Window,
  project: ProjectId,
  repo: PathBuf,
  tasks: Vec<(String, String, &'static str)>,
}

impl Page {
  fn seeded(scenario: &str) -> Self {
    let dir = TempDir::new().expect("a temporary folder");
    let request = kontur::SeedRequest {
      data_dir: dir.path().to_path_buf(),
      repo: dir.path().join(format!("repo-{scenario}")),
      scenario: kontur::scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let data = Data::at(dir, Runtime::new().expect("tokio runtime"));
    let launch = data.launch();
    let window = Window::of(data, launch);
    let tasks = seeded
      .tasks
      .into_iter()
      .map(|task| (task.key, task.title, task.state))
      .collect();
    Self {
      window,
      project: seeded.project,
      repo: seeded.repo,
      tasks,
    }
  }

  fn key(&self, state: &str) -> String {
    self
      .tasks
      .iter()
      .find(|(_, _, seeded)| *seeded == state)
      .unwrap_or_else(|| panic!("a task {state}"))
      .0
      .clone()
  }

  fn key_titled(&self, title: &str) -> String {
    self
      .tasks
      .iter()
      .find(|(_, seeded, _)| seeded == title)
      .unwrap_or_else(|| panic!("a task {title}"))
      .0
      .clone()
  }

  /// The stored task `key`.
  fn task(&self, key: &str) -> Task {
    ProjectDb::open(&self.repo)
      .expect("the project")
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("the task")
  }

  /// Opens `tab` of the task `key` and waits until `ready` is shown.
  fn open(&mut self, key: &str, tab: TaskTab, ready: &str) {
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(tab));
    self.window.navigation().go(Route::Project(self.project, page));
    let title = self.task(key).details().title.as_str().to_owned();
    self.window.until(&format!("{ready} of {key}"), |window| {
      window
        .texts("task-title-row")
        .is_some_and(|texts| texts.contains(&title))
        && window.texts(ready).is_some()
    });
  }

  fn text(&mut self, id: &str) -> String {
    self.window.text(id)
  }

  fn has(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// The versions the brief's chip offers, as its menu lists them; the menu closes again.
  fn versions(&mut self) -> Vec<String> {
    self.window.press("task-brief-version");
    self.window.until("the version menu", |window| {
      window.texts("task-brief-version-menu").is_some()
    });
    let mut versions = Vec::new();
    while let Some(texts) = self.window.texts(&format!("task-brief-version-{}", versions.len())) {
      versions.push(texts.join(" "));
    }
    self.window.press("task-brief-version");
    self.window.pass();
    versions
  }

  /// The version the brief shows, as its chip names it.
  fn version(&mut self) -> String {
    self.text("task-brief-version-label")
  }

  /// Picks `label` in the brief's version menu.
  fn choose(&mut self, label: &str) {
    let index = self
      .versions()
      .iter()
      .position(|version| version == label)
      .unwrap_or_else(|| panic!("a version {label}"));
    self.window.press("task-brief-version");
    self.window.until("the version menu", |window| {
      window.texts("task-brief-version-menu").is_some()
    });
    self.window.press(&format!("task-brief-version-{index}"));
    let label = label.to_owned();
    self.window.until("the version chosen", |window| {
      window.texts("task-brief-version-label") == Some(vec![label.clone()])
    });
  }

  fn locked(&mut self, id: &str) -> bool {
    self
      .window
      .tree
      .get_element_by_id(&format!("{id}-icon"))
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph == orchester_app::ui::icon::Icon::Lock.glyph().to_string())
  }

  /// Tells the page that the project's tasks were changed outside it, as automation's changes do.
  fn changed_outside(&mut self) {
    let services = self.window.launch.services().expect("the session started").clone();
    services.changes.record(self.project, Outside::Tasks);
  }
}

/// The Summary shows `details` whole: its brief and each criterion, numbered and read as Markdown, and nothing else in
/// the criteria list.
fn assert_shows(page: &mut Page, details: &TaskDetails, what: &str) {
  let brief = page.text("task-brief-text");
  for line in details.brief.lines().filter(|line| !line.trim().is_empty()) {
    let words = line.trim_start_matches(['#', '-', '*', ' ']);
    let first = words.split_whitespace().next().unwrap_or_default();
    assert!(brief.contains(first), "{what}: {brief} has {line}");
  }
  let count = details.acceptance_criteria.len();
  assert_eq!(
    page.text("task-criteria-head"),
    format!("Acceptance criteria {count}"),
    "{what}"
  );
  let mut expected = Vec::new();
  let mut shown = Vec::new();
  for (number, criterion) in (1..).zip(&details.acceptance_criteria) {
    expected.push(format!("{number:02} {}", plain_text(criterion)));
    let row = format!("task-criterion-{number}");
    let words = page
      .window
      .texts(&format!("{row}-text"))
      .expect("the criterion")
      .concat();
    shown.push(format!("{} {words}", page.text(&format!("{row}-number"))));
  }
  if count > 0 {
    let squeezed = |text: String| text.split_whitespace().collect::<String>();
    let list = page.window.texts("task-criteria-list").expect("the criteria");
    assert_eq!(
      squeezed(list.concat()),
      squeezed(shown.concat()),
      "{what}: nothing else in the list"
    );
    assert_eq!(shown, expected, "{what}: every criterion, unscored");
  }
}

#[test]
fn details_the_summary_shows_every_requirement_in_each_state_without_results() {
  let mut page = Page::seeded("task-states");
  for state in [
    "running",
    "waits after retries",
    "failed",
    "ready for review",
    "merged",
    "draft, ready to queue",
    "queued, run not started",
  ] {
    let key = page.key(state);
    page.open(&key, TaskTab::Summary, "task-criteria");
    let task = page.task(&key);
    assert_shows(&mut page, task.details(), state);
    let shown = page.text("task-criteria");
    for result in ["Passed", "PASS", "Failed", "✓"] {
      assert!(
        !shown.contains(result),
        "{state}: no result is said of a criterion: {shown}"
      );
    }
    for markup in ["**", "`", "]("] {
      assert!(!shown.contains(markup), "{state}: Markdown is rendered: {shown}");
    }
    assert_eq!(page.version(), "As created", "{state}");
  }
}

#[test]
fn details_the_version_chip_switches_versions_and_keeps_its_choice() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  let created = page.task(&key);
  // The task is edited after its run: a criterion more.
  let mut edited = created.details().clone();
  edited
    .acceptance_criteria
    .push("Cancellation stops pending retries.".into());
  let mut db = ProjectDb::open(&page.repo).expect("the project");
  db.update_task(created.id(), created.revision(), edited.clone())
    .expect("edit")
    .saved()
    .expect("saved as read");
  drop(db);
  page.open(&key, TaskTab::Summary, "task-criteria");
  let versions = page.versions();
  let number = versions[1]
    .trim_start_matches("As run #")
    .split(' ')
    .next()
    .unwrap_or_default()
    .to_owned();
  assert_eq!(
    versions,
    vec![
      "As created".to_owned(),
      format!("As run #{number} started"),
      "Current task".to_owned()
    ]
  );
  assert_eq!(page.version(), "As created");
  assert_shows(&mut page, created.details(), "as created");
  page.choose(&format!("As run #{number} started"));
  assert_shows(&mut page, created.details(), "as the run started");
  page.choose("Current task");
  assert_shows(&mut page, &edited, "now");
  // The choice stays as the tabs switch and the task is read again.
  page.window.press("task-tab-conversation");
  page
    .window
    .until("the Conversation", |window| window.texts("task-criteria").is_none());
  page.window.press("task-tab-summary");
  page
    .window
    .until("the Summary", |window| window.texts("task-criteria").is_some());
  assert_eq!(page.version(), "Current task");
  page.changed_outside();
  page
    .window
    .until("the task read again", |window| window.texts("task-criteria").is_some());
  assert_eq!(page.version(), "Current task");
  assert_shows(&mut page, &edited, "read again");
}

#[test]
fn details_a_task_opens_on_its_summary_its_tabs_summary_conversation_changes() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  let route = Route::parse(&format!("/project/{}/tasks/{key}", page.project)).expect("a task's page");
  page.window.navigation().go(route);
  page
    .window
    .until("the Summary", |window| window.texts("task-criteria").is_some());
  let tabs = page.window.texts("task-tabs").expect("the page tabs");
  assert_eq!(tabs.len(), 3, "three tabs: {tabs:?}");
  assert_eq!(tabs[0], "Summary");
  assert_eq!(tabs[1], "Conversation");
  assert!(tabs[2].starts_with("Changes"), "{tabs:?}");
  for gone in [
    "task-tab-details",
    "task-tab-artifacts",
    "task-tab-memory",
    "task-tab-local-integration",
  ] {
    assert!(!page.has(gone), "no {gone}");
  }
}

#[test]
fn details_an_older_task_says_its_creation_version_was_not_recorded() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  let task = page.task(&key);
  let connection = rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("database");
  connection
    .execute_batch("DROP TRIGGER task_creation_snapshots_no_delete; DELETE FROM task_creation_snapshots;")
    .expect("as before creation versions");
  drop(connection);
  page.open(&key, TaskTab::Summary, "task-criteria");
  let versions = page.versions();
  assert_eq!(versions.len(), 2, "no creation version is offered: {versions:?}");
  assert!(page.version().ends_with("started"), "{}", page.version());
  assert_eq!(
    page.text("task-brief-fallback-text"),
    "Creation version not recorded. This older task has no creation snapshot; the version its latest run started with \
     is shown instead."
  );
  assert_shows(&mut page, task.details(), "the run's start version");
  page.choose("Current task");
  assert!(
    !page.has("task-brief-fallback"),
    "the current task stands in for nothing"
  );
}

#[test]
fn details_the_properties_say_what_the_run_started_with_and_what_changed_since() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  page.open(&key, TaskTab::Summary, "task-properties");
  for (property, key) in [
    ("task-property-status-key", "Status"),
    ("task-property-priority-key", "Priority"),
    ("task-property-pipeline-key", "Pipeline"),
    ("task-property-tags-key", "Tags"),
    ("task-property-depends-key", "Depends on"),
    ("task-property-limits-key", "Limits"),
    ("task-property-branch-key", "Branch"),
    ("task-property-created-key", "Created"),
  ] {
    assert_eq!(page.text(property), key);
  }
  assert!(
    page.text("task-property-pipeline").starts_with("Feature delivery "),
    "the pinned pipeline and its version"
  );
  assert!(!page.has("task-property-limits-changed"));
  // The task's limits change after its run: the run's stay, said changed.
  let task = page.task(&key);
  let mut changed = task.details().clone();
  changed.limits = TaskLimits::new(Some(TokenBudget::new(123_000).expect("above zero")), None).expect("limits");
  let mut db = ProjectDb::open(&page.repo).expect("the project");
  db.update_task(task.id(), task.revision(), changed)
    .expect("edit")
    .saved()
    .expect("saved as read");
  drop(db);
  page.changed_outside();
  page.window.until("the limits said changed", |window| {
    window.texts("task-property-limits-changed").is_some()
  });
  assert_eq!(page.text("task-property-limits-changed"), "· changed");
  assert!(
    !page.text("task-property-limits-value").contains("123k"),
    "the run's limits, not the task's now"
  );
}

#[test]
fn summary_before_the_first_run_has_no_runs_yet_over_the_brief() {
  let mut page = Page::seeded("task-states");
  let key = page.key("draft, ready to queue");
  page.open(&key, TaskTab::Summary, "task-summary-empty");
  assert_eq!(page.text("task-summary-empty-title"), "No runs yet");
  assert_eq!(
    page.text("task-run-line"),
    "No runs yet",
    "the header in the Summary's words"
  );
  assert!(page.locked("task-empty-start"), "Start run waits for a runner");
  assert_eq!(
    page.text("task-empty-start-reason"),
    "The runner is not connected; the run's controls return once it is online."
  );
  for nothing in ["task-run-head", "task-output", "task-runs", "task-facts"] {
    assert!(!page.has(nothing), "no {nothing} before the first run");
  }
  let task = page.task(&key);
  assert_shows(&mut page, task.details(), "before the first run");
  let key = page.key("draft, missing details");
  page.open(&key, TaskTab::Summary, "task-summary-empty");
  assert!(page.locked("task-empty-start"));
  assert!(
    page
      .text("task-empty-start-reason")
      .starts_with("Start run is unavailable: the draft is missing details")
  );
  let key = page.key("queued, run not started");
  page.open(&key, TaskTab::Summary, "task-summary-empty");
  assert!(page.locked("task-empty-start"));
  // The header's control for the first run reads as the Summary's (`kh5Er` in `Xxrdy`).
  assert_eq!(page.text("task-empty-start"), "Start run");
  assert_eq!(page.text("task-start-run"), "Start run");
}

#[test]
fn summary_a_blocked_task_before_its_first_run_says_what_blocks_it() {
  let mut page = Page::seeded("tasks");
  let key = page.key_titled("Verify webhook signatures on receive");
  let blocker = page.key_titled("Webhook signing secrets per endpoint");
  page.open(&key, TaskTab::Summary, "task-summary-empty");
  assert!(page.locked("task-empty-start"));
  assert_eq!(
    page.text("task-empty-start-reason"),
    format!("Unavailable until {blocker} finishes.")
  );
  assert_eq!(page.text("task-property-depends"), blocker);
}

#[test]
fn summary_a_run_that_cannot_be_read_is_not_taken_for_no_runs() {
  let mut page = Page::seeded("task-states");
  let key = page.key("latest run unreadable");
  page.open(&key, TaskTab::Summary, "task-notice-title");
  assert!(!page.has("task-summary-empty"));
  assert_eq!(page.text("task-property-pipeline"), "Not recorded");
  assert_eq!(page.text("task-property-limits"), "Not recorded");
}

#[test]
fn details_never_leak_another_tasks_choice() {
  let mut page = Page::seeded("task-states");
  let failed = page.key("failed");
  page.open(&failed, TaskTab::Summary, "task-criteria");
  page.choose("Current task");
  let merged = page.key("merged");
  page.open(&merged, TaskTab::Summary, "task-criteria");
  assert_eq!(page.version(), "As created");
  let task = page.task(&merged);
  assert_shows(&mut page, task.details(), "the other task");
}

#[test]
fn details_roles_follow_the_design() {
  use lurq::{app::App, layout::text_style::FontWeight};
  use orchester_app::theme::{self, Mode, TaskText};
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, line_height) in [
    (TaskText::SectionTitle, 13.0, FontWeight::SemiBold, 1.3),
    (TaskText::Brief, 13.0, FontWeight::Normal, 1.6),
    (TaskText::EmptyGlyph, 32.0, FontWeight::Normal, 1.0),
    (TaskText::EmptyTitle, 20.0, FontWeight::SemiBold, 1.25),
    (TaskText::EmptyText, 13.0, FontWeight::Normal, 1.6),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, style.line_height),
      (size, weight, line_height),
      "{role:?}"
    );
  }
}
