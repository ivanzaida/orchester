//! The merge queue (ORC-19) in the app's real window, on the QA seed's delivery scenarios (`examples/seed`): the Merge
//! queue screen with its rows, its blocked notice and its sidebar count, and a task's Summary while the queue delivers
//! its run: Review before merging with *Merge into main*, which the queue then merges; Integrate failed with *Resolve
//! myself*, which leaves the merge in the run's worktree; and a run merged by the queue. The queue's driver runs as in
//! the app, with git. Needs `git`, as the seed does.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
mod support;

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{
  id::ProjectId,
  merge_queue::{QueueEntry, QueueState},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window},
};

/// How long the queue may take to sync, check and merge with git on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);

struct Seeded {
  window: Window,
  project: ProjectId,
  repo: PathBuf,
}

impl Seeded {
  fn new(scenario: &str) -> Self {
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
    window.tree.resize(1440, 1400);
    Self {
      window,
      project: seeded.project,
      repo: seeded.repo,
    }
  }

  fn go(&mut self, page: ProjectPage) {
    self.window.navigation().go(Route::Project(self.project, page));
    self.window.pass();
  }

  fn queue_screen(&mut self) {
    self.go(ProjectPage::MergeQueue);
    self.window.until("the merge queue", |window| {
      window.texts("merge-queue-header").is_some()
    });
  }

  fn summary(&mut self, key: &str) {
    let key = key.parse().expect("a task key");
    self.go(ProjectPage::Task(key, TaskView::of(TaskTab::Summary)));
  }

  /// The entry of the run numbered `number` in the project's queue.
  fn entry(&self, number: u32) -> QueueEntry {
    entry_of(&self.repo, number).unwrap_or_else(|| panic!("run #{number} is in the queue"))
  }

  fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
}

#[test]
fn merge_queue_lists_runs_waiting_for_approval_left_and_merged_today_and_counts_those_in_it() {
  let mut seeded = Seeded::new("delivery");
  seeded.queue_screen();
  for row in ["merge-queue-row-1", "merge-queue-row-2", "merge-queue-row-3"] {
    seeded.window.until(row, |window| window.texts(row).is_some());
  }
  let waiting = seeded.window.text("merge-queue-row-2");
  assert!(waiting.contains("Waiting for approval"), "{waiting}");
  assert!(waiting.contains("Typed errors per module"), "{waiting}");
  assert!(seeded.shows("merge-queue-row-2-review"), "Review & merge is offered");
  let left = seeded.window.text("merge-queue-row-3");
  assert!(left.contains("Integrate couldn't resolve 2 files"), "{left}");
  let merged = seeded.window.text("merge-queue-row-1");
  assert!(merged.contains("Merged"), "{merged}");
  // Only the run waiting for approval is still in the queue.
  seeded.window.until("the sidebar's count", |window| {
    window
      .texts("nav-merge_queue")
      .is_some_and(|texts| texts.iter().any(|text| text == "1"))
  });
}

#[test]
fn merge_queue_blocked_by_a_dirty_checkout_says_why_and_goes_on_once_it_is_clean() {
  let mut seeded = Seeded::new("delivery-blocked");
  seeded.queue_screen();
  seeded.window.until("the blocked notice", |window| {
    window.texts("merge-queue-blocked").is_some()
  });
  let notice = seeded.window.text("merge-queue-blocked");
  assert!(notice.contains("uncommitted changes"), "{notice}");
  let checkout = seeded.window.text("merge-queue-checkout-state");
  assert_eq!(checkout, "· 1 uncommitted file");
  let first = seeded.window.text("merge-queue-row-1");
  assert!(first.contains("src/webhooks/settings.rs"), "{first}");
  let held = seeded.window.text("merge-queue-row-2");
  assert!(held.contains("Held until the checkout is clean"), "{held}");
  // Retry while the checkout is still dirty leaves the run blocked.
  seeded.window.press("merge-queue-retry");
  seeded.window.until("the retry", |window| window.texts("merge-queue-blocked").is_some());
  assert!(matches!(seeded.entry(1).state, QueueState::Blocked(_)));
  discard(&seeded.repo, "src/webhooks/settings.rs");
  seeded.window.press("merge-queue-retry");
  let repo = seeded.repo.clone();
  seeded.window.until_within(GIT_WORK, "the blocked run to merge", |_| {
    merged(&repo, 1)
  });
}

#[test]
fn merge_queue_removes_a_waiting_run_which_then_merges_by_hand() {
  let mut seeded = Seeded::new("delivery-blocked");
  seeded.queue_screen();
  seeded
    .window
    .until("the waiting run", |window| window.texts("merge-queue-row-2-remove").is_some());
  // A long checkout path keeps the head its size: the rows stay in view (the base box shortens the path).
  let header = seeded
    .window
    .tree
    .get_element_by_id_mut("merge-queue-header")
    .and_then(|element| element.bounds())
    .expect("the head is laid out");
  assert!(header.height < 120.0, "the head is {} tall", header.height);
  seeded.window.press("merge-queue-row-2-remove");
  let repo = seeded.repo.clone();
  seeded.window.until("the run to leave the queue", |_| {
    entry_state(&repo, 2) == Some(QueueState::Removed)
  });
  seeded.window.until("the row to go", |window| window.texts("merge-queue-row-2").is_none());
  seeded.summary("KON-2");
  seeded
    .window
    .until("Ready for review", |window| window.texts("task-merge").is_some());
}

#[test]
fn merge_queue_review_before_merging_approves_and_the_queue_merges_the_run() {
  let mut seeded = Seeded::new("delivery");
  seeded.summary("KON-2");
  seeded.window.until("the review", |window| window.texts("task-review").is_some());
  assert_eq!(
    seeded.window.text("task-delivery-decision-title"),
    "Ready to merge into main"
  );
  assert!(seeded.shows("task-delivery-open-changes"));
  assert!(seeded.shows("task-delivery-reject"));
  let details = seeded.window.text("task-run-details");
  assert!(details.contains("Approve to merge"), "{details}");
  seeded.window.press("task-delivery-merge");
  let repo = seeded.repo.clone();
  seeded
    .window
    .until("the approval", |_| entry_of(&repo, 2).is_some_and(|entry| entry.approved_at.is_some()));
  seeded.window.until_within(GIT_WORK, "the queue to merge the run", |_| merged(&repo, 2));
  seeded.window.until("the merged notice", |window| {
    window
      .texts("task-notice")
      .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Merged into main")))
  });
}

#[test]
fn merge_queue_integrate_failed_offers_resolve_myself_which_leaves_the_merge_in_the_worktree() {
  let mut seeded = Seeded::new("delivery");
  seeded.summary("KON-3");
  seeded
    .window
    .until("Integrate failed", |window| window.texts("task-delivery-conflicts").is_some());
  assert_eq!(seeded.window.text("task-delivery-decision-title"), "Conflicts need you");
  let conflicts = seeded.window.text("task-delivery-conflicts");
  assert!(conflicts.contains("src/webhooks/sender.rs"), "{conflicts}");
  seeded.window.press("task-delivery-resolve-myself");
  seeded.window.until_within(GIT_WORK, "the worktree to hold the merge", |window| {
    window.texts("task-delivery-resolve").is_some()
  });
  assert!(seeded.shows("task-delivery-resolved"), "I've resolved it is offered");
  seeded.window.press("task-delivery-back");
  seeded.window.until("the decision again", |window| {
    window
      .texts("task-delivery-resolve-myself")
      .is_some_and(|texts| texts.iter().any(|text| text == "Continue resolving"))
  });
}

#[test]
fn merge_queue_a_run_it_merged_says_where_its_work_went() {
  let mut seeded = Seeded::new("delivery");
  seeded.summary("KON-1");
  seeded.window.until("the merged notice", |window| {
    window
      .texts("task-notice")
      .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Merged into main")))
  });
  assert!(seeded.shows("task-stage-deliver"));
  let details = seeded.window.text("task-run-details");
  assert!(details.contains("Merged at"), "{details}");
}

fn entry_of(repo: &Path, number: u32) -> Option<QueueEntry> {
  let db = ProjectDb::open(repo).ok()?;
  db.merge_queue().ok()?.into_iter().find(|entry| {
    db.run(entry.run)
      .ok()
      .flatten()
      .is_some_and(|run| run.number().get() == number)
  })
}

fn entry_state(repo: &Path, number: u32) -> Option<QueueState> {
  entry_of(repo, number).map(|entry| entry.state)
}

fn merged(repo: &Path, number: u32) -> bool {
  matches!(entry_state(repo, number), Some(QueueState::Merged(_)))
}

/// Discards the change to `file` in the checkout at `repo`, as a person would.
fn discard(repo: &Path, file: &str) {
  let status = std::process::Command::new("git")
    .args(["checkout", "--", file])
    .current_dir(repo)
    .status()
    .expect("git runs");
  assert!(status.success(), "the change is discarded");
}
