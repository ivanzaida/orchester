//! Runs whose result is not a commit (ORC-93), against a real repository: a run that passed every step and committed
//! nothing waits to be completed when its task or pipeline says it delivers a report or work outside the repository,
//! and is flagged when changes were expected; completing keeps the last step's report as the run's *Report* and sets
//! the task Done; the Orchestrator completes such a run on its own as it would merge one, never a flagged one.
//! `cargo test -p orchester-workspace --test run_completion completion`.

mod support;

use std::{collections::HashSet, fs, path::Path, process};

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId, TaskId},
  resource::{Deliverable, StepKey},
  run::{
    CompletedBy, DecidedBy, DecidedToolApproval, PendingToolApproval, REPORT_ARTIFACT, Run, StepOutcome, StepOutput,
    TaskPhase, ToolApprovalDecision,
  },
  store::{ActivityStore, TaskStore},
  task::organization::{ChangeActor, WorkflowStatus},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::{
  orchestrator::{
    auto_merge::completes_in,
    waits::{MergeChecks, Switches, follow_in},
  },
  tasks::{
    completion::{complete_in, outside_changes},
    data::ProjectTasks,
    delivery::Delivery,
    detail::TaskDetail,
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

/// A task titled `title` that delivers `delivers` (`None`: as the fixture's pipeline, changes), and its run, which
/// passed its step at `now` with `report` as its result and committed nothing: its branch is where it started.
fn nothing_committed(
  fixture: &mut Fixture,
  (title, delivers): (&str, Option<Deliverable>),
  (base, report): (&str, &str),
) -> (TaskId, Run) {
  let task = fixture.task(title, at(10));
  fixture
    .db
    .update_task_organization(task, ChangeActor::Person, at(11), |value| value.with_delivers(delivers))
    .expect("organized");
  let mut run = fixture.started_run_from(task, (base, "main"), at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  output(fixture, &run, execution, report);
  fixture.succeed(&mut run, at(30));
  git(fixture.repo.path(), &["branch", run.branch().as_str(), base]);
  (task, run)
}

/// `execution` of `run` left `text` as its result.
fn output(fixture: &mut Fixture, run: &Run, execution: StepExecutionId, text: &str) {
  let output = StepOutput {
    run: run.id(),
    execution,
    step: StepKey::new("implement").expect("key"),
    outcome: Some(StepOutcome::Done),
    text: text.to_owned(),
    unverified: Vec::new(),
    created_at: at(29),
  };
  fixture.db.save_step_output(&output).expect("output kept");
}

fn waiting(tasks: &ProjectTasks) -> Vec<WaitingKind> {
  tasks.waiting().iter().map(|item| item.kind.clone()).collect()
}

fn complete(fixture: &Fixture, run: RunId, anyway: bool) -> Result<(), String> {
  complete_in(
    (fixture.repo.path(), &Programs::processes()),
    run,
    (CompletedBy::Person, anyway),
    Timestamp::now(),
  )
  .map(|_| ())
  .map_err(|message| message.key.to_owned())
}

#[test]
fn completion_a_report_run_with_nothing_to_merge_waits_to_be_completed_and_keeps_its_report() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let report = "## Cause\nThe cache key ignores the locale.";
  let (task, run) = nothing_committed(
    &mut fixture,
    ("Investigate the crash", Some(Deliverable::Report)),
    (&base, report),
  );
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read");
  assert_eq!(waiting(&tasks), [WaitingKind::ReadyToComplete]);
  let rows = list::rows(&tasks.tasks, &tasks, TaskTab::All);
  assert_eq!(rows[0].detail.key, "tasks.progress.reportReady");
  let page = TaskDetail::load_settled(fixture.repo.path(), task, &programs).expect("the task reads");
  assert_eq!(page.delivery, Delivery::NothingToMerge);
  let result = page.result.expect("a run that passed every step has a result");
  assert_eq!(result.delivers, Deliverable::Report);
  assert_eq!(result.completion, None);
  assert_eq!(
    result.report.map(|report| report.text.to_string()).as_deref(),
    Some(report)
  );

  complete(&fixture, run.id(), false).expect("completed");
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  assert_eq!(
    db.task(task).expect("read").expect("task").organization().status(),
    WorkflowStatus::Done,
    "completing sets the task Done, as a merge does"
  );
  let kept = db.run_artifacts(run.id()).expect("artifacts");
  assert_eq!(kept.len(), 1);
  assert_eq!(
    (kept[0].name.as_str(), kept[0].text.as_str()),
    (REPORT_ARTIFACT, report)
  );
  let completion = db.run_completion(run.id()).expect("read").expect("completed");
  assert_eq!(completion.by, CompletedBy::Person);
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read again");
  assert!(waiting(&tasks).is_empty(), "a Done task waits for nothing");
  assert_eq!(
    complete(&fixture, run.id(), false),
    Err("runs.complete.failure.alreadyCompleted".to_owned())
  );
}

#[test]
fn completion_a_run_that_committed_nothing_where_changes_were_expected_is_flagged() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let (task, run) = nothing_committed(&mut fixture, ("Fix the crash", None), (&base, "Nothing to change."));
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read");
  assert_eq!(waiting(&tasks), [WaitingKind::CommittedNothing]);
  assert_eq!(
    complete(&fixture, run.id(), false),
    Err("runs.complete.failure.expectsChanges".to_owned()),
    "only the person's Complete anyway completes it"
  );
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  assert!(
    !completes_in(&db, run.id()).expect("read"),
    "the Orchestrator never completes it"
  );
  complete(&fixture, run.id(), true).expect("completed anyway");
  let page = TaskDetail::load_settled(fixture.repo.path(), task, &programs).expect("the task reads");
  assert_eq!(page.phase(), TaskPhase::Succeeded);
  assert!(page.result.and_then(|result| result.completion).is_some());
}

#[test]
fn completion_a_run_with_commits_is_merged_not_completed() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let task = fixture.task("Write docs", at(10));
  fixture
    .db
    .update_task_organization(task, ChangeActor::Person, at(11), |value| {
      value.with_delivers(Some(Deliverable::ChangesOrReport))
    })
    .expect("organized");
  let mut run = fixture.started_run_from(task, (&base, "main"), at(20));
  fixture.succeed(&mut run, at(30));
  let repo = fixture.repo.path();
  git(repo, &["checkout", "--quiet", "-b", run.branch().as_str()]);
  fs::write(repo.join("docs.md"), "work\n").expect("a file");
  git(repo, &["add", "--all"]);
  git(repo, &["commit", "--quiet", "-m", "docs"]);
  git(repo, &["checkout", "--quiet", "main"]);
  assert_eq!(
    complete(&fixture, run.id(), true),
    Err("runs.complete.failure.hasCommits".to_owned())
  );
  let tasks = ProjectTasks::load(repo, &Programs::processes()).expect("the tasks read");
  assert_eq!(waiting(&tasks), [WaitingKind::ReadyToMerge]);
}

#[test]
fn completion_the_orchestrator_completes_a_report_run_on_its_own_as_it_would_merge() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let (task, run) = nothing_committed(
    &mut fixture,
    ("Design the sign-in page", Some(Deliverable::Outside)),
    (&base, "Drew the page in Pencil."),
  );
  let checks = MergeChecks {
    quiet: HashSet::from([(run.id(), run.last_sequence().get())]),
    waiting: HashSet::new(),
    nothing: HashSet::from([(run.id(), run.last_sequence().get())]),
  };
  let project = fixture.db.project().id();
  let programs = Programs::processes();
  let follow =
    |switches| follow_in((fixture.repo.path(), project, &programs), &checks, (switches, true)).expect("followed");
  let off = follow(Switches::default());
  assert!(!off.completed, "without the person's switches nothing is completed");
  let opted_out = |value: orchester_core::task::organization::TaskOrganization| value.with_no_automatic_merge(true);
  fixture
    .db
    .update_task_organization(task, ChangeActor::Person, Timestamp::now(), opted_out)
    .expect("organized");
  let merging = Switches {
    answers: true,
    continues: false,
    merges: true,
  };
  assert!(
    !follow(merging).completed,
    "Don't merge automatically keeps it for the person"
  );
  fixture
    .db
    .update_task_organization(task, ChangeActor::Person, Timestamp::now(), |value| {
      value.with_no_automatic_merge(false)
    })
    .expect("organized");
  assert!(follow(merging).completed);
  let completion = ProjectDb::open(fixture.repo.path())
    .expect("open")
    .run_completion(run.id())
    .expect("read")
    .expect("completed");
  assert_eq!(completion.by, CompletedBy::Orchestrator);
  assert!(!follow(merging).completed, "once is enough");
}

#[test]
fn completion_lists_the_mcp_tool_calls_that_were_approved_outside_orchesters_own_tools() {
  let request = |server: &str, command: bool| PendingToolApproval {
    id: format!("{server}-{command}"),
    run: RunId::generate(),
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    agent: "Designer".to_owned(),
    command,
    scope: None,
    server: server.to_owned(),
    action: "batch_design".to_owned(),
    target: Some("sign-in.pen".to_owned()),
    requested_at: at(10),
    native: None,
  };
  let decided = |request, decision| DecidedToolApproval {
    request,
    decision,
    decided_by: Some(DecidedBy::Person),
    decided_at: at(20),
    review: None,
    for_project: false,
    note: None,
  };
  let listed = outside_changes(&[
    decided(request("pencil", false), Some(ToolApprovalDecision::Approved)),
    decided(request("pencil", false), Some(ToolApprovalDecision::Denied)),
    decided(request("pencil", true), Some(ToolApprovalDecision::Approved)),
    decided(
      request(orchester_core::orchester_tools::SERVER, false),
      Some(ToolApprovalDecision::Approved),
    ),
    decided(request("linear", false), None),
  ]);
  assert_eq!(listed.len(), 1, "{listed:?}");
  assert_eq!(
    (
      &*listed[0].server,
      &*listed[0].tool,
      listed[0].target.as_deref(),
      listed[0].by
    ),
    ("pencil", "batch_design", Some("sign-in.pen"), Some(DecidedBy::Person))
  );
  assert_eq!(listed[0].at, at(20));
}
