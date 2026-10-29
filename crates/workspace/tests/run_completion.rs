//! Runs whose result is not a commit (ORC-93), against a real repository: a run that passed every step and committed
//! nothing waits to be completed when its task or pipeline says it delivers a report or work outside the repository,
//! and is flagged when changes were expected; completing keeps the last step's report as the run's *Report* and sets
//! the task Done; the Orchestrator completes such a run on its own as it would merge one, never a flagged one. A
//! closed task never waits and counts in Done; closing it settles its run's waiting completion, however it was closed
//! (ORC-150). `cargo test -p orchester-workspace --test run_completion completion`.

mod support;

use std::{
  collections::{HashMap, HashSet},
  fs,
  path::Path,
  process,
};

use orchester_core::{
  id::{InvocationId, ProjectId, RunId, ServerId, StepExecutionId, TaskId},
  resource::{Deliverable, StepKey},
  run::{
    CompletedBy, DecidedBy, DecidedToolApproval, PendingToolApproval, REPORT_ARTIFACT, Run, StepOutcome, StepOutput,
    TaskPhase, ToolApprovalDecision,
  },
  sharing::MemberRole,
  store::{ActivityStore, TaskStore},
  task::organization::{ChangeActor, WorkflowStatus},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{CachedTask, ProjectDb, SharingLink};
use orchester_workspace::{
  orchestrator::{
    auto_merge::completes_in,
    waits::{MergeChecks, Switches, follow_in},
  },
  tasks::{
    completion::{complete_in, outside_changes, settle_closed_in},
    data::ProjectTasks,
    delivery::Delivery,
    detail::TaskDetail,
    list::{self, TaskTab},
    needs_you::WaitingKind,
    views::ListView,
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

/// How many tasks Needs you, Active and Done hold, as the Tasks list counts them.
fn view_counts(tasks: &ProjectTasks) -> [usize; 3] {
  let rows = list::rows(&tasks.tasks, tasks, TaskTab::All);
  let waiting = tasks.waiting_tasks();
  [ListView::NeedsYou, ListView::Active, ListView::Done].map(|view| view.count(&rows, &waiting))
}

fn settle(fixture: &Fixture) -> bool {
  settle_closed_in((fixture.repo.path(), &Programs::processes())).expect("settled")
}

fn completion_of(fixture: &Fixture, run: RunId) -> Option<(CompletedBy, Timestamp)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  db.run_completion(run)
    .expect("read")
    .map(|completion| (completion.by, completion.completed_at))
}

#[test]
fn completion_a_closed_task_whose_report_waits_is_in_done_and_closing_settles_it() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let (task, run) = nothing_committed(
    &mut fixture,
    ("Write the report", Some(Deliverable::Report)),
    (&base, "Found it."),
  );
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read");
  assert_eq!(view_counts(&tasks), [1, 1, 0], "it waits to be completed");
  fixture
    .db
    .close_task_organization(task, WorkflowStatus::Done, (ChangeActor::Person, at(40)))
    .expect("closed");
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read again");
  assert!(waiting(&tasks).is_empty(), "a closed task waits for nothing");
  assert_eq!(view_counts(&tasks), [0, 0, 1], "it is in Done only");

  assert!(settle(&fixture));
  assert_eq!(completion_of(&fixture, run.id()), Some((CompletedBy::Person, at(40))));
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  assert_eq!(db.run_artifacts(run.id()).expect("artifacts")[0].text, "Found it.");
  assert_eq!(
    db.task(task).expect("read").expect("task").organization().status(),
    WorkflowStatus::Done
  );
  assert!(!settle(&fixture), "once is enough");

  fixture
    .db
    .update_task_organization(task, ChangeActor::Person, at(50), |value| {
      value.with_status(WorkflowStatus::Todo)
    })
    .expect("reopened");
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read again");
  assert!(
    waiting(&tasks).is_empty(),
    "reopened, its completed run asks nothing again"
  );
  let rows = list::rows(&tasks.tasks, &tasks, TaskTab::All);
  assert_eq!(rows[0].detail.key, "tasks.progress.completed");
  let page = TaskDetail::load_settled(fixture.repo.path(), task, &programs).expect("the task reads");
  assert!(page.result.and_then(|result| result.completion).is_some());
}

#[test]
fn completion_closing_over_mcp_settles_as_automation_and_a_run_with_commits_is_left() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let (canceled, run) = nothing_committed(
    &mut fixture,
    ("Draw the page", Some(Deliverable::Outside)),
    (&base, "Drew it."),
  );
  fixture
    .db
    .update_task_organization(canceled, ChangeActor::Orchester, at(41), |value| {
      value.with_status(WorkflowStatus::Canceled)
    })
    .expect("canceled over MCP");
  let (merged_later, committed) = nothing_committed(
    &mut fixture,
    ("Report with commits", Some(Deliverable::Report)),
    (&base, "x"),
  );
  let repo = fixture.repo.path();
  git(repo, &["checkout", "--quiet", committed.branch().as_str()]);
  fs::write(repo.join("work.md"), "work\n").expect("a file");
  git(repo, &["add", "--all"]);
  git(repo, &["commit", "--quiet", "-m", "work"]);
  git(repo, &["checkout", "--quiet", "main"]);
  fixture
    .db
    .close_task_organization(merged_later, WorkflowStatus::Done, (ChangeActor::Person, at(42)))
    .expect("closed");

  assert!(settle(&fixture));
  assert_eq!(
    completion_of(&fixture, run.id()),
    Some((CompletedBy::Automation, at(41)))
  );
  assert_eq!(
    ProjectDb::open(fixture.repo.path())
      .expect("open")
      .task(canceled)
      .expect("read")
      .expect("task")
      .organization()
      .status(),
    WorkflowStatus::Canceled,
    "settling leaves the status as it was closed"
  );
  assert_eq!(
    completion_of(&fixture, committed.id()),
    None,
    "work to merge is no completion"
  );
}

/// Shares the project with `task` as the server has it, organized as `organization`, last changed at `changed`.
fn shared(fixture: &mut Fixture, task: TaskId, (organization, changed): (WorkflowStatus, Timestamp)) {
  let found = fixture.db.task(task).expect("read").expect("task");
  let details = found.details();
  let value = fixture
    .db
    .task_organization(task)
    .expect("organization")
    .with_status(organization);
  let cached = CachedTask {
    id: task,
    number: found.key().number(),
    revision: found.revision(),
    title: details.title.clone(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: None,
    limits: details.limits,
    status: found.status(),
    organization: value,
    module: None,
    former_key: None,
    created_by: "Alice Moreau".to_owned(),
    created_at: found.created_at(),
    updated_by: "Alice Moreau".to_owned(),
    updated_at: changed,
    claim: None,
    run: None,
  };
  if fixture.db.sharing().expect("read").is_some() {
    fixture.db.cache_shared_tasks(&[cached], None).expect("cached");
    return;
  }
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: found.key().project().clone(),
    role: Some(MemberRole::Member),
    joined_at: at(1),
    cursor: 0,
    last_synced_at: at(1),
    removed_at: None,
    address: Some("https://orchester.kontur.lan/kontur".into()),
  };
  fixture
    .db
    .start_sharing(&link, &[], (&[cached], &HashMap::new()))
    .expect("shared");
}

#[test]
fn completion_a_member_closing_the_task_on_the_team_server_settles_it() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let (task, run) = nothing_committed(
    &mut fixture,
    ("Platform report", Some(Deliverable::Report)),
    (&base, "Done."),
  );
  shared(&mut fixture, task, (WorkflowStatus::InProgress, at(35)));
  assert!(!settle(&fixture), "an open task waits for the person");
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read");
  assert_eq!(waiting(&tasks), [WaitingKind::ReadyToComplete]);

  shared(&mut fixture, task, (WorkflowStatus::Done, at(45)));
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read again");
  assert_eq!(view_counts(&tasks), [0, 0, 1], "the server's Done wins over the run");
  assert!(settle(&fixture));
  assert_eq!(completion_of(&fixture, run.id()), Some((CompletedBy::Member, at(45))));
}

/// OP-26: the Orchestrator completes a shared task's report run; the task's Done waits for the team server, and
/// meanwhile the completed run waits for no one.
#[test]
fn completion_a_shared_task_the_orchestrator_completed_waits_for_nothing_before_the_servers_done() {
  let mut fixture = Fixture::new();
  let base = repository(&fixture);
  let (task, run) = nothing_committed(
    &mut fixture,
    ("Platform report", Some(Deliverable::Report)),
    (&base, "Done."),
  );
  shared(&mut fixture, task, (WorkflowStatus::InProgress, at(35)));
  complete_in(
    (fixture.repo.path(), &Programs::processes()),
    run.id(),
    (CompletedBy::Orchestrator, false),
    at(40),
  )
  .expect("completed");
  let programs = Programs::processes();
  let tasks = ProjectTasks::load(fixture.repo.path(), &programs).expect("the tasks read");
  let status = tasks.tasks[0].organization().status();
  assert_eq!(status, WorkflowStatus::InProgress, "the server has not taken Done yet");
  assert!(waiting(&tasks).is_empty(), "a completed run waits for no one");
  assert_eq!(view_counts(&tasks), [0, 1, 0]);
}
