//! The top card of a task page whose run waits to be merged (ORC-731; `Review · redesign`, Row 4: 01i `k9GBy`, 01j
//! `k4C0Es`) in the app's real window, on the QA seed's `task-merge-card-*` scenarios (`examples/seed`): *Ready to
//! merge* with Merge (no key cap), Review changes opening the run's review page and Close without merging, the
//! header's Merge, and ⌘↵ (Ctrl+Enter elsewhere) merging from another tab; a merge refused before Git
//! turns the card to *Not merged into main* with what stopped it, Try again and Open in Review to the attempt page, and
//! the header to Not merged; conflicts offer Resolve, which opens the attempt page; Create pull request is in *More
//! actions*. Needs `git`, as the seed does. `cargo test -p orchester-app --test task_merge_card task_merge_card`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::{path::PathBuf, time::Duration};

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{
  id::{ProjectId, RunId},
  project::TaskKey,
  run::RunNumber,
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's attempts and changes with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// The seeded task of every scenario, as the design names it.
const TITLE: &str = "Add retry to webhook sender";
/// Why Merge waits while a merge waits in the run's worktree.
const WAITS_RESOLVING: &str =
  "Merge waits while Orchester merges into the run's worktree for you to resolve its conflicts.";

struct Page {
  window: Window,
  _stand_ins: StandIns,
  project: ProjectId,
  repo: PathBuf,
  key: TaskKey,
  run: (RunId, RunNumber),
}

impl Page {
  /// The window on the seed's `scenario`, at the Summary of its task, until the top card says `title`.
  fn open(scenario: &str, title: &str) -> Self {
    let data = Data::new();
    let repo = data.dir.path().join(format!("repo-{scenario}"));
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: repo.clone(),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
    window.tree.resize(1440, 1400);
    let db = ProjectDb::open(&repo).expect("the project database");
    let task = db
      .tasks()
      .expect("the tasks")
      .into_iter()
      .find(|task| task.details().title.as_str() == TITLE)
      .expect("the design's task");
    let run = db
      .runs_of_task(task.id())
      .expect("the runs")
      .into_iter()
      .find_map(Result::ok)
      .expect("its run");
    let mut page = Self {
      window,
      _stand_ins: stand_ins,
      project: seeded.project,
      repo,
      key: task.key().clone(),
      run: (run.id(), run.number()),
    };
    page.go(TaskTab::Summary);
    page.until_says("task-notice-title", title);
    page
  }

  /// Opens the task's `tab`.
  fn go(&mut self, tab: TaskTab) {
    let page = ProjectPage::Task(self.key.clone(), TaskView::of(tab));
    self.window.navigation().go(Route::Project(self.project, page));
  }

  /// Until the element `id` says `words`.
  fn until_says(&mut self, id: &str, words: &str) {
    self.window.until_within(GIT_WORK, &format!("{id}: {words}"), |window| {
      window.tree.tick_timers();
      window.texts(id).is_some_and(|texts| texts.join(" ") == words)
    });
  }

  fn has(&mut self, id: &str) -> bool {
    self.window.texts(id).is_some()
  }

  /// The task's review at `/review/<KEY>`, which Review changes opens.
  fn review_page(&self) -> Route {
    Route::Project(self.project, ProjectPage::TaskReview(self.key.clone()))
  }

  /// The run's merge attempts under its task, which Resolve opens.
  fn run_page(&self) -> Route {
    Route::Project(self.project, ProjectPage::TaskMerge(self.key.clone(), self.run.1))
  }

  /// ⌘↵ on macOS, Ctrl+Enter elsewhere, as a person presses them.
  fn merge_keys(&mut self) {
    let (ctrl, meta) = if cfg!(target_os = "macos") {
      (false, true)
    } else {
      (true, false)
    };
    self
      .window
      .tree
      .key_down_with_meta("Enter".to_owned(), "Enter".to_owned(), false, ctrl, false, meta);
    self.window.pass();
  }

  /// The run's worktree, as the seed adds it.
  fn worktree(&self) -> PathBuf {
    self.repo.join(".orchester/worktrees").join(self.run.0.to_string())
  }

  /// No merge of the run began: none was recorded and no attempt either, for as long as one would take to begin.
  fn no_merge_began(&mut self, what: &str) {
    let (run, repo) = (self.run.0, self.repo.clone());
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < until {
      self.window.tree.tick_futures();
      self.window.tree.tick_timers();
      self.window.pass();
      std::thread::sleep(Duration::from_millis(20));
    }
    let db = ProjectDb::open(&repo).expect("the project database");
    assert!(
      db.run_merge(run).expect("the run's merge").is_none(),
      "{what}: nothing merged"
    );
    assert!(
      db.merge_attempts(run).expect("the run's attempts").is_empty(),
      "{what}: no merge was tried"
    );
  }
}

/// `git <args>` in `dir`, as a person with a name; what it printed.
fn git_in(dir: &std::path::Path, args: &[&str]) -> String {
  let output = std::process::Command::new("git")
    .args(["-c", "user.name=Kontur", "-c", "user.email=kontur@example.com"])
    .args(args)
    .current_dir(dir)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn task_merge_card_ready_offers_merge_review_changes_and_close() {
  let mut page = Page::open("task-merge-card-ready", "Ready to merge");
  let meta = page.window.text("task-notice-meta");
  assert_eq!(
    meta,
    format!("{} · every step passed · run {}", page.key, page.run.1.get())
  );
  assert_eq!(page.window.text("task-merge-label"), "Merge");
  assert!(!page.has("task-merge-key"), "no key cap in the button");
  assert_eq!(page.window.text("task-review-changes-label"), "Review changes");
  assert_eq!(
    page.window.text("task-close-without-merging-label"),
    "Close without merging"
  );
  assert_eq!(
    page.window.text("task-ready-actions"),
    "Merge Review changes Close without merging",
    "the card draws exactly 01i's actions"
  );
  for gone in [
    "task-ask-agent",
    "task-create-pull-request",
    "task-review-pull-request",
    "task-ready-columns",
  ] {
    assert!(!page.has(gone), "{gone} is not on the card");
  }
  // The header's Merge is as plain as the card's, on the Summary as on every tab.
  assert_eq!(page.window.text("task-primary-merge-label"), "Merge");
  assert!(!page.has("task-primary-merge-key"), "no key cap in the button");
  page.window.press("task-review-changes");
  let route = page.review_page();
  page.window.until("the run's review page", |window| {
    window.navigation().route_now() == route
  });
}

#[test]
fn task_merge_card_shortcut_merges_from_another_tab() {
  let mut page = Page::open("task-merge-card-ready", "Ready to merge");
  page.go(TaskTab::Changes);
  page.until_says("task-primary-merge-label", "Merge");
  page.merge_keys();
  let (run, repo) = (page.run.0, page.repo.clone());
  page.window.until_within(GIT_WORK, "the merge ⌘↵ asked for", |window| {
    window.tree.tick_timers();
    let db = ProjectDb::open(&repo).expect("the project database");
    let merged = db.run_merge(run).expect("the run's merge").is_some();
    merged || !db.merge_attempts(run).expect("the run's attempts").is_empty()
  });
}

#[test]
fn task_merge_card_header_and_shortcut_wait_while_a_merge_waits_in_the_worktree() {
  let mut page = Page::open("task-merge-card-ready", "Ready to merge");
  // A merge left unfinished in the run's worktree, as one the person resolves in their editor: no resolving job runs.
  // Begun outside Orchester, it is read when the task's page opens again.
  let worktree = page.worktree();
  let side = git_in(&worktree, &["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "side"]);
  git_in(&worktree, &["merge", "--no-ff", "--no-commit", &side]);
  let away = page.run_page();
  page.window.navigation().go(away);
  page
    .window
    .until("another page", |window| window.texts("task-frame").is_none());
  page.go(TaskTab::Summary);
  page.until_says("task-merge-reason", WAITS_RESOLVING);
  page.go(TaskTab::Changes);
  page.until_says("task-primary-merge-label", "Merge");
  page.window.press("task-primary-merge");
  page.merge_keys();
  page.no_merge_began("the header's Merge and ⌘↵");
  git_in(&worktree, &["rev-parse", "--verify", "MERGE_HEAD"]);
}

#[test]
fn task_merge_card_shortcut_does_nothing_while_typing() {
  let mut page = Page::open("task-merge-card-ready", "Ready to merge");
  page.go(TaskTab::Conversation);
  page.until_says("task-primary-merge-label", "Merge");
  page
    .window
    .until("the composer", |window| window.texts("composer-input").is_some());
  page.window.press("composer-input");
  page.merge_keys();
  page.no_merge_began("⌘↵ in the composer");
}

#[test]
fn task_merge_card_refused_says_why_and_opens_the_attempt_page() {
  let mut page = Page::open("task-merge-card-refused", "Not merged into main");
  let meta = page.window.text("task-notice-meta");
  assert!(
    meta.starts_with(&format!("{} · refused at Integration lock · ", page.key)),
    "{meta}"
  );
  let body = page.window.text("task-notice-body");
  assert!(
    body.starts_with("Refused before Git at Integration lock: ") && body.ends_with("Nothing changed."),
    "one line of what stopped it, as the attempt page says it: {body}"
  );
  assert_eq!(page.window.text("task-merge-label"), "Try again");
  assert!(!page.has("task-merge-key"), "no key cap in the button");
  assert_eq!(page.window.text("task-open-in-review-label"), "Open in Review");
  assert!(page.has("task-close-without-merging"));
  assert!(
    page.window.text("task-status").starts_with("Not merged"),
    "the header's status: {}",
    page.window.text("task-status")
  );
  assert_eq!(page.window.text("task-primary-merge-label"), "Try again");
  assert!(!page.has("task-primary-merge-key"), "no key cap in the button");
  page.window.until_within(GIT_WORK, "Properties' Merge row", |window| {
    window
      .texts("task-property-merge-label")
      .is_some_and(|texts| texts.join(" ").starts_with("Not merged · "))
  });
  page.window.press("task-open-in-review");
  let route = page.run_page();
  page.window.until("the run's attempt page", |window| {
    window.navigation().route_now() == route
  });
  page.window.until_within(GIT_WORK, "the attempt page", |window| {
    window.texts("merge-attempt-status").is_some()
  });
}

#[test]
fn task_merge_card_conflicts_resolve_on_the_attempt_page() {
  let mut page = Page::open("task-merge-card-conflicts", "Conflicts with main");
  assert_eq!(page.window.text("task-merge-label"), "Resolve");
  assert!(!page.has("task-merge-key"), "no key cap in the button");
  assert_eq!(page.window.text("task-notice-body"), "Conflicts with main in 2 files.");
  assert!(page.window.text("task-status").starts_with("Not merged"));
  page.window.press("task-merge");
  let route = page.run_page();
  page.window.until("the run's attempt page", |window| {
    window.navigation().route_now() == route
  });
}

#[test]
fn task_merge_card_puts_create_pull_request_in_more_actions() {
  let mut page = Page::open("task-merge-card-ready", "Ready to merge");
  page.window.press("task-more");
  page.window.until_within(GIT_WORK, "Create pull request", |window| {
    window.texts("task-create-pull-request").is_some()
  });
  assert!(page.has("task-pull-request-draft"), "with Open as draft");
}
