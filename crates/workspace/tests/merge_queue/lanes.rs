//! The queue's lanes (ORC-406): a run is in the lane of the base branch it recorded; each lane delivers one run at a
//! time, first finished first, and what holds one lane (a blocked checkout, a run waiting for approval, a run that
//! left with conflicts) holds no other. Sync, the re-check and *base moved* follow the lane's own base head; the read
//! model gives each lane with its branch, how it lands and its runs; a run's delivery names its lane.
//! `cargo test -p orchester-workspace --test merge_queue merge_queue_lanes`.

use std::{fs, process::Command};

use orchester_core::{
  integration::base::RunBaseSource,
  merge_queue::{Blocked, Left, QueueEvent, QueueState},
  run::Run,
};
use orchester_store::ProjectDb;
use orchester_workspace::merge_queue::{self, CheckoutState, LaneLanding, Move, QueueRefusal};

use super::harness::{Repo, at, git};

/// Whether `branch` holds `commit`.
fn holds(repo: &Repo, branch: &str, commit: &str) -> bool {
  Command::new("git")
    .args(["merge-base", "--is-ancestor", commit, branch])
    .current_dir(repo.root())
    .status()
    .expect("git")
    .success()
}

/// A repository with `release` beside `main`, at the same commit.
fn with_release() -> Repo {
  let repo = Repo::new();
  git(repo.root(), &["branch", "release", "main"]);
  repo
}

/// From now on, `repo`'s runs start from `release`, set as their task's base branch, so they land beside a checkout
/// on `main`.
fn on_release(repo: &mut Repo) {
  repo.base_branch = "release";
  repo.base_source = RunBaseSource::Task;
}

/// Commits `text` to `file` on `branch` in the checkout, which goes back to `main` after; the branch's new head.
fn commit_on(repo: &Repo, branch: &str, (file, text): (&str, &str)) -> String {
  git(repo.root(), &["switch", "--quiet", branch]);
  fs::write(repo.root().join(file), text).expect("edit");
  git(repo.root(), &["commit", "-am", &format!("A change on {branch}")]);
  let head = git(repo.root(), &["rev-parse", "HEAD"]);
  git(repo.root(), &["switch", "--quiet", "main"]);
  head
}

fn read(repo: &Repo) -> merge_queue::QueueRead {
  merge_queue::read(repo.root(), &repo.programs()).expect("read")
}

#[test]
fn merge_queue_lanes_deliver_each_base_branch_on_its_own_while_another_lane_is_blocked() {
  let mut repo = with_release();
  let (first, second) = (repo.task("Sign in"), repo.task("Release notes"));
  let on_main = repo.succeeded(
    (&first, "auto"),
    "orchester/orc-1-run-1",
    ("shared.txt", "main's run\n"),
    100,
  );
  on_release(&mut repo);
  let released = repo.succeeded(
    (&second, "auto"),
    "orchester/orc-2-run-1",
    ("other.txt", "release's run\n"),
    200,
  );
  let tips = (
    git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"]),
    git(repo.root(), &["rev-parse", "orchester/orc-2-run-1"]),
  );
  fs::write(repo.root().join("other.txt"), "not committed\n").expect("edit");

  repo.drive();

  assert_eq!(
    repo.state(&on_main),
    QueueState::Blocked(Blocked::Uncommitted {
      files: vec!["M other.txt".to_owned()]
    }),
    "the dirty checkout of main holds main's lane"
  );
  assert!(
    matches!(repo.state(&released), QueueState::Merged(_)),
    "release's lane delivered though it finished later: {:?}",
    repo.state(&released)
  );
  assert!(holds(&repo, "release", &tips.1));
  assert!(!holds(&repo, "main", &tips.1), "release's run went into release only");
  assert!(!holds(&repo, "main", &tips.0));

  assert_reads_both_lanes(&repo, &on_main, &released);

  git(repo.root(), &["checkout", "--", "other.txt"]);
  repo.drive();
  assert!(
    matches!(repo.state(&on_main), QueueState::Merged(_)),
    "{:?}",
    repo.state(&on_main)
  );
  assert!(holds(&repo, "main", &tips.0));
}

/// The read model of `merge_queue_lanes_deliver_each_base_branch_on_its_own_while_another_lane_is_blocked`: main's
/// lane blocked in the checkout, release's beside it with its own head, and the delivery naming its lane.
fn assert_reads_both_lanes(repo: &Repo, on_main: &Run, released: &Run) {
  let queue = read(repo);
  let lanes: Vec<(&str, &LaneLanding)> = queue
    .lanes
    .iter()
    .map(|lane| (lane.base.branch.as_str(), &lane.lands))
    .collect();
  assert_eq!(
    lanes,
    [
      ("main", &LaneLanding::InCheckout),
      ("release", &LaneLanding::Beside(Some("main".to_owned())))
    ]
  );
  assert_eq!(
    queue.lanes[0].base.checkout,
    CheckoutState::Uncommitted(vec!["M other.txt".to_owned()])
  );
  assert_eq!(queue.lanes[0].runs, [on_main.id()]);
  assert_eq!(queue.lanes[1].runs, [released.id()]);
  assert_eq!(
    queue
      .lane_rows(&queue.lanes[1])
      .map(|row| row.entry.run)
      .collect::<Vec<_>>(),
    [released.id()]
  );
  assert_eq!(
    queue.lanes[1].base.head.as_ref().map(|head| head.as_str().to_owned()),
    Some(git(repo.root(), &["rev-parse", "release"])),
    "each lane reads its own base head"
  );
  let db = ProjectDb::open(repo.root()).expect("open");
  let delivery = merge_queue::delivery_of(&db, released.id())
    .expect("read")
    .expect("joined");
  assert_eq!(
    delivery.entry.base.as_ref().map(|base| base.as_str()),
    Some("release"),
    "the delivery names its lane"
  );
}

#[test]
fn merge_queue_lanes_a_run_waiting_for_approval_or_leaving_with_conflicts_holds_no_other_lane() {
  let mut repo = with_release();
  let tasks: Vec<_> = ["Approve me", "Conflicts", "Main later", "Release later"]
    .into_iter()
    .map(|title| repo.task(title))
    .collect();
  let waiting = repo.succeeded(
    (&tasks[0], "approve"),
    "orchester/orc-1-run-1",
    ("shared.txt", "approve\n"),
    100,
  );
  on_release(&mut repo);
  let conflicting = repo.succeeded(
    (&tasks[1], "unfit"),
    "orchester/orc-2-run-1",
    ("shared.txt", "run\n"),
    150,
  );
  commit_on(&repo, "release", ("shared.txt", "release's line\n"));
  repo.base_branch = "main";
  repo.base_source = RunBaseSource::CheckedOut;
  let main_later = repo.succeeded(
    (&tasks[2], "auto"),
    "orchester/orc-3-run-1",
    ("other.txt", "main\n"),
    200,
  );
  on_release(&mut repo);
  let release_later = repo.succeeded(
    (&tasks[3], "auto"),
    "orchester/orc-4-run-1",
    ("other.txt", "release\n"),
    250,
  );
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-4-run-1"]);

  assert_eq!(repo.drive(), Vec::new(), "no Integrate step to send conflicts to");

  assert_eq!(repo.state(&waiting), QueueState::WaitingForApproval);
  assert_eq!(
    repo.state(&conflicting),
    QueueState::Left(Left::Conflicts {
      files: vec!["shared.txt".to_owned()]
    })
  );
  for (run, branch) in [(&main_later, "main"), (&release_later, "release")] {
    assert!(
      matches!(repo.state(run), QueueState::Merged(_)),
      "{branch}'s later run merged: {:?}",
      repo.state(run)
    );
  }
  assert!(holds(&repo, "release", &tip));
  assert!(!holds(&repo, "main", &tip));
}

#[test]
fn merge_queue_lanes_sync_and_base_moved_follow_the_lanes_own_base_head() {
  let mut repo = with_release();
  on_release(&mut repo);
  let task = repo.task("Release notes");
  let run = repo.succeeded(
    (&task, "approve"),
    "orchester/orc-1-run-1",
    ("other.txt", "notes\n"),
    100,
  );
  let release_moved = commit_on(&repo, "release", ("shared.txt", "release moved\n"));

  repo.drive();

  assert_eq!(repo.state(&run), QueueState::WaitingForApproval);
  let synced = repo.events(&run).into_iter().find_map(|event| match event {
    QueueEvent::Synced { base, .. } => Some(base.as_str().to_owned()),
    _ => None,
  });
  assert_eq!(synced, Some(release_moved), "synced with release's head, not main's");
  let base_moved = |repo: &Repo, run: &Run| {
    read(repo)
      .rows
      .iter()
      .find(|row| row.entry.run == run.id())
      .expect("row")
      .base_moved
  };
  assert!(!base_moved(&repo, &run), "checked on release as it is");

  Repo::commit_on_main(repo.root(), ("shared.txt", "main moved\n"));
  assert!(!base_moved(&repo, &run), "main moving is another lane's");

  commit_on(&repo, "release", ("shared.txt", "release moved again\n"));
  assert!(base_moved(&repo, &run), "release moved since its check");
}

#[test]
fn merge_queue_lanes_move_waiting_runs_within_their_lane_only() {
  let mut repo = with_release();
  let tasks: Vec<_> = ["One", "Two", "Three"]
    .into_iter()
    .map(|title| repo.task(title))
    .collect();
  let first = repo.succeeded(
    (&tasks[0], "approve"),
    "orchester/orc-1-run-1",
    ("other.txt", "one\n"),
    100,
  );
  on_release(&mut repo);
  let release = repo.succeeded(
    (&tasks[1], "approve"),
    "orchester/orc-2-run-1",
    ("other.txt", "two\n"),
    200,
  );
  repo.base_branch = "main";
  repo.base_source = RunBaseSource::CheckedOut;
  let second = repo.succeeded(
    (&tasks[2], "approve"),
    "orchester/orc-3-run-1",
    ("other.txt", "three\n"),
    300,
  );
  let mut db = ProjectDb::open(repo.root()).expect("open");
  assert!(merge_queue::join(&mut db, (repo.root(), &repo.programs()), at(1_000)).expect("joined"));

  assert_eq!(
    merge_queue::move_run(repo.root(), release.id(), Move::Up),
    Err(QueueRefusal::First),
    "first in its lane, though second in the queue"
  );
  merge_queue::move_run(repo.root(), second.id(), Move::Up).expect("moved up");
  let queue = read(&repo);
  let main = &queue.lanes[0];
  assert_eq!(main.base.branch.as_str(), "main");
  assert_eq!(main.runs, [second.id(), first.id()], "moved up past main's first run");
  let places: Vec<(u32, Option<usize>)> = queue.rows.iter().map(|row| (row.run_number, row.place)).collect();
  assert!(
    queue
      .rows
      .iter()
      .all(|row| row.place == Some(if row.entry.run == first.id() { 2 } else { 1 })),
    "places count within each lane: {places:?}"
  );
}
