//! The task list's reading reads the project's runs for the whole project at once (ORC-742): it gives the same
//! `ProjectTasks` as reading them task by task did, and its statements do not grow with the project's tasks.

mod support;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId, TaskId},
  integration::{MergeKind, RunMerge},
  resource::{Pipeline, ResourceKind},
  revision::Revision,
  run::{
    BranchName, CompletedBy, InterruptCause, Run, RunCompletion, RunLimit, StepFailure, TaskPhase, WaitReason,
    WorktreeFailure,
  },
  store::{MergeStore, ResourceStore, TaskStore, WorktreeStore},
  task::{
    TaskDetails, TaskLimits,
    organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  },
  task_group::{GroupHold, ReviewedPipeline, Scheduling, TaskGroup},
  tokens::TokenBudget,
};
use orchester_store::statements_run;
use orchester_workspace::tasks::{data::ProjectTasks, list::Latest};
use rusqlite::Connection;
use support::{Fixture, at};

fn execution(run: &Run) -> orchester_core::id::StepExecutionId {
  run.current_attempt().expect("attempt").executions()[0].id()
}

/// Sets `assignments` on the row of `id` in `table`, as an edit or damage outside Orchester would.
fn tamper(fixture: &Fixture, (table, id): (&str, String), assignments: &str) {
  Connection::open(fixture.repo.path().join(".orchester/orchester.db"))
    .expect("open raw database")
    .execute(&format!("UPDATE {table} SET {assignments} WHERE id = ?1"), [id])
    .expect("tamper");
}

/// The events of one or more of a run's transitions, in order.
type Transitions = Vec<Vec<orchester_core::run::RunEvent>>;

/// A task with one run started at `now`, which `then` takes on from `now + 2`.
fn task_with(
  fixture: &mut Fixture,
  title: &str,
  now: i64,
  then: impl FnOnce(&mut Run, i64) -> Transitions,
) -> (TaskId, Run) {
  let task = fixture.task(title, at(now));
  let mut run = fixture.started_run(task, at(now + 1));
  let events: Vec<_> = then(&mut run, now + 2).into_iter().flatten().collect();
  fixture.record(&run, &events);
  (task, run)
}

/// From `base` on: a draft and a queued task without runs, and runs pending, stuck, paused, stopped at a limit and
/// interrupted with and without their worktree. The draft, the pending and the stuck task.
fn stopped_runs(fixture: &mut Fixture, base: i64) -> (TaskId, TaskId, TaskId) {
  let draft = fixture.task("Draft", at(base));
  fixture.queued_task("Queued", at(base + 1));
  let pending = fixture.queued_task("Pending", at(base + 2));
  fixture.pending_run(pending, at(base + 3));
  let (stuck, _) = task_with(fixture, "Stuck", base + 10, |run, now| {
    let broke = run
      .fail_step(execution(run), StepFailure::AgentFailed, at(now))
      .expect("broke");
    let restart = execution(run);
    let mut again = run.start_step(restart, at(now + 1)).expect("restarted");
    again.extend(
      run
        .fail_step(restart, StepFailure::AgentFailed, at(now + 2))
        .expect("broke again"),
    );
    vec![broke, again]
  });
  task_with(fixture, "Paused", base + 20, |run, now| {
    let asked = run.request_pause(at(now)).expect("asked");
    vec![asked, run.pause(at(now + 1)).expect("paused")]
  });
  task_with(fixture, "At limit", base + 30, |run, now| {
    vec![run.stop_at_limit(RunLimit::Tokens, at(now)).expect("stopped")]
  });
  task_with(fixture, "Interrupted", base + 40, |run, now| {
    vec![run.interrupt(InterruptCause::Crashed, at(now)).expect("interrupted")]
  });
  let (_, removed) = task_with(fixture, "Worktree removed", base + 50, |run, now| {
    vec![run.interrupt(InterruptCause::Crashed, at(now)).expect("interrupted")]
  });
  fixture
    .db
    .save_worktree_removal(removed.id(), at(base + 55))
    .expect("removed");
  (draft, pending, stuck)
}

/// From `base` on: runs succeeded, completed, merged, failed to start and cancelled. The succeeded and the merged
/// task.
fn finished_runs(fixture: &mut Fixture, base: i64) -> (TaskId, TaskId) {
  let (succeeded, _) = task_with(fixture, "Succeeded", base + 60, |run, now| {
    vec![run.complete_step(execution(run), at(now)).expect("passed")]
  });
  let (_, completed) = task_with(fixture, "Completed", base + 70, |run, now| {
    vec![run.complete_step(execution(run), at(now)).expect("passed")]
  });
  let completion = RunCompletion {
    run: completed.id(),
    by: CompletedBy::Person,
    completed_at: at(base + 75),
  };
  fixture.db.save_run_completion(&completion, None).expect("completed");
  let (merged, merged_run) = task_with(fixture, "Merged", base + 80, |run, now| {
    vec![run.complete_step(execution(run), at(now)).expect("passed")]
  });
  let merge = RunMerge {
    run: merged_run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: at(base + 85),
  };
  fixture.db.save_run_merge(&merge).expect("merged");
  let failed = fixture.task("Failed", at(base + 90));
  let mut never_started = fixture.pending_run(failed, at(base + 91));
  let events = never_started
    .fail_to_start(WorktreeFailure::NoFreeBranch, at(base + 92))
    .expect("failed to start");
  fixture.record(&never_started, &events);
  let cancelled = fixture.task("Cancelled", at(base + 100));
  let mut earlier = fixture.started_run(cancelled, at(base + 101));
  let events = earlier.cancel(at(base + 102)).expect("cancelled");
  fixture.record(&earlier, &events);
  let mut again = fixture.started_run(cancelled, at(base + 103));
  let events = again.cancel(at(base + 104)).expect("cancelled");
  fixture.record(&again, &events);
  (succeeded, merged)
}

/// From `base` on: a task whose latest run cannot be read, one whose earlier run cannot, and a task kept *Not shared*
/// when the project joined, with its run and the tag names it kept. The first task.
fn unreadable_and_kept(fixture: &mut Fixture, base: i64) -> TaskId {
  let unreadable = fixture.task("Unreadable", at(base + 110));
  let bad = fixture.started_run(unreadable, at(base + 111));
  tamper(fixture, ("runs", bad.id().to_string()), "status = 'pending'");
  let after_unreadable = fixture.task("After an unreadable run", at(base + 120));
  let bad = fixture.started_run(after_unreadable, at(base + 121));
  tamper(fixture, ("runs", bad.id().to_string()), "snapshot = 'not json'");
  let mut later = fixture.started_run(after_unreadable, at(base + 122));
  fixture.succeed(&mut later, at(base + 123));
  // A task kept *Not shared* when the project joined, with its run and the tag names it kept.
  let kept = fixture.task("Not shared", at(base + 130));
  let mut kept_run = fixture.started_run(kept, at(base + 131));
  fixture.succeed(&mut kept_run, at(base + 132));
  tamper(
    fixture,
    ("tasks", kept.to_string()),
    r#"sharing = 'not_shared', kept_key = 'OLD', kept_tag_names = '{"schema":1,"body":["CI"]}'"#,
  );
  unreadable
}

/// One task of every kind whose runs have ended or wait without an agent at work, created from `base` on: drafts and
/// queued tasks, runs pending, stuck, paused, stopped at a limit, interrupted with and without their worktree,
/// succeeded, completed, merged, failed, cancelled and unreadable, a task kept *Not shared*, and blocking links.
fn ended_tasks(fixture: &mut Fixture, base: i64) {
  let (draft, pending, stuck) = stopped_runs(fixture, base);
  let (succeeded, merged) = finished_runs(fixture, base);
  let unreadable = unreadable_and_kept(fixture, base);
  let links = [
    (succeeded, DependencyGate::Review),
    (succeeded, DependencyGate::Merge),
    (merged, DependencyGate::Merge),
    (stuck, DependencyGate::Review),
    (pending, DependencyGate::Review),
    (unreadable, DependencyGate::Review),
  ];
  let organization = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::None,
    None,
    links.iter().map(|&(task, gate)| BlockingTask { task, gate }).collect(),
    Vec::new(),
  )
  .expect("organization");
  fixture
    .db
    .set_task_organization(draft, &organization, ChangeActor::Person, at(base + 140))
    .expect("linked");
}

/// A complete draft titled `title` on the fixture's pipeline, which a reviewed group can queue.
fn complete_draft(fixture: &mut Fixture, title: &str, base: i64) -> TaskId {
  let pipeline = fixture.db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let details = TaskDetails {
    title: orchester_core::name::Name::new(title).expect("title"),
    brief: "Implement this.".into(),
    acceptance_criteria: vec!["It works.".into()],
    pipeline: Some(pipeline),
    limits: TaskLimits::new(
      Some(TokenBudget::new(1_000_000).expect("budget")),
      Some(std::time::Duration::from_secs(1_800)),
    )
    .expect("limits"),
  };
  fixture
    .db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(base))
    .expect("draft")[0]
    .id()
}

/// The tasks whose runs have an agent at work, and a reviewed group that holds its work: a running run that
/// auto-approves, one that waits for an answer, and a group whose one task's latest run is its first and whose other
/// task ran again.
fn live_tasks(fixture: &mut Fixture) -> (TaskId, TaskId, RunId) {
  fixture.connect_builder(AgentProvider::CodexCli);
  let running = fixture.task("Running", at(10));
  let run = fixture.started_run(running, at(11));
  fixture
    .db
    .set_run_auto_approve(run.id(), Some(true))
    .expect("auto-approves");
  task_with(fixture, "Waiting", 20, |run, now| {
    vec![
      run
        .wait_step(execution(run), WaitReason::Message, at(now))
        .expect("waits"),
    ]
  });
  let first = complete_draft(fixture, "Grouped first", 30);
  let again = complete_draft(fixture, "Grouped again", 31);
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    vec![first, again],
    Scheduling::Sequential,
    TokenBudget::new(2_000_000).expect("cap"),
  )
  .expect("group");
  let reviewed: Vec<_> = [first, again]
    .into_iter()
    .map(|task| {
      let pipeline = fixture
        .db
        .task(task)
        .expect("task")
        .expect("exists")
        .details()
        .pipeline
        .expect("pipeline");
      let version = fixture
        .db
        .latest_version::<Pipeline>(pipeline)
        .expect("version")
        .expect("published")
        .number();
      ReviewedPipeline {
        task,
        pipeline,
        version,
      }
    })
    .collect();
  fixture.db.queue_task_group(&group, None, &reviewed).expect("queued");
  fixture
    .db
    .set_task_group_hold(group.key(), Some(&GroupHold::PreviousTask))
    .expect("held");
  fixture.first_run(first, at(32));
  let mut ran = fixture.first_run(again, at(33));
  let events = ran.cancel(at(34)).expect("cancelled");
  fixture.record(&ran, &events);
  fixture.started_run(again, at(35));
  (first, again, run.id())
}

fn read(fixture: &Fixture) -> ProjectTasks {
  ProjectTasks::read(&fixture.db, fixture.repo.path(), None).expect("the tasks read")
}

#[test]
fn tasks_reading_gives_what_reading_task_by_task_gives() {
  let mut fixture = Fixture::new();
  let (first, again, running) = live_tasks(&mut fixture);
  ended_tasks(&mut fixture, 100);

  let read = read(&fixture);
  let by_task = ProjectTasks::read_task_by_task(&fixture.db, fixture.repo.path(), None).expect("read task by task");
  assert_eq!(read, by_task);

  // The seed covers what the reading reads.
  let phases: std::collections::BTreeSet<_> = read
    .phases
    .values()
    .filter_map(Latest::phase)
    .map(|phase| format!("{phase:?}"))
    .collect();
  for phase in [
    TaskPhase::Queued,
    TaskPhase::Running,
    TaskPhase::Waiting,
    TaskPhase::Stuck,
    TaskPhase::Paused,
    TaskPhase::StoppedAtLimit,
    TaskPhase::Interrupted,
    TaskPhase::Succeeded,
    TaskPhase::Merged,
    TaskPhase::Failed,
    TaskPhase::Cancelled,
  ] {
    assert!(phases.contains(&format!("{phase:?}")), "a run is {phase:?}: {phases:?}");
  }
  assert_eq!(
    read.phases.values().filter(|latest| latest.phase().is_none()).count(),
    1
  );
  assert!(
    read
      .phases
      .values()
      .any(|latest| matches!(latest, Latest::Run(run) if run.completed))
  );
  assert!(
    read
      .phases
      .values()
      .any(|latest| matches!(latest, Latest::Run(run) if run.ended_for_run_again))
  );
  assert!(read.group_holds.contains_key(&first) && !read.group_holds.contains_key(&again));
  assert_eq!(
    read.blocking.values().map(Vec::len).max(),
    Some(4),
    "four of six links are unfinished"
  );
  assert_eq!(
    read.kept_tags.values().cloned().collect::<Vec<_>>(),
    [vec!["CI".to_owned()]]
  );
  assert_eq!(
    read.auto_approving.iter().map(|run| run.run).collect::<Vec<_>>(),
    [running]
  );
  assert!(
    read
      .run_facts
      .at_work
      .get(&running)
      .is_some_and(|at_work| at_work.agent.is_some())
  );
  assert_eq!(read.run_facts.merged_at.len(), 1);
  assert!(
    read.active.len() >= 6,
    "pending, running, waiting, stuck, paused, at limit, interrupted"
  );
}

/// Only the runs with an agent at work (running or waiting) and the reviewed groups are read one by one; the seed
/// keeps those and adds tasks of every other kind.
#[test]
fn tasks_reading_statements_do_not_grow_with_the_tasks() {
  let mut fixture = Fixture::new();
  live_tasks(&mut fixture);
  ended_tasks(&mut fixture, 100);
  let statements = |fixture: &Fixture| {
    let before = statements_run();
    let read = read(fixture);
    let read_at_once = statements_run() - before;
    let before = statements_run();
    ProjectTasks::read_task_by_task(&fixture.db, fixture.repo.path(), None).expect("read task by task");
    (read_at_once, statements_run() - before, read.tasks.len())
  };
  let (once, by_task_once, tasks) = statements(&fixture);
  ended_tasks(&mut fixture, 1_000);
  ended_tasks(&mut fixture, 2_000);
  let (thrice, by_task_thrice, more) = statements(&fixture);
  assert!(more > tasks * 2, "the project has more tasks: {tasks} then {more}");
  assert!(
    by_task_thrice > by_task_once,
    "reading task by task takes more statements for more tasks"
  );
  assert_eq!(
    once, thrice,
    "the reading takes as many statements for {more} tasks as for {tasks}"
  );
}
