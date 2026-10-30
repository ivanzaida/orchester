//! The merge queue (ORC-19, `orchester_workspace::merge_queue`) against a real git repository: runs of a pipeline
//! that ends in a Deliver step join it and deliver one at a time, first finished first (synced with `main`, checked
//! again on the merged result, merged, cleared); a person moves waiting runs up or to the top; a base that moves during
//! the check means syncing and checking again, at most three times in a row, and a run is never merged on a check of
//! an older base; conflicts go to the Integrate step, and when it fails nothing reaches `main`; a dirty checkout
//! blocks the queue until it is clean; a run waiting for approval does not hold later runs; a pipeline without a
//! Deliver step keeps its runs out of the queue. The check is a command tool whose program the tests answer for
//! (`harness`). `cargo test -p orchester-workspace --test merge_queue merge_queue`.

mod conversation;
mod decisions;
mod harness;

use std::{fs, sync::Arc};

use orchester_core::{
  integration::base::RunBaseSource,
  merge_queue::{Blocked, Left, QueueEvent, QueueState},
  revision::Revision,
  run::{Answerer, Run},
  store::MergeStore,
  task::Task,
};
use orchester_ipc::Command as RunnerCommand;
use orchester_store::ProjectDb;
use orchester_workspace::merge_queue::{self, Advance, Move};
use parking_lot::Mutex;

use self::harness::{Repo, at, git};

#[test]
fn merge_queue_delivers_one_run_at_a_time_first_finished_first_and_clears_after_it() {
  let mut repo = Repo::new();
  let (first, second) = (repo.task("Sign in"), repo.task("Sign out"));
  let later = repo.succeeded(
    (&second, "auto"),
    "orchester/orc-2-run-1",
    ("other.txt", "second\n"),
    200,
  );
  let earlier = repo.succeeded(
    (&first, "auto"),
    "orchester/orc-1-run-2",
    ("shared.txt", "first\n"),
    100,
  );
  let tips = (
    git(repo.root(), &["rev-parse", "orchester/orc-1-run-2"]),
    git(repo.root(), &["rev-parse", "orchester/orc-2-run-1"]),
  );

  repo.drive();

  let QueueState::Merged(merged_first) = repo.state(&earlier) else {
    panic!("{:?}", repo.state(&earlier));
  };
  let QueueState::Merged(merged_second) = repo.state(&later) else {
    panic!("{:?}", repo.state(&later));
  };
  assert!(repo.main_holds(&tips.0) && repo.main_holds(&tips.1));
  let order = git(repo.root(), &["log", "--first-parent", "--format=%H", "-2", "main"]);
  let order: Vec<&str> = order.lines().collect();
  assert_eq!(
    order,
    [merged_second.revision.as_str(), merged_first.revision.as_str()],
    "first finished, first merged"
  );
  assert!(
    merged_first.worktree_removed && merged_first.branch_deleted,
    "{merged_first:?}"
  );
  assert!(!repo.worktree(&earlier).exists());
  assert_eq!(repo.checks(), 2, "Check ran once on each merged result");
  let db = ProjectDb::open(repo.root()).expect("open");
  assert!(
    db.run_merge(earlier.id()).expect("merge").is_some(),
    "the merge is recorded, so the task is done"
  );
  let events = repo.events(&later);
  assert!(
    matches!(events.as_slice(), [QueueEvent::Joined, QueueEvent::Synced { files, .. }, QueueEvent::CheckStarted { .. }, QueueEvent::CheckPassed { .. }, QueueEvent::Merged { .. }, QueueEvent::CleanedUp { .. }] if files.is_empty()),
    "{events:?}"
  );
}

#[test]
fn merge_queue_squashes_into_one_commit_titled_by_the_task_and_deletes_the_branch() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "squash"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  repo.drive();
  let QueueState::Merged(merged) = repo.state(&run) else {
    panic!("{:?}", repo.state(&run));
  };
  assert_eq!(
    git(repo.root(), &["log", "-1", "--format=%s", "main"]),
    format!("{}: Sign in", task.key())
  );
  assert_eq!(
    git(repo.root(), &["rev-list", "--count", "main"]),
    "2",
    "one commit on top of the base"
  );
  assert!(merged.branch_deleted, "{merged:?}");
  assert!(git(repo.root(), &["branch", "--list", "orchester/orc-1-run-1"]).is_empty());
}

#[test]
fn merge_queue_moves_waiting_runs_up_and_to_the_top_and_removes_them() {
  let mut repo = Repo::new();
  let tasks: Vec<Task> = ["One", "Two", "Three"]
    .into_iter()
    .map(|title| repo.task(title))
    .collect();
  let runs: Vec<Run> = tasks
    .iter()
    .enumerate()
    .map(|(index, task)| {
      let branch = format!("orchester/orc-{}-run-1", index + 1);
      let time = 100 * (i64::try_from(index).expect("small") + 1);
      repo.succeeded((task, "approve"), &branch, ("other.txt", "edit\n"), time)
    })
    .collect();
  let mut db = ProjectDb::open(repo.root()).expect("open");
  let programs = repo.programs();
  assert!(merge_queue::join(&mut db, (repo.root(), &programs), at(1_000)).expect("joined"));
  let order = |repo: &Repo| -> Vec<u32> {
    merge_queue::read(repo.root(), &repo.programs())
      .expect("read")
      .rows
      .iter()
      .map(|row| row.run_number)
      .collect()
  };
  let numbers: Vec<u32> = runs.iter().map(|run| run.number().get()).collect();
  assert_eq!(order(&repo), numbers, "first finished first");
  assert_eq!(
    merge_queue::move_run(repo.root(), runs[0].id(), Move::Up),
    Err(merge_queue::QueueRefusal::First)
  );
  merge_queue::move_run(repo.root(), runs[2].id(), Move::Up).expect("moved up");
  assert_eq!(order(&repo), [numbers[0], numbers[2], numbers[1]]);
  merge_queue::move_run(repo.root(), runs[1].id(), Move::ToTop).expect("moved to the top");
  assert_eq!(order(&repo), [numbers[1], numbers[0], numbers[2]]);
  merge_queue::remove(repo.root(), runs[0].id(), at(1_001)).expect("removed");
  assert_eq!(repo.state(&runs[0]), QueueState::Removed);
  assert_eq!(
    order(&repo),
    [numbers[1], numbers[2]],
    "a removed run is not shown and does not join again"
  );
  let mut db = ProjectDb::open(repo.root()).expect("open");
  assert!(!merge_queue::join(&mut db, (repo.root(), &programs), at(1_002)).expect("joined"));
}

#[test]
fn merge_queue_syncs_again_when_main_moves_during_the_check_and_never_merges_on_an_older_base() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let root = repo.root().to_path_buf();
  let moved = Arc::new(Mutex::new(None));
  let seen = moved.clone();
  repo.check_with(move || {
    let mut moved = seen.lock();
    if moved.is_none() {
      *moved = Some(Repo::commit_on_main(&root, ("other.txt", "moved during the check\n")));
    }
    0
  });
  repo.drive();
  let QueueState::Merged(merged) = repo.state(&run) else {
    panic!("{:?}", repo.state(&run));
  };
  let moved = moved.lock().clone().expect("main moved");
  assert_eq!(repo.checks(), 2, "the check ran again once main moved");
  let events = repo.events(&run);
  assert!(
    events
      .iter()
      .any(|event| matches!(event, QueueEvent::CheckSuperseded { now, .. } if now.as_str() == moved)),
    "{events:?}"
  );
  let passed: Vec<&Revision> = events
    .iter()
    .filter_map(|event| match event {
      QueueEvent::CheckPassed { base, .. } => Some(base),
      _ => None,
    })
    .collect();
  assert_eq!(
    passed.last().map(|base| base.as_str()),
    Some(moved.as_str()),
    "the check that counted ran on main as it was merged into"
  );
  assert_eq!(
    git(repo.root(), &["rev-parse", &format!("{}^1", merged.revision)]),
    moved
  );
}

#[test]
fn merge_queue_takes_a_run_out_once_main_keeps_moving_and_nothing_reaches_main() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"]);
  let root = repo.root().to_path_buf();
  let mut count = 0;
  repo.check_with(move || {
    count += 1;
    Repo::commit_on_main(&root, ("other.txt", &format!("move {count}\n")));
    0
  });
  repo.drive();
  let QueueState::Left(Left::KeepsMoving { base, heads }) = repo.state(&run) else {
    panic!("{:?}", repo.state(&run));
  };
  assert_eq!(base, "main");
  assert_eq!(repo.checks(), 4, "three re-syncs in a row, then it leaves");
  assert_eq!(heads.len(), 4, "{heads:?}");
  assert!(!repo.main_holds(&tip), "nothing of the run reached main");
}

#[test]
fn merge_queue_sends_conflicts_to_the_integrate_step_and_when_it_fails_nothing_reaches_main() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded(
    (&task, "auto"),
    "orchester/orc-1-run-1",
    ("shared.txt", "the run's line\n"),
    100,
  );
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"]);
  Repo::commit_on_main(repo.root(), ("shared.txt", "main's line\n"));
  let commands = repo.drive();
  let [RunnerCommand::ReturnRun(returned)] = commands.as_slice() else {
    panic!("{commands:?}");
  };
  assert_eq!(returned.run, run.id());
  assert_eq!(
    returned.step.as_str(),
    "implement",
    "the step the integrator profile works in"
  );
  assert_eq!(
    returned.conflicts.as_ref().map(|conflicts| conflicts.files.clone()),
    Some(vec!["shared.txt".to_owned()])
  );
  assert!(
    matches!(repo.state(&run), QueueState::Integrating { .. }),
    "{:?}",
    repo.state(&run)
  );
  assert_eq!(
    git(&repo.worktree(&run), &["status", "--porcelain"]),
    "",
    "the sync that conflicted was put back"
  );

  let refused = orchester_workspace::messages::Message::new("errors.storage");
  merge_queue::integrate_sent(repo.root(), run.id(), Err(refused)).expect("told");
  assert_eq!(
    repo.state(&run),
    QueueState::Left(Left::Conflicts {
      files: vec!["shared.txt".to_owned()]
    })
  );
  assert!(!repo.main_holds(&tip), "nothing of the run reached main");
  assert_eq!(repo.drive(), Vec::new(), "the run left the queue");
}

#[test]
fn merge_queue_blocks_on_a_dirty_checkout_and_goes_on_once_it_is_clean() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  fs::write(repo.root().join("other.txt"), "not committed\n").expect("edit");
  repo.drive();
  assert_eq!(
    repo.state(&run),
    QueueState::Blocked(Blocked::Uncommitted {
      files: vec!["M other.txt".to_owned()]
    })
  );
  assert_eq!(
    merge_queue::advance(repo.root(), &repo.programs()).expect("advanced"),
    Advance::Idle { poll: true },
    "it looks again later, by itself"
  );
  git(repo.root(), &["checkout", "--", "other.txt"]);
  repo.drive();
  assert!(
    matches!(repo.state(&run), QueueState::Merged(_)),
    "{:?}",
    repo.state(&run)
  );
}

/// ORC-81: a run that took the checked-out branch is blocked by a checkout on another branch; a run whose base branch
/// was set (its task's) lands in its recorded base beside the checkout, which stays on its branch with its files as
/// they were.
#[test]
fn merge_queue_lands_beside_a_checkout_on_another_branch_once_the_base_branch_is_set() {
  let mut following = Repo::new();
  let task = following.task("Sign in");
  let run = following.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  git(following.root(), &["switch", "--quiet", "-c", "feature"]);
  following.drive();
  assert_eq!(
    following.state(&run),
    QueueState::Blocked(Blocked::OtherBranch {
      current: Some("feature".to_owned())
    })
  );

  let mut repo = Repo::new();
  repo.base_source = RunBaseSource::Task;
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"]);
  git(repo.root(), &["switch", "--quiet", "-c", "feature"]);
  let feature = git(repo.root(), &["rev-parse", "feature"]);
  repo.drive();
  assert!(
    matches!(repo.state(&run), QueueState::Merged(_)),
    "{:?}",
    repo.state(&run)
  );
  let read = merge_queue::read(repo.root(), &repo.programs()).expect("read");
  assert_eq!(
    read.base.expect("a base").checkout,
    merge_queue::CheckoutState::Beside(Some("feature".to_owned())),
    "the queue delivers into main beside the checkout, as the run recorded"
  );
  assert!(repo.main_holds(&tip));
  assert_eq!(git(repo.root(), &["symbolic-ref", "--short", "HEAD"]), "feature");
  assert_eq!(
    git(repo.root(), &["rev-parse", "HEAD"]),
    feature,
    "the checkout did not move"
  );
  assert_eq!(
    fs::read_to_string(repo.root().join("shared.txt")).expect("file"),
    "base\n"
  );
}

#[test]
fn merge_queue_waiting_for_approval_does_not_hold_later_runs_and_checks_again_once_approved() {
  let mut repo = Repo::new();
  let (first, second) = (repo.task("Sign in"), repo.task("Sign out"));
  let waiting = repo.succeeded(
    (&first, "approve"),
    "orchester/orc-1-run-1",
    ("shared.txt", "first\n"),
    100,
  );
  let later = repo.succeeded(
    (&second, "auto"),
    "orchester/orc-2-run-1",
    ("other.txt", "second\n"),
    200,
  );
  repo.drive();
  assert_eq!(repo.state(&waiting), QueueState::WaitingForApproval);
  assert!(
    matches!(repo.state(&later), QueueState::Merged(_)),
    "{:?}",
    repo.state(&later)
  );
  let read = merge_queue::read(repo.root(), &repo.programs()).expect("read");
  let row = read.rows.iter().find(|row| row.entry.run == waiting.id()).expect("row");
  assert!(row.base_moved, "main moved since its check");

  merge_queue::approve(repo.root(), waiting.id(), at(1_000), Answerer::Person).expect("approved");
  repo.drive();
  assert!(
    matches!(repo.state(&waiting), QueueState::Merged(_)),
    "{:?}",
    repo.state(&waiting)
  );
  assert_eq!(
    repo.checks(),
    3,
    "the approved run was checked again on main as it was then"
  );
}

#[test]
fn merge_queue_leaves_out_runs_whose_pipeline_has_no_deliver_step_and_takes_a_failed_check_out() {
  let mut repo = Repo::new();
  let (manual, checked) = (repo.task("Merged by hand"), repo.task("Fails its check"));
  let by_hand = repo.succeeded(
    (&manual, "none"),
    "orchester/orc-1-run-1",
    ("other.txt", "by hand\n"),
    100,
  );
  let failing = repo.succeeded(
    (&checked, "auto"),
    "orchester/orc-2-run-1",
    ("shared.txt", "run\n"),
    200,
  );
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-2-run-1"]);
  repo.check_with(|| 1);
  repo.drive();
  let db = ProjectDb::open(repo.root()).expect("open");
  assert_eq!(
    db.merge_queue_entry(by_hand.id()).expect("entry"),
    None,
    "it keeps today's way"
  );
  assert!(
    matches!(repo.state(&failing), QueueState::Left(Left::CheckFailed { .. })),
    "{:?}",
    repo.state(&failing)
  );
  assert!(!repo.main_holds(&tip), "nothing of the run reached main");
}

#[test]
fn merge_queue_skips_a_run_that_committed_nothing() {
  let mut repo = Repo::new();
  let (report, change) = (repo.task("Investigate the crash"), repo.task("Fix the crash"));
  let nothing = repo.succeeded_without_commits((&report, "auto"), "orchester/orc-1-run-1", 100);
  let fixed = repo.succeeded(
    (&change, "auto"),
    "orchester/orc-2-run-1",
    ("shared.txt", "fixed\n"),
    200,
  );
  repo.drive();
  let db = ProjectDb::open(repo.root()).expect("open");
  assert_eq!(
    db.merge_queue_entry(nothing.id()).expect("entry"),
    None,
    "a run with nothing to merge never joins: it is completed, or it waits for the person"
  );
  assert!(
    matches!(repo.state(&fixed), QueueState::Merged(_)),
    "{:?}",
    repo.state(&fixed)
  );
}
