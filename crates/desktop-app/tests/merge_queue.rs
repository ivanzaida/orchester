//! The merge queue (ORC-19) in the app's real window, on the QA seed's delivery scenarios (`examples/seed`): the Merge
//! queue screen with its rows, its blocked notice and its sidebar count, and a task's Summary while the queue delivers
//! its run: Review before merging with its changes, commits and checks and *Merge into main*, which the queue then
//! merges; Integrate failed with *Resolve myself*, which leaves the merge in the run's worktree, and *I've resolved it*
//! with markers left; a run merged by the queue, and its conversation telling its delivery. A lane per base branch
//! (ORC-410, `merge_queue_lanes`): each with its head, how it lands and its runs, and a run waiting in one lane holding
//! no other. The queue's driver runs as in the app, with git. Needs `git`, as the seed does.

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
    self
      .window
      .until("the merge queue", |window| window.texts("merge-queue-header").is_some());
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
  assert!(waiting.contains("Waiting for your approval"), "{waiting}");
  assert!(waiting.contains("Typed errors per module"), "{waiting}");
  assert!(seeded.shows("merge-queue-row-2-review"), "Review & merge is offered");
  let left = seeded.window.text("merge-queue-row-3");
  assert!(left.contains("Integrate couldn't resolve 2 files"), "{left}");
  let merged = seeded.window.text("merge-queue-row-1");
  assert!(merged.contains("Merged"), "{merged}");
  // Only the run waiting for approval is still in the queue: the sidebar's Review and the hub's tab count it.
  for counted in ["nav-review", "review-hub-tab-merge_queue"] {
    seeded.window.until(counted, |window| {
      window
        .texts(counted)
        .is_some_and(|texts| texts.iter().any(|text| text == "1"))
    });
  }
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
  assert_eq!(checkout, "on main · 1 uncommitted file");
  let first = seeded.window.text("merge-queue-row-1");
  assert!(first.contains("src/webhooks/settings.rs"), "{first}");
  let held = seeded.window.text("merge-queue-row-2");
  assert!(held.contains("Held until the checkout is clean"), "{held}");
  // Retry while the checkout is still dirty leaves the run blocked.
  seeded.window.press("merge-queue-retry");
  seeded
    .window
    .until("the retry", |window| window.texts("merge-queue-blocked").is_some());
  assert!(matches!(seeded.entry(1).state, QueueState::Blocked(_)));
  discard(&seeded.repo, "src/webhooks/settings.rs");
  seeded.window.press("merge-queue-retry");
  let repo = seeded.repo.clone();
  seeded
    .window
    .until_within(GIT_WORK, "the blocked run to merge", |_| merged(&repo, 1));
}

#[test]
fn merge_queue_removes_a_waiting_run_which_then_merges_by_hand() {
  let mut seeded = Seeded::new("delivery-blocked");
  seeded.queue_screen();
  seeded.window.until("the waiting run", |window| {
    window.texts("merge-queue-row-2-remove").is_some()
  });
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
  seeded
    .window
    .until("the row to go", |window| window.texts("merge-queue-row-2").is_none());
  seeded.summary("KON-2");
  seeded
    .window
    .until("Ready for review", |window| window.texts("task-merge").is_some());
}

/// Waits for the lane of `branch` and gives the texts of its head and of its rows.
fn lane(seeded: &mut Seeded, branch: &str) -> (String, String) {
  let id = format!("merge-queue-lane-{branch}");
  seeded
    .window
    .until(&id, |window| window.texts(&format!("{id}-rows")).is_some());
  (
    seeded.window.text(&format!("{id}-head")),
    seeded.window.text(&format!("{id}-rows")),
  )
}

#[test]
fn merge_queue_lanes_show_a_lane_per_base_branch_with_its_head_how_it_lands_and_its_runs() {
  let mut seeded = Seeded::new("delivery-lanes");
  seeded.queue_screen();
  let (main_head, main_rows) = lane(&mut seeded, "main");
  assert!(main_head.starts_with("Into main"), "{main_head}");
  assert!(main_head.ends_with("merges in the checkout"), "{main_head}");
  for title in ["Retry budget per endpoint", "Honor Retry-After on 429 and 503"] {
    assert!(main_rows.contains(title), "{main_rows}");
  }
  assert!(!main_rows.contains("Add retry to webhook sender"), "{main_rows}");
  let (release_head, release_rows) = lane(&mut seeded, "release/2.0");
  assert_eq!(
    release_head, "Into release/2.0 1 lands beside the checkout: it is on main",
    "the run waiting for approval is counted in its own lane only"
  );
  assert!(release_rows.contains("Add retry to webhook sender"), "{release_rows}");
  assert!(
    release_rows.contains("Waiting for your approval · checked on release/2.0 @"),
    "{release_rows}"
  );
  assert!(!release_rows.contains("Retry budget per endpoint"), "{release_rows}");
  assert!(seeded.shows("merge-queue-row-1-review"), "Review & merge is offered");
  // main's lane comes first: its run is delivered now.
  let mut top = |id: &str| {
    seeded
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|lane| lane.bounds())
      .map(|bounds| bounds.y)
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  assert!(
    top("merge-queue-lane-main") < top("merge-queue-lane-release/2.0"),
    "main's lane is above release/2.0's"
  );
  // The note names a run waiting for the person and another lane: release/2.0's run, or main's first once the queue
  // delivered it to the person too.
  let note = seeded.window.text("merge-queue-note");
  assert!(
    note.contains("holds nothing in main's lane") || note.contains("holds nothing in release/2.0's lane"),
    "{note}"
  );
}

#[test]
fn merge_queue_lanes_a_run_waiting_for_approval_in_one_lane_holds_no_other() {
  let mut seeded = Seeded::new("delivery-lanes");
  seeded.queue_screen();
  let repo = seeded.repo.clone();
  // main's first run is synced and checked while release/2.0's waits for the person.
  seeded
    .window
    .until_within(GIT_WORK, "main's first run to be delivered", |_| {
      entry_state(&repo, 2) == Some(QueueState::WaitingForApproval)
    });
  assert_eq!(seeded.entry(1).state, QueueState::WaitingForApproval);
  seeded.window.until("main's lane to show it", |window| {
    window.texts("merge-queue-row-2-state-line").is_some_and(|texts| {
      texts
        .iter()
        .any(|text| text.starts_with("Waiting for your approval · checked on main"))
    })
  });
  let (_, release_rows) = lane(&mut seeded, "release/2.0");
  assert!(release_rows.contains("Add retry to webhook sender"), "{release_rows}");
}

#[test]
fn merge_queue_review_before_merging_approves_and_the_queue_merges_the_run() {
  let mut seeded = Seeded::new("delivery");
  seeded.summary("KON-2");
  seeded
    .window
    .until("the review", |window| window.texts("task-review").is_some());
  assert_eq!(
    seeded.window.text("task-delivery-decision-title"),
    "Ready to merge into main"
  );
  assert!(seeded.shows("task-delivery-open-changes"));
  assert!(seeded.shows("task-delivery-reject"));
  // The delivery's rows are among the run's other facts.
  seeded.window.press("task-facts-more");
  let details = seeded.window.text("task-facts");
  assert!(details.contains("Approve to merge"), "{details}");
  let meta = seeded.window.text("task-review-meta");
  assert!(!meta.contains("vv"), "the pipeline's version is named once: {meta}");
  // Changes and the run's commits beside the checks (`wN5Bz`).
  seeded.window.until_within(GIT_WORK, "the run's commits", |window| {
    window.texts("task-review-commits-count").is_some()
  });
  let count = seeded.window.text("task-review-commits-count");
  assert!(count.parse::<usize>().is_ok_and(|count| count > 0), "{count}");
  assert!(seeded.window.text("task-ready-changes").starts_with("Changes"));
  assert!(seeded.window.text("task-ready-verification").starts_with("Checks"));
  seeded.window.press("task-delivery-merge");
  let repo = seeded.repo.clone();
  seeded.window.until("the approval", |_| {
    entry_of(&repo, 2).is_some_and(|entry| entry.approved_at.is_some())
  });
  seeded
    .window
    .until_within(GIT_WORK, "the queue to merge the run", |_| merged(&repo, 2));
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
  seeded.window.until("Integrate failed", |window| {
    window.texts("task-delivery-conflicts").is_some()
  });
  assert_eq!(seeded.window.text("task-delivery-decision-title"), "Conflicts need you");
  let conflicts = seeded.window.text("task-delivery-conflicts");
  assert!(conflicts.contains("src/webhooks/sender.rs"), "{conflicts}");
  seeded.window.press("task-delivery-resolve-myself");
  seeded
    .window
    .until_within(GIT_WORK, "the worktree to hold the merge", |window| {
      window.texts("task-delivery-resolve").is_some()
    });
  assert!(seeded.shows("task-delivery-resolved"), "I've resolved it is offered");
  // A long worktree path is cut, so *Copy path* stays in the window (`h6jKOa`).
  let copy = seeded
    .window
    .tree
    .get_element_by_id_mut("task-delivery-copy-path")
    .and_then(|element| element.bounds())
    .expect("Copy path is laid out");
  assert!(
    copy.x + copy.width <= 1440.0,
    "Copy path ends at {}",
    copy.x + copy.width
  );
  // The markers git left in the files are still there: the checks say so (`e7geY3`).
  seeded.window.press("task-delivery-resolved");
  seeded.window.until_within(GIT_WORK, "the markers left", |window| {
    window.texts("task-delivery-resolve-note").is_some()
  });
  assert_eq!(
    seeded.window.text("task-delivery-resolve-note"),
    "Remove the markers, then press it again."
  );
  let markers = seeded.window.text("task-delivery-resolve-markers-detail");
  assert!(markers.contains("src/webhooks/sender.rs"), "{markers}");
  assert_eq!(seeded.window.text("task-delivery-resolve-merges-detail"), "Not checked");
  assert!(
    !seeded.shows("task-delivery-refused"),
    "the checks say it, not an error line"
  );
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
  // The delivery's rows are among the run's other facts.
  seeded.window.press("task-facts-more");
  let details = seeded.window.text("task-facts");
  assert!(details.contains("Merged at"), "{details}");
}

#[test]
fn merge_queue_conversation_tells_the_runs_delivery_after_its_steps() {
  let mut seeded = Seeded::new("delivery");
  let key = "KON-1".parse().expect("a task key");
  seeded.go(ProjectPage::Task(key, TaskView::of(TaskTab::Conversation)));
  // The chat opens at its end: the merge, what was cleared and the run's end (`Urfxj`).
  seeded.window.until("the queue's lines", |window| {
    window.texts("conversation-chat").is_some_and(|texts| {
      texts
        .iter()
        .any(|text| text.starts_with("Merged into main · merge commit"))
        && texts.iter().any(|text| text == "Removed the run's worktree")
    })
  });
  let chat = seeded.window.texts("conversation-chat").unwrap_or_default();
  assert!(
    chat
      .iter()
      .any(|text| text.starts_with("You approved the merge into main")),
    "{chat:?}"
  );
  assert!(
    !chat.iter().any(|text| text.starts_with("Deleted branch")),
    "the seed kept the branch: {chat:?}"
  );
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
