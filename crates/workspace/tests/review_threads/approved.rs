//! An approved run on the review page (ORC-800): it shows its reviewed diff with its threads kept, read-only, as merged
//! with who approved it and when; a merge nobody is known to have approved says so.

use orchester_core::{
  integration::{
    MergeKind, RunMerge,
    attempt::{MergeAsker, MergeVia},
  },
  merge_queue::QueueState,
  review::ThreadState,
  run::Answerer,
  store::MergeStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  merge_queue,
  tasks::{
    changes::Checkpointed,
    data::ProjectTasks,
    integration::merge_in_as,
    review_threads::{
      self,
      approved_run::{Approval, ApprovedRun},
      page::{ReviewPage, task_review_page},
    },
    worktree_removal::WorktreeRemover,
  },
};

use super::support::{Repo, at, body, git};

fn page(repo: &Repo) -> Option<ReviewPage> {
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(repo.root(), &programs).expect("the tasks read");
  let key = tasks.tasks[0].key().clone();
  task_review_page(&tasks, (repo.root(), &programs), &key).expect("the page reads")
}

fn merge(repo: &Repo, asked: (MergeAsker, MergeVia)) {
  let remover = WorktreeRemover::here_for_test(repo.root(), Programs::processes());
  merge_in_as(
    &remover,
    &Programs::processes(),
    (repo.run, Some(&repo.head), None),
    asked,
  )
  .expect("merged");
}

#[test]
fn review_threads_page_a_merged_run_keeps_its_diff_and_threads_read_only() {
  let repo = Repo::new();
  review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  let before = Timestamp::now();
  merge(&repo, (MergeAsker::Person, MergeVia::TaskPage));

  let page = page(&repo).expect("a merged run is reviewed read-only, not 'nothing to review'");
  assert_eq!(page.run.run, repo.run);
  assert_eq!(page.review_in, None, "it waits for no one");
  let Some(ApprovedRun {
    approval: Approval::Person(approved),
    landed: Some(landed),
  }) = page.approved.clone()
  else {
    panic!("approved by the person and landed: {:?}", page.approved);
  };
  assert!(approved >= before);
  assert_eq!(approved, landed.at);
  assert_eq!(landed.revision.as_str(), git(repo.root(), &["rev-parse", "main"]));
  assert_eq!(landed.kind, MergeKind::FastForward);
  assert_eq!(landed.into.as_str(), "main");
  assert!(page.since_review.is_empty());

  assert_eq!(page.review.head.as_ref(), Some(&repo.head));
  let Checkpointed::Read { changes, .. } = &page.changes else {
    panic!("the reviewed diff is read");
  };
  assert_eq!(changes.diff.files()[0].path(), "src/lib.rs");
  assert_eq!(page.review.threads.len(), 1, "the comment is kept");
  assert_eq!(page.review.threads[0].state, ThreadState::NotSent);
  assert_eq!(page.agent().map(|(step, _)| step), Some("Implement"));
}

#[test]
fn review_threads_page_a_merge_by_automation_is_not_called_approved_by_a_person() {
  let repo = Repo::new();
  merge(&repo, (MergeAsker::Automation, MergeVia::MergeRun));
  let page = page(&repo).expect("shown read-only");
  let approved = page.approved.expect("merged");
  assert_eq!(approved.approval, Approval::Automatic);
  assert!(approved.landed.is_some());
}

#[test]
fn review_threads_page_a_merge_recorded_without_its_attempt_says_it_was_not_recorded() {
  let repo = Repo::new();
  let merged = RunMerge {
    run: repo.run,
    into: orchester_core::run::BranchName::new("main").expect("branch"),
    revision: repo.head.clone(),
    kind: MergeKind::MergeCommit,
    merged_at: at(30),
  };
  repo.db().save_run_merge(&merged).expect("recorded");
  let page = page(&repo).expect("shown read-only");
  assert_eq!(page.approved.expect("merged").approval, Approval::NotRecorded);
}

#[test]
fn review_threads_page_a_run_nobody_approved_is_not_approved() {
  let repo = Repo::new();
  let page = page(&repo).expect("it waits for review");
  assert_eq!(page.approved, None);
  assert!(page.review_in.is_some());
}

#[test]
fn review_threads_page_a_run_approved_in_the_merge_queue_is_merging_until_the_queue_merges_it() {
  let repo = Repo::new();
  let mut db = repo.db();
  let task = repo.stored().task();
  assert!(db.join_merge_queue((repo.run, task), at(10), at(11)).expect("joined"));
  let mut entry = db.merge_queue_entry(repo.run).expect("read").expect("an entry");
  entry.state = QueueState::WaitingForApproval;
  assert!(
    db.save_merge_queue_entry(&entry, &QueueState::Queued, (&[], at(12)))
      .expect("saved")
  );
  assert_eq!(
    page(&repo).expect("waits for review").approved,
    None,
    "not approved yet"
  );

  merge_queue::approve(repo.root(), repo.run, at(13), Answerer::Person).expect("approved");
  let page = page(&repo).expect("read-only while the queue delivers it");
  assert_eq!(
    page.approved,
    Some(ApprovedRun {
      approval: Approval::Person(at(13)),
      landed: None
    })
  );
  assert_eq!(page.review_in, None);
}
