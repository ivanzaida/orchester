//! A task's page on its Summary, headless at 1440 × 900 on the QA seed's `task-states` scenario (one task
//! in each state the Summary shows) and, for Run again, on `tasks`: the notice per phase, the step rows,
//! *This run*, the run controls with why they are unavailable (no runner is connected here: its program
//! does not exist), Run again's reason and its confirmation in place, *Runs of this task* with *Remove
//! worktree* confirmed in place, and the lock line of a run that cannot be read. Needs `git`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod support;

use lurq::{
  app::{App, events::MouseButton},
  layout::text_style::FontWeight,
  node::{dimension::Dimension, element::ElementRef},
};
use orchester_app::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{self, Mode, TaskSpacing, TaskText},
  ui::icon::Icon,
};
use orchester_core::{id::ProjectId, time::Timestamp};
use support::{Data, Window};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// The window on a seeded scenario's project, and the seeded tasks: key, title and state.
struct Page {
  window: Window,
  project: ProjectId,
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
    let data = Data {
      dir,
      runtime: Runtime::new().expect("tokio runtime"),
    };
    assert_eq!(
      seeded.app_database,
      data.database(),
      "the seed registers in the window's folder"
    );
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
      tasks,
    }
  }

  /// Opens the Summary of the task left in `state`, once it shows; its key.
  fn open(&mut self, state: &str) -> String {
    let title = self
      .tasks
      .iter()
      .find(|(_, _, seeded)| *seeded == state)
      .unwrap_or_else(|| panic!("a task {state}"))
      .1
      .clone();
    self.open_titled(&title)
  }

  /// Opens the Summary of the task titled `title`, once it shows; its key.
  fn open_titled(&mut self, title: &str) -> String {
    let key = self
      .tasks
      .iter()
      .find(|(_, seeded, _)| seeded == title)
      .unwrap_or_else(|| panic!("a task {title}"))
      .0
      .clone();
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(TaskTab::Summary));
    self.window.navigation().go(Route::Project(self.project, page));
    let title = title.to_owned();
    self.window.until(&format!("the Summary of {key}"), |window| {
      window
        .texts("task-title-row")
        .is_some_and(|texts| texts.contains(&title))
        && (window.texts("task-notice-title").is_some() || window.texts("task-steps").is_some())
    });
    key
  }

  fn text(&mut self, id: &str) -> String {
    self
      .window
      .texts(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .join(" ")
  }

  fn has(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// Every element id under `id` that starts with `prefix`.
  fn ids(&mut self, id: &str, prefix: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(element) = self.window.tree.get_element_by_id(id) {
      ids_in(element, prefix, &mut found);
    }
    found
  }

  /// Clicks the middle of the element `id`.
  fn click(&mut self, id: &str) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.window.tree.mouse_move(x, y);
    self.window.tree.mouse_down(x, y, MouseButton::Left);
    self.window.tree.mouse_up(x, y, MouseButton::Left);
    self.window.pass();
  }

  /// Whether the button or tab `id` shows the lock of an unavailable action in place of its icon.
  fn locked(&mut self, id: &str) -> bool {
    self
      .window
      .tree
      .get_element_by_id(&format!("{id}-icon"))
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph == Icon::Lock.glyph().to_string())
  }
}

fn ids_in(element: ElementRef<'_>, prefix: &str, into: &mut Vec<String>) {
  let part = |id: &str| id.ends_with("-icon") || id.ends_with("-label");
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix) && !part(id)) {
    into.push(id.to_owned());
  }
  for child in element.children() {
    ids_in(child, prefix, into);
  }
}

#[test]
fn summary_notice_says_where_each_task_stands() {
  let mut page = Page::seeded("task-states");
  let cases = [
    ("draft, ready to queue", "Draft · ready to run"),
    ("draft, missing details", "Draft · not run yet"),
    ("queued, run not started", "Queued · its run has not started"),
    // No runner is connected: a run recorded as running is interrupted until one is.
    ("running", "Interrupted at Implement"),
    ("stopped by the daily budget", "Stopped at the daily token budget"),
    ("interrupted", "Interrupted at Implement"),
    ("failed", "Failed at Run checks"),
    ("cancelled", "Canceled"),
    ("waits after retries", "Run checks still fails after its retries"),
    ("ready for review", "Ready for your review"),
    ("merged", "Merged"),
  ];
  for (state, title) in cases {
    page.open(state);
    assert_eq!(page.text("task-notice-title"), title, "{state}");
  }
  page.open("stopped at its token limit");
  let limit = page.text("task-notice-title");
  assert!(limit.starts_with("Stopped at the token limit"), "{limit}");
  page.open("waits after retries");
  assert_eq!(page.text("task-status-label"), "Needs input · retries used");
  page.open("ready for review");
  assert_eq!(
    page.text("task-status-label"),
    "Ready for review",
    "the run waits for review"
  );
  page.open("paused");
  assert!(page.text("task-notice-title").starts_with("Paused"), "paused");
  let key = page.open("latest run unreadable");
  let notice = page.text("task-notice-title");
  assert!(
    notice.starts_with("Run #") && notice.ends_with("could not be read"),
    "{key}: {notice}"
  );
}

#[test]
fn summary_a_run_without_a_runner_reads_interrupted_everywhere() {
  let mut page = Page::seeded("task-states");
  page.open("running, approval waiting");
  assert_eq!(page.text("task-status-label"), "Interrupted · 03 Implement");
  assert!(page.text("task-step-implement-state").starts_with("Interrupted"));
  let rows = page.text("task-run-rows");
  assert!(rows.contains("Interrupted") && !rows.contains("Running"), "{rows}");
  assert!(rows.contains("No runner is connected"), "{rows}");
}

#[test]
fn summary_steps_and_the_panels_show_the_run() {
  let mut page = Page::seeded("task-states");
  page.open("failed");
  let rows = page.ids("task-step-rows", "task-step-");
  let states: Vec<&String> = rows.iter().filter(|id| id.ends_with("-state")).collect();
  assert_eq!(states.len(), 6, "a row per step of Feature delivery: {rows:?}");
  assert_eq!(page.text("task-steps-head"), "Steps 6");
  let checks = page.text("task-step-run-checks-state");
  assert!(checks.starts_with("Command failed · attempt 3 · "), "{checks}");
  assert_eq!(page.text("task-step-review-state"), "Skipped");
  // A run that ended has its details, not *This run*.
  assert_eq!(page.text("task-run-details-title"), "Run details");
  assert_eq!(page.text("task-this-run-project"), "Kontur");
  assert!(page.text("task-this-run-tokens").ends_with(" of 500k"));
  assert!(!page.has("task-this-run"));
  page.open("stopped at its token limit");
  assert_eq!(page.text("task-this-run-title"), "This run");
  assert_eq!(page.text("task-this-run-pipeline"), "Feature delivery v3");
  assert_eq!(page.text("task-this-run-timeLimit"), "90 min");
  assert_eq!(page.text("task-this-run-budget"), "500k / 500k tokens · limit reached");
  assert!(page.text("task-this-run-permissions-lead").starts_with("Last ran with"));
  assert_eq!(
    page.text("task-brief-text"),
    "Say in the status bar when the GitHub token expired."
  );
  assert_eq!(
    page.text("task-criteria"),
    "Acceptance criteria 2 1 The change is covered by tests. 2 Nothing changes for existing users."
  );
}

#[test]
fn summary_controls_say_why_they_are_unavailable() {
  let mut page = Page::seeded("task-states");
  page.open("paused");
  assert!(page.locked("task-resume-run"), "Resume waits for a runner");
  assert!(page.locked("task-cancel-run"), "Cancel waits for a runner");
  assert_eq!(page.text("task-action-reasons"), "Runner not connected");
  page.open("queued, run not started");
  assert!(page.locked("task-start-run"), "Retry run waits for a runner");
  page.open("draft, ready to queue");
  assert!(page.locked("task-run"), "a ready draft runs once a runner is connected");
  page.open("draft, missing details");
  assert!(!page.has("task-run"), "a draft that misses details has nothing to run");
  page.open("failed");
  assert!(!page.has("task-cancel-run"), "a run that ended has nothing to cancel");
  page
    .window
    .until("Run again", |window| window.texts("task-run-again").is_some());
  assert!(page.locked("task-run-again"), "the day's budget is used");
  assert_eq!(page.text("task-action-reasons"), "Today's token budget is used");
  page.open("latest run unreadable");
  assert!(
    page.locked("task-cancel-run"),
    "nothing can change a run that cannot be read"
  );
  let lock = page.text("task-unreadable-actions");
  assert!(lock.contains("are unavailable for run #"), "{lock}");
  assert!(!page.has("task-action-reasons"), "the Summary says it once");
  assert!(page.has("task-brief-text"), "what the task asks for can still be read");
}

#[test]
fn summary_a_blocked_task_says_what_blocks_it() {
  let mut page = Page::seeded("tasks");
  page.open_titled("Verify webhook signatures on receive");
  assert!(page.locked("task-run"), "a blocked draft does not run");
  let blocker = page
    .tasks
    .iter()
    .find(|(_, title, _)| title == "Webhook signing secrets per endpoint")
    .map(|(key, ..)| key.clone())
    .expect("the blocking task");
  assert_eq!(page.text("task-action-reasons"), format!("Blocked by {blocker}"));
  let notice = page.text("task-blocked");
  assert!(notice.starts_with("Blocked "), "{notice}");
  assert!(notice.contains(&format!("until {blocker} finish")), "{notice}");
}

#[test]
fn summary_remove_worktree_from_the_header_and_then_continue_is_unavailable() {
  let mut page = Page::seeded("task-states");
  let key = page.open("interrupted");
  let branch = format!("orchester/{}-run-1", key.to_lowercase());
  page.window.until("Remove worktree", |window| {
    window.texts("task-remove-worktree").is_some()
  });
  assert!(
    page.ids("task-run-rows", "run-remove-worktree-").is_empty(),
    "not twice"
  );
  page.click("task-remove-worktree");
  let confirm = page.ids("task-header", "run-remove-confirm-");
  let [confirm] = confirm.as_slice() else {
    panic!("one confirmation under the header: {confirm:?}");
  };
  let confirm = confirm.clone();
  let said = page.text(&confirm);
  assert!(
    said.contains("was interrupted: without its worktree it can never continue"),
    "{said}"
  );
  // It stays open as the tabs switch.
  page.click("task-tab-conversation");
  page.window.until("the Conversation tab", |window| {
    window.texts("task-conversation").is_some()
  });
  assert!(page.has(&confirm), "the confirmation stays open");
  page.click("task-tab-summary");
  page
    .window
    .until("the Summary", |window| window.texts("task-notice").is_some());
  let number = confirm.trim_start_matches("run-remove-confirm-").to_owned();
  page.click(&format!("run-remove-start-{number}"));
  page.window.until("the worktree to go", |window| {
    window.texts(&confirm).is_none()
      && window
        .texts("task-run-rows")
        .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Worktree removed")))
  });
  assert!(page.text("task-run-rows").contains(&branch));
  assert!(page.locked("task-continue-run"), "nothing is left to continue from");
  assert_eq!(
    page.text("task-action-reasons"),
    "Worktree removed · Runner not connected · Today's token budget is used"
  );
  assert_eq!(page.text("task-notice-title"), format!("Run #{number} can't continue"));
}

#[test]
fn summary_an_older_runs_removal_confirms_in_its_row() {
  let mut page = Page::seeded("tasks");
  // Tall enough for the runs and the confirmation without scrolling the Summary.
  page.window.tree.resize(1440, 2000);
  page.open("running, approval waiting");
  page.window.until("what the runs left", |window| {
    window
      .texts("task-run-rows")
      .is_some_and(|texts| texts.iter().any(|text| text.contains("files not committed")))
  });
  let buttons = page.ids("task-run-rows", "run-remove-worktree-");
  let [button] = buttons.as_slice() else {
    panic!("the canceled run's Remove worktree: {buttons:?}");
  };
  let number = button.trim_start_matches("run-remove-worktree-").to_owned();
  page.click(button);
  let confirm = format!("run-remove-confirm-{number}");
  page
    .window
    .until("the confirmation in the row", |window| window.texts(&confirm).is_some());
  assert!(page.ids("task-run-rows", "run-remove-confirm-").contains(&confirm));
  // Files would be lost: Remove waits for the switch that agrees to discard them.
  let start = format!("run-remove-start-{number}");
  page.click(&start);
  assert!(page.has(&confirm), "nothing is removed before discarding is agreed to");
  page.click(&format!("run-remove-discard-{number}"));
  page.click(&format!("run-remove-cancel-{number}"));
  page
    .window
    .until("the confirmation to close", |window| window.texts(&confirm).is_none());
}

#[test]
fn summary_run_again_confirms_in_place_and_says_why_the_runner_refused() {
  let mut page = Page::seeded("tasks");
  let key = page.open("failed");
  page
    .window
    .until("Run again", |window| window.texts("task-run-again").is_some());
  assert!(!page.locked("task-run-again"), "the task can run again");
  page.click("task-run-again");
  page.window.until("the confirmation", |window| {
    window.texts("task-run-again-confirm").is_some()
  });
  let title = page.text("task-run-again-title");
  assert!(title.starts_with(&format!("Run {key} again as run #")), "{title}");
  assert_eq!(page.text("task-run-again-pipeline"), "Feature delivery v3");
  assert_eq!(page.text("task-run-again-starts"), "Research · 6 steps");
  assert_eq!(page.text("task-run-again-limits"), "500k tokens · 90 min");
  page
    .window
    .until("the switch that removes the run before's worktree", |window| {
      window.tree.get_element_by_id("task-run-again-remove").is_some()
    });
  page.click("task-run-again-start");
  page.window.until("the refusal", |window| {
    window
      .texts("task-run-again-confirm")
      .is_some_and(|texts| texts.iter().any(|text| text.contains("The runner is not connected")))
  });
  page.click("task-run-again-cancel");
  page.window.until("the confirmation to close", |window| {
    window.texts("task-run-again-confirm").is_none()
  });
}

#[test]
fn summary_roles_follow_the_task_page_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, line_height) in [
    (TaskText::SectionTitle, 13.0, FontWeight::SemiBold, 1.3),
    (TaskText::StepName, 12.5, FontWeight::Medium, 1.3),
    (TaskText::RunTitle, 12.5, FontWeight::SemiBold, 1.3),
    (TaskText::Brief, 13.0, FontWeight::Normal, 1.55),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, style.line_height),
      (size, weight, line_height),
      "{role:?}"
    );
  }
  for (spacing, value) in [
    (TaskSpacing::Block, 22.0),
    (TaskSpacing::Panel, 20.0),
    (TaskSpacing::Foot, 28.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}
