//! When work that passed every step stops waiting to be merged without a merge in Orchester: its task is
//! Done or Canceled, a person merged its branch into the branch it started from with git, or the run
//! committed nothing, so there is nothing to merge; the task list's rows, the task's page with its runs and the
//! Orchestrator chat's cards read it alike, against a real repository.

mod support;

use std::{
  fs,
  path::Path,
  process,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{Change, ProposedChange},
  },
  run::{Run, TaskPhase},
  store::TaskStore,
  task::organization::{ChangeActor, TaskOrganization, WorkflowStatus},
};
use orchester_exec::{ProcessRunner, programs::Programs};
use orchester_workspace::{
  orchestrator::facts::{Facts, Stands, load_in},
  tasks::{
    data::ProjectTasks,
    delivery::{Delivery, settle_records},
    detail::TaskDetail,
    history::runs_of,
    list::{self, TaskTab},
    needs_you::WaitingKind,
  },
};
use support::{Fixture, at};

/// `git <args>` in `repo`, which must succeed; its trimmed output.
fn git(repo: &Path, args: &[&str]) -> String {
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
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The fixture's folder as a repository on `main` with one commit, which it returns.
fn repository(fixture: &Fixture) -> String {
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
  git(repo, &["rev-parse", "HEAD"])
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
    .iter()
    .filter(|item| item.kind == WaitingKind::ReadyToMerge)
    .filter_map(|item| item.task.as_ref().map(|task| task.title.to_string()))
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
  let base = repository(&fixture);
  let by_hand = fixture.task("Write docs", at(10));
  let mut merged = fixture.started_run_from(by_hand, (&base, "main"), at(20));
  fixture.succeed(&mut merged, at(30));
  work_on_branch(&fixture, &merged, "docs.md");
  let waiting = fixture.task("Add sign-out", at(40));
  let mut open = fixture.started_run_from(waiting, (&base, "main"), at(50));
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
  let page = TaskDetail::load_settled(fixture.repo.path(), by_hand, &programs).expect("the task reads");
  assert_eq!(
    (page.phase(), page.delivery),
    (TaskPhase::Merged, Delivery::MergedOutside("main".into())),
    "its page says what its row says"
  );
  let listed = &page.history[0];
  assert_eq!(listed.phase, Some(TaskPhase::Merged), "*Runs of this task* says so too");
  assert_eq!(listed.detail.key, "runs.history.mergedOutside");
  let card = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Write docs".to_owned(),
      change: Change::Merge {
        run: merged.id(),
        number: merged.number().get(),
        checkpoint: None,
        into: None,
      },
    })),
    ..ChatAction::new("merge_run", None, ActionOutcome::Waits)
  };
  let Some(Facts::Run(facts)) = load_in(fixture.repo.path(), &programs, &card).expect("the card's facts") else {
    panic!("a merge proposal's card reads its run");
  };
  assert_eq!(
    facts.stands,
    Stands::MergedOutside {
      into: "main".to_owned()
    },
    "the Orchestrator's card does not call it ready to merge"
  );
}

#[test]
fn ready_to_merge_a_run_that_committed_nothing_is_neither_ready_nor_merged() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let review = fixture.task("Review the sign-in flow", at(10));
  let mut run = fixture.started_run_from(review, (&base, "main"), at(20));
  fixture.succeed(&mut run, at(30));
  git(fixture.repo.path(), &["branch", run.branch().as_str(), &base]);
  git(
    fixture.repo.path(),
    &["commit", "--quiet", "--allow-empty", "-m", "the person's own work"],
  );
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read");
  assert!(ready_to_merge(&tasks).is_empty(), "there is nothing to merge");
  let latest = tasks
    .phases
    .get(&review)
    .and_then(|latest| latest.run())
    .expect("its run");
  assert_eq!(
    (latest.phase, latest.nothing_to_merge, latest.merged_into.as_deref()),
    (TaskPhase::Succeeded, true, None),
    "main holds its tip, which is where it started: that is no merge"
  );
  let rows = list::rows(&tasks.tasks, &tasks, TaskTab::All);
  assert_eq!(rows[0].detail.key, "tasks.progress.nothingToMerge");
  assert!(rows[0].nothing_to_merge);
  let page = TaskDetail::load_settled(fixture.repo.path(), review, &programs).expect("the task reads");
  assert_eq!(
    (page.phase(), page.delivery),
    (TaskPhase::Succeeded, Delivery::NothingToMerge),
    "its page says what its row says"
  );
}

/// Runs every command with git as `Programs::processes` does, counting them.
struct Counted {
  git: ProcessRunner,
  runs: AtomicUsize,
}

impl CommandRunner for Counted {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    self.runs.fetch_add(1, Ordering::Relaxed);
    self.git.run(command)
  }
}

#[test]
fn ready_to_merge_runs_of_many_tasks_are_settled_in_one_reading_of_the_branches() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let mut records = Vec::new();
  for (title, file) in [
    ("Write docs", "docs.md"),
    ("Add sign-out", "sign_out.rs"),
    ("Fix zoom", "zoom.rs"),
  ] {
    let task = fixture.task(title, at(10));
    let mut run = fixture.started_run_from(task, (&base, "main"), at(20));
    fixture.succeed(&mut run, at(30));
    work_on_branch(&fixture, &run, file);
    let task = fixture.db.task(task).expect("read").expect("the task");
    records.extend(runs_of(&fixture.db, &task).expect("its runs"));
  }
  git(
    fixture.repo.path(),
    &[
      "merge",
      "--quiet",
      "--no-ff",
      "-m",
      "by hand",
      records[0].run.as_ref().expect("run").branch().as_str(),
    ],
  );
  let counted = Arc::new(Counted {
    git: ProcessRunner::new(Duration::from_secs(30)),
    runs: AtomicUsize::new(0),
  });
  let settled = settle_records(
    &Programs::with_runner(counted.clone()),
    fixture.repo.path(),
    records.iter_mut(),
  );
  assert_eq!(settled.len(), 3);
  assert_eq!(records[0].phase, Some(TaskPhase::Merged));
  assert_eq!(records[1].phase, Some(TaskPhase::Succeeded));
  assert_eq!(
    counted.runs.load(Ordering::Relaxed),
    3,
    "one branch listing and one of what main holds for all tasks, and one ancestry check of the merged run"
  );
}
