#[path = "tasks/browse.rs"]
mod browse;
#[path = "tasks/pipelines.rs"]
mod pipelines;
mod support;

use std::{fs, sync::Arc, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::StepKey,
  run::{RunLimit, TaskPhase},
  store::TaskStore,
  task::{MissingDetail, Task, TaskDetails},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_desktop::{
  display,
  tasks::{
    data::{ProjectTasks, save_draft},
    detail::TaskDetail,
    form::TaskForm,
    list::{self, DraftSummary, Readiness, TaskTab},
    progress::{ProgressState, progress},
  },
};
use orchester_store::ProjectDb;
use tempfile::TempDir;

fn form<'a>(title: &'a str, criteria: &'a [String], budget: &'a str, time_limit: &'a str) -> TaskForm<'a> {
  TaskForm {
    title,
    brief: "  Let people find a task.  ",
    criteria,
    pipeline: None,
    budget,
    time_limit,
    separator: ",",
    max_budget: TokenBudget::new(10_000_000).expect("above zero"),
  }
}

fn project_db() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create the project database");
  (repo, db)
}

fn details(title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("valid title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  }
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

#[test]
fn form_maps_entered_values_to_task_details() {
  let criteria = vec!["  Filters as you type. ".to_owned(), "   ".to_owned()];
  let details = form(" Add task search ", &criteria, "1,400,000", "30")
    .details()
    .expect("valid form");
  assert_eq!(details.title.as_str(), "Add task search");
  assert_eq!(details.brief, "Let people find a task.");
  assert_eq!(details.acceptance_criteria, vec!["Filters as you type.".to_owned()]);
  let limits = details.limits.expect("both limits entered");
  assert_eq!(limits.budget().get(), 1_400_000);
  assert_eq!(limits.max_duration(), Duration::from_secs(30 * 60));
}

#[test]
fn form_allows_an_incomplete_draft_and_names_what_is_missing() {
  let blank_rows = vec![String::new()];
  let incomplete = TaskForm {
    brief: "",
    ..form("Add task search", &blank_rows, "", "")
  };
  let details = incomplete
    .details()
    .expect("a draft may leave everything but the title out");
  assert_eq!(details.limits, None);
  assert!(details.acceptance_criteria.is_empty());
  assert_eq!(
    incomplete.missing(),
    vec![
      MissingDetail::Brief,
      MissingDetail::AcceptanceCriteria,
      MissingDetail::Pipeline,
      MissingDetail::Limits
    ]
  );
}

#[test]
fn form_reports_missing_details_while_the_title_is_blank() {
  let criteria = vec!["Filters as you type.".to_owned()];
  let blank_title = form("  ", &criteria, "5", "30");
  let errors = blank_title.details().expect_err("a title is required");
  assert_eq!(errors.title.map(|message| message.key), Some("errors.titleRequired"));
  assert_eq!(blank_title.missing(), vec![MissingDetail::Pipeline]);
}

#[test]
fn form_rejects_invalid_and_half_entered_limits() {
  let error_keys = |budget: &str, time_limit: &str| {
    let errors = form("Task", &[], budget, time_limit)
      .details()
      .expect_err("limits are invalid");
    (
      errors.budget.map(|message| message.key),
      errors.time_limit.map(|message| message.key),
    )
  };
  assert_eq!(error_keys("5", ""), (None, Some("errors.timeLimitRequired")));
  assert_eq!(error_keys("", "30"), (Some("tokens.errors.empty"), None));
  assert_eq!(
    error_keys("1.4M", "half an hour"),
    (Some("tokens.errors.digits"), Some("errors.timeLimitInvalid"))
  );
  assert_eq!(error_keys("0", "30"), (Some("tokens.errors.zero"), None));
  assert_eq!(error_keys("12,000,000", "30"), (Some("tokens.errors.aboveMax"), None));
  assert_eq!(error_keys("5", "0"), (None, Some("errors.timeLimitZero")));
}

#[test]
fn retrying_a_save_with_the_same_key_creates_one_task() {
  let (_repo, mut db) = project_db();
  let batch = IdempotencyKey::generate();
  let first = save_draft(&mut db, batch, details("Add task search"), now()).expect("first attempt");
  let retry = save_draft(&mut db, batch, details("Add task search"), now()).expect("retry");
  assert_eq!(retry.id(), first.id());
  assert_eq!(db.tasks().expect("read tasks").len(), 1);
}

#[test]
fn a_retry_after_editing_keeps_the_task_and_the_edits() {
  let (_repo, mut db) = project_db();
  let batch = IdempotencyKey::generate();
  // The first attempt was stored although the form saw it fail, e.g. the response was lost.
  let stored = save_draft(&mut db, batch, details("Add task serch"), now()).expect("first attempt");
  let edited = TaskDetails {
    brief: "Let people find a task.".into(),
    ..details("Add task search")
  };
  let retried = save_draft(&mut db, batch, edited.clone(), now()).expect("retry");
  assert_eq!(retried.id(), stored.id());
  let tasks = db.tasks().expect("read tasks");
  assert_eq!(tasks.len(), 1);
  assert_eq!(tasks[0].details(), &edited);
  assert_eq!(tasks[0].key().to_string(), "ORC-1");
}

#[test]
fn a_new_key_creates_the_next_task() {
  let (_repo, mut db) = project_db();
  save_draft(&mut db, IdempotencyKey::generate(), details("First"), now()).expect("first task");
  let second = save_draft(&mut db, IdempotencyKey::generate(), details("Second"), now()).expect("second task");
  assert_eq!(second.key().to_string(), "ORC-2");
}

fn stored_tasks() -> Vec<Task> {
  let (_repo, mut db) = project_db();
  save_draft(&mut db, IdempotencyKey::generate(), details("Incomplete"), now()).expect("draft");
  let mut queued = db
    .create_tasks(IdempotencyKey::generate(), vec![details("To queue")], now())
    .expect("create")
    .remove(0);
  // Queueing is not in the UI yet; a queued task still must not appear under Drafts.
  let complete = TaskDetails {
    brief: "Brief.".into(),
    acceptance_criteria: vec!["Works.".into()],
    pipeline: Some(ResourceId::generate()),
    limits: Some(
      orchester_core::task::TaskLimits::new(TokenBudget::new(1).expect("above zero"), Duration::from_secs(60))
        .expect("positive limits"),
    ),
    ..details("To queue")
  };
  queued = Task::from_stored(
    queued.id(),
    queued.key().clone(),
    complete,
    queued.status(),
    queued.pipeline_version(),
    queued.created_at(),
  );
  queued
    .queue(orchester_core::resource::VersionNumber::FIRST)
    .expect("complete draft can be queued");
  db.save_task(&queued).expect("save queued task");
  db.tasks().expect("read tasks")
}

#[test]
fn tabs_filter_and_count_tasks() {
  let tasks = stored_tasks();
  let count = |tab| list::rows(&tasks, &[], &Default::default(), tab).len();
  assert_eq!(count(TaskTab::All), 2);
  assert_eq!(count(TaskTab::Backlog), 2);
  for other in [TaskTab::Todo, TaskTab::InProgress, TaskTab::InReview, TaskTab::Done] {
    assert_eq!(count(other), 0);
  }
  let drafts = list::rows(&tasks, &[], &Default::default(), TaskTab::Backlog);
  assert_eq!(drafts.len(), 2);
  assert_eq!(&*drafts[0].title, "Incomplete");
  assert_eq!(drafts[0].phase, Some(orchester_core::run::TaskPhase::Draft));
  assert_eq!(
    DraftSummary::of(&tasks),
    DraftSummary {
      ready: 0,
      incomplete: 1
    }
  );
  let recent = list::recent(&tasks, &[], &Default::default(), 1);
  assert_eq!(&*recent[0].title, "To queue");
}

#[test]
fn readiness_names_one_missing_detail_and_counts_several() {
  assert_eq!(Readiness::Ready.message().key, "tasks.readiness.ready");
  assert_eq!(
    Readiness::Missing(vec![MissingDetail::Pipeline]).message().key,
    "tasks.readiness.missingPipeline"
  );
  let several = Readiness::Missing(vec![MissingDetail::Brief, MissingDetail::Limits]).message();
  assert_eq!(several.key, "tasks.readiness.missingCount");
  assert_eq!(several.args, vec![("count", "2".to_owned())]);
}

#[test]
fn elapsed_time_uses_the_largest_whole_unit() {
  let at = |millis: i64| Timestamp::from_unix_millis(1_700_000_000_000 + millis);
  let key = |millis: i64| display::elapsed(at(millis), at(0));
  assert_eq!(key(59_000).key, "time.justNow");
  assert_eq!(key(-5_000).key, "time.justNow");
  assert_eq!(key(4 * 60_000).args, vec![("count", "4".to_owned())]);
  assert_eq!(key(4 * 60_000).key, "time.minutesAgo");
  assert_eq!(key(2 * 3_600_000).key, "time.hoursAgo");
  assert_eq!(key(3 * 86_400_000).key, "time.daysAgo");
}

#[test]
fn form_names_one_problem_per_message_for_invalid_limits() {
  let with_limits = |budget: &'static str, time_limit: &'static str| form("Task", &[], budget, time_limit);
  let invalid = with_limits("five", "30");
  let errors = invalid.details().expect_err("the budget is invalid");
  assert!(!invalid.missing().contains(&MissingDetail::Limits));
  assert_eq!(
    errors
      .save_reasons()
      .iter()
      .map(|reason| reason.key)
      .collect::<Vec<_>>(),
    ["tasks.new.limitsBlockSave"]
  );
  let half_entered = with_limits("5", "");
  assert!(!half_entered.missing().contains(&MissingDetail::Limits));
  assert!(with_limits("", "").missing().contains(&MissingDetail::Limits));

  let blank_title = form(" ", &[], "5", "0")
    .details()
    .expect_err("title and time limit are invalid");
  assert_eq!(
    blank_title
      .save_reasons()
      .iter()
      .map(|reason| reason.key)
      .collect::<Vec<_>>(),
    ["errors.titleRequired", "tasks.new.limitsBlockSave"]
  );
}

#[test]
fn task_list_keeps_task_order_and_overview_shows_newest_first() {
  let tasks = stored_tasks();
  let titles = |rows: Vec<list::TaskRow>| rows.iter().map(|row| row.title.to_string()).collect::<Vec<_>>();
  assert_eq!(
    titles(list::rows(&tasks, &[], &Default::default(), TaskTab::All)),
    ["Incomplete", "To queue"]
  );
  assert_eq!(
    titles(list::recent(&tasks, &[], &Default::default(), 2)),
    ["To queue", "Incomplete"]
  );
}

#[test]
fn task_list_footer_counts_tasks_and_started_runs() {
  let footer = |tasks, runs| list::footer(tasks, runs);
  assert_eq!(footer(10, 0).key, "tasks.footer.noRuns.other");
  assert_eq!(footer(1, 0).key, "tasks.footer.noRuns.one");
  assert_eq!(footer(3, 1).key, "tasks.footer.oneRun.other");
  assert_eq!(footer(1, 2).key, "tasks.footer.runs.one");
  assert_eq!(
    footer(3, 4).args,
    vec![("count", "3".to_owned()), ("runs", "4".to_owned())]
  );
}

#[test]
fn task_list_reads_run_counts_and_interrupts_running_runs_without_a_runner() {
  let mut fixture = support::Fixture::new();
  let first = fixture.task("Add login", support::at(10));
  let second = fixture.task("Fix zoom", support::at(11));
  let mut earlier = fixture.started_run(first, support::at(20));
  fixture.succeed(&mut earlier, support::at(30));
  fixture.started_run(first, support::at(40));
  fixture.started_run(second, support::at(50));
  let loaded = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  assert_eq!(loaded.runs, 3);
  assert_eq!(
    list::footer(loaded.tasks.len(), loaded.runs).key,
    "tasks.footer.runs.other"
  );
  let latest = loaded.phases[&first].run().expect("a readable run");
  assert_eq!(latest.phase, TaskPhase::Running);
  assert_eq!(latest.number.get(), 2);
  assert_eq!(latest.updated_at, support::at(40));

  let away = loaded.without_runner();
  assert_eq!(away.phases[&first].phase(), Some(TaskPhase::Interrupted));
  assert_eq!(away.phases[&second].phase(), Some(TaskPhase::Interrupted));
  assert_eq!(
    away.phases[&first].message().key,
    "tasks.progress.interruptedNoCheckpoint"
  );
  assert_eq!(away.runs, 3);
}

#[test]
fn task_page_run_recorded_running_lost_its_agent_without_a_runner() {
  let mut fixture = support::Fixture::new();
  let running = fixture.task("Fix zoom", support::at(10));
  fixture.started_run(running, support::at(20));
  let done = fixture.task("Add login", support::at(30));
  let mut run = fixture.started_run(done, support::at(40));
  fixture.succeed(&mut run, support::at(50));

  let detail = TaskDetail::load_in(fixture.repo.path(), running).expect("read the task");
  assert!(!detail.lost_its_runner(true));
  assert!(detail.lost_its_runner(false));
  let settled = TaskDetail::load_in(fixture.repo.path(), done).expect("read the task");
  assert_eq!(settled.phase(), TaskPhase::Succeeded);
  assert!(!settled.lost_its_runner(false));
}

/// A run cancelled before its first attempt reads back into the task list and the task page: the
/// task is cancelled and none of its steps waits.
#[test]
fn task_list_and_page_read_a_run_cancelled_before_it_started() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Fix zoom", support::at(10));
  let mut run = fixture.pending_run(task, support::at(20));
  let events = run.cancel(support::at(21)).expect("cancel the pending run");
  fixture.record(&run, &events);

  let loaded = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  assert_eq!(loaded.runs, 1);
  assert_eq!(loaded.phases[&task].phase(), Some(TaskPhase::Cancelled));
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("read the task");
  let run = detail.run.expect("the task has a run");
  let states: Vec<_> = progress(&run, false, support::at(30))
    .into_iter()
    .map(|row| row.state)
    .collect();
  assert_eq!(states, [ProgressState::Cancelled]);
}

#[test]
fn waiting_parallel_steps_keep_separate_progress_rows() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  fixture.started_run(task, support::at(20));
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("read task");
  let mut run = detail.run.expect("task has a run");
  for (number, key, name) in [(2, "checks", "Run checks"), (3, "review", "Review")] {
    let mut step = run.steps[0].clone();
    step.number = number;
    step.stage = 1;
    step.key = StepKey::new(key).expect("valid step key");
    step.name = Arc::from(name);
    step.execution = None;
    step.status = None;
    run.steps.push(step);
  }

  let rows = progress(&run, false, support::at(30));
  assert_eq!(rows.len(), 3);
  assert_eq!(
    (&*rows[1].id, &*rows[1].step, rows[1].state),
    ("checks", "Run checks", ProgressState::Waiting)
  );
  assert_eq!(
    (&*rows[2].id, &*rows[2].step, rows[2].state),
    ("review", "Review", ProgressState::Waiting)
  );
  assert_eq!(rows[1].agent.as_deref(), Some("Builder"));
  assert_eq!(rows[2].agent.as_deref(), Some("Builder"));
}

/// While a run is stopped at any limit, its working step reads "Stopped at limit", never "Paused":
/// Paused is only a pause the user asked for (design doc, "Stopped at limit").
#[test]
fn task_page_progress_reads_stopped_at_limit_for_every_limit_and_paused_for_a_pause() {
  let mut fixture = support::Fixture::new();
  let step_states = |fixture: &support::Fixture, task| {
    let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("read the task");
    let run = detail.run.expect("the task has a run");
    progress(&run, false, support::at(100))
      .into_iter()
      .map(|row| row.state)
      .collect::<Vec<_>>()
  };
  for (at, limit) in [
    (10, RunLimit::Tokens),
    (20, RunLimit::Time),
    (30, RunLimit::DailyBudget),
  ] {
    let task = fixture.task("Fix zoom", support::at(at));
    let mut run = fixture.started_run(task, support::at(at + 1));
    let events = run
      .stop_at_limit(limit, support::at(at + 2))
      .expect("stop at the limit");
    fixture.record(&run, &events);
    assert_eq!(
      step_states(&fixture, task),
      [ProgressState::StoppedAtLimit],
      "{limit:?}"
    );
  }
  let task = fixture.task("Add login", support::at(40));
  let mut run = fixture.started_run(task, support::at(41));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run
    .stop_at_limit(RunLimit::StepBudget { execution }, support::at(42))
    .expect("stop at the step budget");
  fixture.record(&run, &events);
  assert_eq!(step_states(&fixture, task), [ProgressState::StoppedAtLimit]);

  let task = fixture.task("Add search", support::at(50));
  let mut run = fixture.started_run(task, support::at(51));
  let events = run.pause(support::at(52)).expect("pause");
  fixture.record(&run, &events);
  assert_eq!(step_states(&fixture, task), [ProgressState::Paused]);
}
