//! The merge queue (ORC-19) in the app's real window, on the QA seed's delivery scenarios (`examples/seed`): a task's
//! Summary while the queue delivers its run: Review before merging with its changes, commits and checks and *Merge into
//! main*, which the queue then merges; Integrate failed with *Resolve myself*, which leaves the merge in the run's
//! worktree, and *I've resolved it* with markers left; a run merged by the queue, and its conversation telling its
//! delivery. The queue's driver runs as in the app, with git. Needs `git`, as the seed does.

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

  fn summary(&mut self, key: &str) {
    let key = key.parse().expect("a task key");
    self.go(ProjectPage::Task(key, TaskView::of(TaskTab::Summary)));
  }

  fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
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

fn merged(repo: &Path, number: u32) -> bool {
  entry_of(repo, number).is_some_and(|entry| matches!(entry.state, QueueState::Merged(_)))
}
