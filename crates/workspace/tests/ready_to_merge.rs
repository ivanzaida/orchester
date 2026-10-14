//! When work that passed every step stops waiting to be merged without a merge in Orchester: its task is
//! Done or Canceled, or a person merged its branch into the branch it started from with git, against a real
//! repository.

mod support;

use std::{fs, path::Path, process};

use orchester_core::{
  run::{Run, TaskPhase},
  task::organization::{ChangeActor, TaskOrganization, WorkflowStatus},
};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{data::ProjectTasks, needs_you::WaitingKind};
use support::{Fixture, at};

fn git(repo: &Path, args: &[&str]) {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// The fixture's folder as a repository on `main` with one commit.
fn repository(fixture: &Fixture) {
  let repo = fixture.repo.path();
  fs::remove_dir_all(repo.join(".git")).expect("the fixture's stand-in .git");
  git(repo, &["init", "--quiet", "--initial-branch=main"]);
  git(repo, &["config", "user.name", "Orchester Test"]);
  git(repo, &["config", "user.email", "test@orchester.invalid"]);
  git(repo, &["config", "commit.gpgsign", "false"]);
  fs::write(repo.join(".git").join("info").join("exclude"), "/.orchester/\n").expect("exclude .orchester");
  fs::write(repo.join("README.md"), "hello\n").expect("a file");
  git(repo, &["add", "--all"]);
  git(repo, &["commit", "--quiet", "-m", "initial"]);
}

/// `run`'s branch with one commit of its own on top of `main`, as its agent leaves it.
fn work_on_branch(fixture: &Fixture, run: &Run, file: &str) {
  let repo = fixture.repo.path();
  git(repo, &["checkout", "--quiet", "-b", run.branch().as_str()]);
  fs::write(repo.join(file), "work\n").expect("a file");
  git(repo, &["add", "--all"]);
  git(repo, &["commit", "--quiet", "-m", file]);
  git(repo, &["checkout", "--quiet", "main"]);
}

fn ready_to_merge(tasks: &ProjectTasks) -> Vec<String> {
  tasks
    .waiting()
    .into_iter()
    .filter(|item| item.kind == WaitingKind::ReadyToMerge)
    .filter_map(|item| item.task.map(|task| task.title.to_string()))
    .collect()
}

#[test]
fn ready_to_merge_leaves_out_tasks_done_or_canceled_by_hand() {
  let mut fixture = Fixture::new();
  for (title, status) in [
    ("Write docs", WorkflowStatus::InReview),
    ("Add sign-out", WorkflowStatus::Done),
    ("Fix zoom", WorkflowStatus::Canceled),
  ] {
    let task = fixture.task(title, at(10));
    let mut run = fixture.started_run(task, at(20));
    fixture.succeed(&mut run, at(30));
    fixture
      .db
      .set_task_organization(
        task,
        &TaskOrganization::default().with_status(status),
        ChangeActor::Person,
        at(40),
      )
      .expect("the person moves the task");
  }
  let tasks = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("the tasks read");
  assert_eq!(
    ready_to_merge(&tasks),
    ["Write docs"],
    "work its person marked Done or Canceled waits to be merged no more"
  );
  assert_eq!(tasks.needs_you(), 1);
}

#[test]
fn ready_to_merge_counts_work_merged_with_git_as_merged() {
  let mut fixture = Fixture::new();
  repository(&fixture);
  let by_hand = fixture.task("Write docs", at(10));
  let mut merged = fixture.started_run_from(by_hand, "main", at(20));
  fixture.succeed(&mut merged, at(30));
  work_on_branch(&fixture, &merged, "docs.md");
  let waiting = fixture.task("Add sign-out", at(40));
  let mut open = fixture.started_run_from(waiting, "main", at(50));
  fixture.succeed(&mut open, at(60));
  work_on_branch(&fixture, &open, "sign_out.rs");
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read");
  assert_eq!(ready_to_merge(&tasks), ["Write docs", "Add sign-out"]);

  let branch = merged.branch().as_str();
  git(
    fixture.repo.path(),
    &["merge", "--quiet", "--no-ff", "-m", "merged by hand", branch],
  );
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read again");
  assert_eq!(
    ready_to_merge(&tasks),
    ["Add sign-out"],
    "a merge made with git settles the work as one in Orchester does"
  );
  let latest = tasks
    .phases
    .get(&by_hand)
    .and_then(|latest| latest.run())
    .expect("its run");
  assert_eq!(latest.phase, TaskPhase::Merged);
  assert_eq!(latest.merged_into.as_deref(), Some("main"));
}
