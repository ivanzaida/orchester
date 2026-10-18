//! A task's Details and its Summary before the first run, headless on the QA seed's `task-states` and `tasks`
//! scenarios: Details shows the whole title, description and every criterion of a task in each state, unscored;
//! the version picker offers only the versions kept, falls back honestly for an older task, keeps its choice as
//! the tabs switch and the task is read again, and shows the run's setup apart from the task's organization; in a
//! 960 window the side sections go under the requirements. Before the first run the Summary says there are no
//! runs yet, with Start run unavailable for its real reason and View task details; a run that cannot be read, or
//! one a teammate ran of a shared task, is never taken for none. No runner is connected (its program does not
//! exist). Needs `git`.
//!
//! `cargo test -p orchester-app --test task_details <filter>`

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod support;

use std::{collections::HashMap, num::NonZeroU32, path::PathBuf};

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{
  id::{ProjectId, ServerId, UserId},
  run::TaskPhase,
  sharing::MemberRole,
  store::TaskStore,
  task::{Task, TaskDetails},
  time::Timestamp,
};
use orchester_store::{CachedRunStatus, CachedTask, ProjectDb, SharedPerson, SharingLink};
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

  fn select(&mut self, id: &str) -> (Vec<String>, Vec<String>) {
    let select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"));
    let all = select.labels().iter().map(ToString::to_string).collect();
    let chosen = select.selected_labels().iter().map(ToString::to_string).collect();
    (all, chosen)
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

  fn locked(&mut self, id: &str) -> bool {
    self
      .window
      .tree
      .get_element_by_id(&format!("{id}-icon"))
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph == orchester_app::ui::icon::Icon::Lock.glyph().to_string())
  }

  fn bounds(&mut self, id: &str) -> lurq::core::ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  /// Tells the page that the project's tasks were changed outside it, as automation's changes do.
  fn changed_outside(&mut self) {
    let services = self.window.launch.services().expect("the session started").clone();
    services.changes.record(self.project, Outside::Tasks);
  }
}

/// The Details show `details` whole: its title, its description and each criterion, numbered, and nothing else
/// in the criteria list.
fn assert_shows(page: &mut Page, details: &TaskDetails, what: &str) {
  assert_eq!(page.text("task-details-title-text"), details.title.as_str(), "{what}");
  let description = page.text("task-details-description-text");
  for line in details.brief.lines().filter(|line| !line.trim().is_empty()) {
    let words = line.trim_start_matches(['#', '-', '*', ' ']);
    let first = words.split_whitespace().next().unwrap_or_default();
    assert!(description.contains(first), "{what}: {description} has {line}");
  }
  let count = details.acceptance_criteria.len();
  assert_eq!(
    page.text("task-details-criteria-head"),
    format!("Acceptance criteria · {count}"),
    "{what}"
  );
  let mut expected = Vec::new();
  for (number, criterion) in (1..).zip(&details.acceptance_criteria) {
    expected.push(format!("{number:02}"));
    expected.push(criterion.clone());
  }
  if count > 0 {
    let shown = page.window.texts("task-details-criteria-list").expect("the criteria");
    assert_eq!(shown, expected, "{what}: every criterion, unscored");
  }
}

#[test]
fn details_show_every_requirement_in_each_state_without_results() {
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
    page.open(&key, TaskTab::Details, "task-details-requirements");
    let task = page.task(&key);
    assert_shows(&mut page, task.details(), state);
    let shown = page.text("task-details-requirements");
    for result in ["Passed", "PASS", "Failed", "✓"] {
      assert!(
        !shown.contains(result),
        "{state}: no result is said of a criterion: {shown}"
      );
    }
    assert_eq!(
      page.text("task-details-provenance"),
      "Read-only · original task saved at creation"
    );
    assert_eq!(
      page.select("task-details-source").1,
      vec!["At task creation".to_owned()],
      "{state}"
    );
  }
}

#[test]
fn details_the_picker_switches_versions_and_keeps_its_choice() {
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
  page.open(&key, TaskTab::Details, "task-details-requirements");
  let (options, chosen) = page.select("task-details-source");
  let number = options[1]
    .trim_start_matches("Run #")
    .split(' ')
    .next()
    .unwrap_or_default()
    .to_owned();
  assert_eq!(
    options,
    vec![
      "At task creation".to_owned(),
      format!("Run #{number} · at start"),
      "Current task".to_owned()
    ]
  );
  assert_eq!(chosen, vec!["At task creation".to_owned()]);
  assert_shows(&mut page, created.details(), "at creation");
  page.choose("task-details-source", &format!("Run #{number} · at start"));
  assert_eq!(
    page.text("task-details-provenance"),
    format!("Read-only · requirements given to Run #{number}")
  );
  assert_shows(&mut page, created.details(), "as the run started");
  page.choose("task-details-source", "Current task");
  assert_eq!(
    page.text("task-details-provenance"),
    format!("Current task · edited after Run #{number}")
  );
  assert_shows(&mut page, &edited, "now");
  assert!(
    page
      .text("task-details-setup-title")
      .starts_with(&format!("Run #{number} setup"))
  );
  assert_eq!(page.text("task-details-setup-note"), "Captured for this run");
  assert_eq!(
    page.text("task-details-organization-note"),
    "Current values · not a historical snapshot"
  );
  // The choice stays as the tabs switch and the task is read again.
  page.window.press("task-tab-summary");
  page
    .window
    .until("the Summary", |window| window.texts("task-notice-title").is_some());
  page.window.press("task-tab-details");
  page.window.until("the Details", |window| {
    window.texts("task-details-requirements").is_some()
  });
  assert_eq!(page.select("task-details-source").1, vec!["Current task".to_owned()]);
  page.changed_outside();
  page.window.until("the task read again", |window| {
    window.texts("task-details-requirements").is_some()
  });
  assert_eq!(page.select("task-details-source").1, vec!["Current task".to_owned()]);
  assert_shows(&mut page, &edited, "read again");
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
  page.open(&key, TaskTab::Details, "task-details-requirements");
  let (options, chosen) = page.select("task-details-source");
  assert_eq!(options.len(), 2, "no creation version is offered: {options:?}");
  assert!(chosen[0].ends_with("· at start"), "{chosen:?}");
  assert_eq!(
    page.text("task-details-fallback-title"),
    "Creation version not recorded"
  );
  assert_eq!(
    page.text("task-details-fallback-text"),
    "This older task has no creation snapshot. The saved run-start version is shown instead."
  );
  assert_shows(&mut page, task.details(), "the run's start version");
  page.choose("task-details-source", "Current task");
  assert!(
    !page.has("task-details-fallback"),
    "the current task stands in for nothing"
  );
}

#[test]
fn details_go_under_the_requirements_in_a_narrow_window() {
  let mut page = Page::seeded("task-states");
  let key = page.key("ready for review");
  page.open(&key, TaskTab::Details, "task-details-requirements");
  let (requirements, side) = (
    page.bounds("task-details-requirements"),
    page.bounds("task-details-side"),
  );
  assert!(side.x >= requirements.x + requirements.width - 1.0, "beside at 1440");
  page.window.tree.resize(960, 900);
  page.window.until("the narrow layout", |window| {
    window.texts("task-details-side").is_some()
  });
  page.window.pass();
  let (requirements, side) = (
    page.bounds("task-details-requirements"),
    page.bounds("task-details-side"),
  );
  assert!(side.y >= requirements.y + requirements.height - 1.0, "under at 960");
  let task = page.task(&key);
  assert_shows(&mut page, task.details(), "at 960");
}

#[test]
fn summary_before_the_first_run_has_no_runs_yet() {
  let mut page = Page::seeded("task-states");
  let key = page.key("draft, ready to queue");
  page.open(&key, TaskTab::Summary, "task-summary-empty");
  assert_eq!(page.text("task-summary-empty-title"), "No runs yet");
  assert_eq!(page.text("task-run-line"), "No runs yet", "the header in the Summary's words");
  assert!(page.locked("task-empty-start"), "Start run waits for a runner");
  assert_eq!(
    page.text("task-empty-start-reason"),
    "The runner is not connected; the run's controls return once it is online."
  );
  for nothing in [
    "task-steps",
    "task-run-rows",
    "task-this-run",
    "task-run-details",
    "task-remove-worktree",
  ] {
    assert!(!page.has(nothing), "no {nothing} before the first run");
  }
  page.window.press("task-empty-details");
  page.window.until("the Details", |window| {
    window.texts("task-details-requirements").is_some()
  });
  assert_eq!(page.text("task-details-setup-title"), "First run setup");
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
}

#[test]
fn summary_a_run_that_cannot_be_read_is_not_taken_for_no_runs() {
  let mut page = Page::seeded("task-states");
  let key = page.key("latest run unreadable");
  page.open(&key, TaskTab::Summary, "task-notice-title");
  assert!(!page.has("task-summary-empty"));
  page.open(&key, TaskTab::Details, "task-details-requirements");
  assert_eq!(page.text("task-details-setup-title"), "Run setup");
  assert_eq!(page.text("task-details-setup-pipeline"), "Not recorded");
}

/// Shares the project with `task` as its server has it: Alice Moreau ran it on her computer, run #7, which
/// works on Implement.
fn ran_on_a_teammates_computer(page: &Page, task: &Task) {
  let mut db = ProjectDb::open(&page.repo).expect("the project");
  let details = task.details();
  let alice = SharedPerson {
    id: UserId::generate(),
    name: "Alice Moreau".to_owned(),
  };
  let cached = CachedTask {
    id: task.id(),
    number: task.key().number(),
    revision: task.revision(),
    title: details.title.clone(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: None,
    limits: details.limits,
    status: task.status(),
    organization: db.task_organization(task.id()).expect("organization"),
    former_key: None,
    created_by: alice.name.clone(),
    created_at: task.created_at(),
    updated_by: alice.name.clone(),
    updated_at: Timestamp::now(),
    claim: None,
    run: Some(CachedRunStatus {
      holder: alice,
      number: NonZeroU32::new(7).expect("a number"),
      phase: TaskPhase::Running,
      step: Some("Implement".into()),
      started_at: Timestamp::now(),
      phase_since: Timestamp::now(),
      updated_at: Timestamp::now(),
    }),
  };
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: task.key().project().clone(),
    role: Some(MemberRole::Member),
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: Some("https://orchester.kontur.lan/kontur".into()),
  };
  db.start_sharing(&link, &[], (&[cached], &HashMap::new()))
    .expect("shared");
}

#[test]
fn summary_a_shared_task_a_teammate_ran_is_not_taken_for_no_runs() {
  let mut page = Page::seeded("task-states");
  let key = page.key("queued, run not started");
  let task = page.task(&key);
  ran_on_a_teammates_computer(&page, &task);
  page.open(&key, TaskTab::Summary, "task-notice-title");
  assert!(!page.has("task-summary-empty"), "the task had a run");
  assert_eq!(
    page.text("task-notice-title"),
    "Run #7 on Alice Moreau’s computer · Running · Implement"
  );
  page.open(&key, TaskTab::Details, "task-details-requirements");
  assert_eq!(page.text("task-details-setup-title"), "Run #7 setup");
  assert_eq!(
    page.text("task-details-setup-note"),
    "Ran on another computer · its setup is not on this one"
  );
  assert_eq!(page.text("task-details-setup-pipeline"), "Not recorded");
  page.choose("task-details-source", "Current task");
  assert_eq!(
    page.text("task-details-provenance"),
    "Current task · Run #7 ran on another computer, which keeps what it was given"
  );
  assert_shows(&mut page, task.details(), "a teammate's run");
}

#[test]
fn details_never_leak_another_tasks_choice() {
  let mut page = Page::seeded("task-states");
  let failed = page.key("failed");
  page.open(&failed, TaskTab::Details, "task-details-requirements");
  page.choose("task-details-source", "Current task");
  let merged = page.key("merged");
  page.open(&merged, TaskTab::Details, "task-details-requirements");
  assert_eq!(
    page.select("task-details-source").1,
    vec!["At task creation".to_owned()]
  );
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
    (TaskText::DetailsHeading, 15.0, FontWeight::SemiBold, 1.6),
    (TaskText::DetailsTitle, 16.0, FontWeight::Normal, 1.6),
    (TaskText::DetailsBody, 12.0, FontWeight::Normal, 1.6),
    (TaskText::DetailsNote, 11.0, FontWeight::Normal, 1.6),
    (TaskText::DetailsNoticeHeading, 12.0, FontWeight::Medium, 1.6),
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
