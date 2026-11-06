//! The Review list (ORC-791): the groups and the states of their rows, against a real repository. `cargo test -p
//! orchester-workspace --test review_list`.

#[path = "review_list/second.rs"]
mod second;
#[path = "review_threads/support.rs"]
mod support;

use std::{thread::sleep, time::Duration};

use orchester_core::{
  diff::Changes,
  integration::MergeError,
  merge_queue::{Left, QueueState},
  run::ReviewNote,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  data::ProjectTasks,
  integration::{IntegrationProblem, merge_in},
  resolve::RunWorktrees,
  review_list::{ReviewList, ReviewRow, ReviewState, review_list, waiting_count},
  review_threads::{self, approve::approve_and_merge, send},
  worktree_removal::WorktreeRemover,
};
use support::{Repo, at, body, git};

fn list_at(repo: &Repo, now: Timestamp) -> (ReviewList, usize) {
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(repo.root(), &programs).expect("the tasks read");
  let list = review_list(&tasks, (repo.root(), &programs), now).expect("the list reads");
  let count = waiting_count(&tasks);
  // The sidebar's count and the page's bones are *Waiting for you*'s length: they never disagree.
  assert_eq!(count, list.waiting.len(), "{list:?}");
  (list, count)
}

fn list(repo: &Repo) -> (ReviewList, usize) {
  list_at(repo, Timestamp::now())
}

fn only(rows: &[ReviewRow]) -> &ReviewRow {
  assert_eq!(rows.len(), 1, "{rows:?}");
  &rows[0]
}

fn remover(repo: &Repo) -> WorktreeRemover {
  WorktreeRemover::here_for_test(repo.root(), Programs::processes())
}

fn approve(repo: &Repo, run: orchester_core::id::RunId, head: &orchester_core::revision::Revision) {
  approve_and_merge(&remover(repo), &Programs::processes(), (run, head), Timestamp::now()).expect("merged");
}

#[test]
fn review_list_a_run_that_passed_every_step_waits_for_you_with_what_it_changes() {
  let repo = Repo::new();
  let (list, count) = list(&repo);
  assert_eq!(count, 1);
  assert!(list.with_agent.is_empty() && list.approved_today.is_empty());
  let row = only(&list.waiting);
  assert_eq!(row.key.to_string(), "REV-1");
  assert_eq!(&*row.title, "Add numbers");
  assert_eq!(row.run, repo.run);
  assert_eq!(row.number, repo.stored().number());
  assert_eq!(row.state, ReviewState::Ready);
  assert_eq!(row.open_threads, 0);
  assert_eq!(row.tag, None);
  assert_eq!(
    row.size,
    Some(Changes {
      files: 1,
      additions: 3,
      deletions: 0
    })
  );
}

#[test]
fn review_list_comments_not_sent_count_and_open_threads_fold_when_resolved() {
  let repo = Repo::new();
  let first =
    review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  review_threads::comment(repo.root(), repo.run, repo.anchor(2, 2), &body("Rename two."), at(21)).expect("kept");
  let (list_now, _) = list(&repo);
  let row = only(&list_now.waiting);
  assert_eq!(row.state, ReviewState::CommentsNotSent { count: 2 });
  assert_eq!(row.open_threads, 2);
  review_threads::resolve(repo.root(), first.id(), Some(at(22))).expect("resolved");
  let (list_now, _) = list(&repo);
  let row = only(&list_now.waiting);
  assert_eq!(row.state, ReviewState::CommentsNotSent { count: 1 });
  assert_eq!(row.open_threads, 1);
}

#[test]
fn review_list_changed_since_your_review_counts_the_commits_after_the_reviewed_head() {
  let repo = Repo::new();
  repo.commit("fn one() {}\nfn two() {}\nfn three() {}\nfn four() {}\n");
  repo.commit("fn one() {}\nfn two() {}\nfn three() {}\nfn four() {}\nfn five() {}\n");
  let (list, count) = list(&repo);
  assert_eq!(count, 1);
  assert_eq!(
    only(&list.waiting).state,
    ReviewState::ChangedSinceReview { commits: 2 }
  );
}

#[test]
fn review_list_conflicts_with_the_base_name_the_files_and_wait_from_the_attempt() {
  let repo = Repo::new();
  std::fs::write(repo.root().join("src/lib.rs"), "fn one() { main }\n").expect("write");
  git(repo.root(), &["commit", "-am", "main changed it too"]);
  let before = Timestamp::now();
  let problem = merge_in(&remover(&repo), &Programs::processes(), (repo.run, None)).expect_err("conflicts");
  assert!(
    matches!(problem, IntegrationProblem::Merge(MergeError::Conflict { .. })),
    "{problem:?}"
  );
  let (list, count) = list(&repo);
  assert_eq!(count, 1, "the approved run still waits for you");
  let row = only(&list.waiting);
  assert_eq!(
    row.state,
    ReviewState::Conflicts {
      base: repo.stored().base_branch().cloned(),
      files: 1
    }
  );
  assert!(
    row.since >= before,
    "it waits since the attempt, not since the run finished"
  );
}

#[test]
fn review_list_a_run_the_queue_took_out_with_conflicts_waits_with_its_files() {
  let repo = Repo::new();
  let mut db = repo.db();
  let task = repo.stored().task();
  assert!(db.join_merge_queue((repo.run, task), at(8), at(9)).expect("joined"));
  let mut entry = db.merge_queue_entry(repo.run).expect("read").expect("entry");
  let files = vec!["a.rs".to_owned(), "b.rs".to_owned()];
  let expected = std::mem::replace(&mut entry.state, QueueState::Left(Left::Conflicts { files }));
  entry.since = at(10);
  assert!(
    db.save_merge_queue_entry(&entry, &expected, (&[], at(10)))
      .expect("saved")
  );
  let (list, count) = list(&repo);
  assert_eq!(count, 1);
  let row = only(&list.waiting);
  assert_eq!(row.since, at(10));
  assert!(
    matches!(&row.state, ReviewState::Conflicts { files: 2, .. }),
    "{:?}",
    row.state
  );
}

#[test]
fn review_list_waiting_runs_are_listed_oldest_wait_first() {
  let repo = Repo::new();
  let later = second::run(&repo, 30);
  let (list, count) = list(&repo);
  assert_eq!(count, 2);
  let runs: Vec<_> = list.waiting.iter().map(|row| row.run).collect();
  assert_eq!(runs, [repo.run, later]);
}

#[test]
fn review_list_a_run_sent_back_leaves_waiting_for_the_agent() {
  let repo = Repo::new();
  review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  review_threads::comment(repo.root(), repo.run, repo.anchor(2, 2), &body("Rename two."), at(21)).expect("kept");
  let sending = send::begin(
    repo.root(),
    &RunWorktrees::here_for_test(repo.root(), Programs::processes()),
    repo.run,
  )
  .expect("sending");
  send::sent(repo.root(), &sending, Ok(()), at(22)).expect("sent");
  repo.return_to_implement(&ReviewNote::new("Rename.").expect("note"), at(23));
  let (list, count) = list(&repo);
  assert_eq!(count, 0, "Implement works on it");
  assert!(list.waiting.is_empty());
  let row = only(&list.with_agent);
  assert_eq!(row.run, repo.run);
  assert_eq!(row.state, ReviewState::ImplementOn { comments: 2 });
  assert_eq!(row.open_threads, 2);
  assert_eq!(row.since, at(22));
}

#[test]
fn review_list_approving_moves_the_task_to_approved_today_and_only_for_today() {
  let repo = Repo::new();
  approve(&repo, repo.run, &repo.head);
  let now = Timestamp::now();
  let (list, count) = list_at(&repo, now);
  assert_eq!(count, 0);
  assert!(list.waiting.is_empty() && list.with_agent.is_empty());
  let row = only(&list.approved_today);
  assert_eq!(row.run, repo.run);
  assert_eq!(
    row.state,
    ReviewState::Merged {
      base: repo.stored().base_branch().cloned()
    }
  );
  assert!(row.size.is_some());
  let later = Timestamp::from_unix_millis(now.unix_millis() + 2 * 24 * 60 * 60 * 1000);
  assert!(list_at(&repo, later).0.approved_today.is_empty());
}

#[test]
fn review_list_approved_today_lists_the_newest_approval_first() {
  let repo = Repo::new();
  let other = second::run(&repo, 30);
  approve(&repo, repo.run, &repo.head);
  sleep(Duration::from_millis(20));
  approve(&repo, other, &second::head(&repo, other));
  let (list, count) = list(&repo);
  assert_eq!(count, 0);
  let runs: Vec<_> = list.approved_today.iter().map(|row| row.run).collect();
  assert_eq!(runs, [other, repo.run]);
}
