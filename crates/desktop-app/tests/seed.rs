//! The QA seed (`examples/seed`): every scenario builds a new repository in a temporary folder and registers
//! its project, and what it wrote reads back through the store the way the app reads it: the task list's
//! tabs and phases, the runs' checkpoints, worktrees and transcripts, the live connection, and the schema a
//! newer build left. Needs `git`.

#[path = "../examples/seed/kontur/mod.rs"]
mod kontur;

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
  time::Duration,
};

use kontur::{SeedError, SeedRequest, Seeded, scenarios};
use orchester_core::{
  command::CommandRunner,
  connection::{ConnectionKind, ConnectionStatus, stream::AgentEvent},
  diff::{
    Diff,
    git::{RevisionChanges, WorktreeRead, read_revision_diff, read_worktree_changes},
  },
  integration::WORKTREES_DIR,
  resource::{AgentProfile, Pipeline, ResourceKind},
  run::{PreflightInput, Run, TaskPhase, preflight},
  store::{
    ActivityStore, ConnectionStore, ConversationStore, MergeStore, ResourceStore, RunStore, TaskStore, WorktreeStore,
  },
  task::Task,
  task_group::GroupHold,
  time::Timestamp,
};
use orchester_exec::ProcessRunner;
use orchester_store::{AppDb, PreviousSession, ProjectDb, StoreError};
use tempfile::TempDir;

const GIT: &str = "git";
const RUNNER: ProcessRunner = ProcessRunner::new(Duration::from_secs(60));

/// A scenario seeded in a temporary folder that holds the app's folder and the repository.
struct Seed {
  _folder: TempDir,
  seeded: Seeded,
}

impl Seed {
  fn of(name: &str) -> Self {
    Self::with(name, None).expect("the scenario seeds")
  }

  fn with(name: &str, fake_codex: Option<PathBuf>) -> Result<Self, SeedError> {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let request = SeedRequest {
      data_dir: folder.path().join("bin"),
      repo: folder.path().join(format!("repo-{name}")),
      scenario: scenarios::named(name).expect("a known scenario"),
      fake_codex,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request)?;
    Ok(Self {
      _folder: folder,
      seeded,
    })
  }

  fn repo(&self) -> &Path {
    &self.seeded.repo
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo()).expect("the project database opens")
  }
}

/// Each task by key, with its runs as the store lists them.
fn by_key(db: &ProjectDb) -> BTreeMap<String, Task> {
  db.tasks()
    .expect("tasks")
    .into_iter()
    .map(|task| (task.key().to_string(), task))
    .collect()
}

/// The latest run of the task with `key`; `None` without one, `Err` when it cannot be read back.
fn latest(db: &ProjectDb, key: &str) -> Option<Result<Run, ()>> {
  let task = &by_key(db)[key];
  let runs = db.runs_of_task(task.id()).expect("runs");
  runs.into_iter().last().map(|run| run.map_err(drop))
}

/// Where the task with `key` stands, as the task list reads it: its latest run's phase, a run whose agent
/// waits for a command decision waiting too; `None` when its latest run cannot be read.
fn phase(db: &ProjectDb, key: &str) -> Option<TaskPhase> {
  let task = &by_key(db)[key];
  let Some(run) = latest(db, key) else {
    return Some(TaskPhase::of(task, None, None));
  };
  let run = run.ok()?;
  let merge = db.run_merge(run.id()).expect("merge");
  let phase = TaskPhase::of(task, Some(&run), merge.as_ref());
  let asks = !db.pending_tool_approvals(run.id()).expect("approvals").is_empty();
  Some(if phase == TaskPhase::Running && asks {
    TaskPhase::Waiting
  } else {
    phase
  })
}

fn workflow_counts(db: &ProjectDb) -> BTreeMap<String, usize> {
  let mut counts = BTreeMap::new();
  for task in db.tasks().expect("tasks") {
    let status = db.task_organization(task.id()).expect("organization").status();
    *counts.entry(format!("{status:?}")).or_insert(0) += 1;
  }
  counts
}

#[test]
fn seed_every_scenario_builds_a_repository_and_registers_kontur() {
  for scenario in scenarios::ALL
    .into_iter()
    .filter(|scenario| !scenario.needs_fake_codex())
  {
    let seed = Seed::of(scenario.name());
    let exclude = std::fs::read_to_string(seed.repo().join(".git/info/exclude")).expect("exclude file");
    assert!(
      exclude.lines().any(|line| line == "/.orchester/"),
      "{}",
      scenario.name()
    );
    let commits = RUNNER
      .run(&orchester_core::diff::git::git_command(
        Path::new(GIT),
        seed.repo(),
        ["rev-list", "--count", "main"],
      ))
      .expect("git runs");
    assert!(
      commits.stdout.trim().parse::<u32>().expect("a count") >= 1,
      "{}",
      scenario.name()
    );
    let projects = AppDb::open(&seed.seeded.app_database)
      .expect("app database")
      .projects()
      .expect("projects");
    let [project] = projects.as_slice() else {
      panic!("one project registered for {}: {projects:?}", scenario.name());
    };
    assert_eq!((project.id, project.name.as_str()), (seed.seeded.project, "Kontur"));
    assert_eq!(project.repo_root, seed.seeded.repo);
  }
}

#[test]
fn seed_names_every_scenario_once() {
  let names: Vec<&str> = scenarios::ALL.iter().map(|scenario| scenario.name()).collect();
  assert_eq!(
    names,
    [
      "empty",
      "tasks",
      "task-states",
      "conversation",
      "changes",
      "interrupted",
      "unreadable-db",
      "live"
    ]
  );
  for name in names {
    assert_eq!(scenarios::named(name).map(|scenario| scenario.name()), Some(name));
  }
  assert!(scenarios::named("Tasks").is_none());
}

#[test]
fn seed_empty_has_the_catalogue_and_no_task() {
  let db = Seed::of("empty").db();
  assert!(db.tasks().expect("tasks").is_empty());
  let pipelines: Vec<String> = db
    .resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .iter()
    .map(|resource| resource.name().as_str().to_owned())
    .collect();
  assert_eq!(pipelines, ["Feature delivery", "Quick fix"]);
  assert_eq!(db.project().key().as_str(), "KON");
}

#[test]
fn seed_tasks_fills_the_design_tabs_and_phases() {
  let seed = Seed::of("tasks");
  let db = seed.db();
  let tasks = by_key(&db);
  assert_eq!(tasks.len(), 24);
  let expected = [
    ("Backlog", 3),
    ("Canceled", 1),
    ("Done", 2),
    ("InProgress", 8),
    ("InReview", 2),
    ("Todo", 8),
  ];
  assert_eq!(
    workflow_counts(&db),
    expected.map(|(status, count)| (status.to_owned(), count)).into()
  );
  let phases: BTreeMap<&str, Option<TaskPhase>> = [
    "KON-1", "KON-3", "KON-8", "KON-9", "KON-10", "KON-11", "KON-12", "KON-13", "KON-14", "KON-16", "KON-21", "KON-23",
    "KON-24",
  ]
  .into_iter()
  .map(|key| (key, phase(&db, key)))
  .collect();
  use TaskPhase::{Draft, Failed, Merged, Queued, Running, StoppedAtLimit, Stuck, Succeeded, Waiting};
  let want = [
    ("KON-1", Some(Merged)),
    ("KON-3", Some(Draft)),
    ("KON-8", Some(Queued)),
    ("KON-9", Some(Succeeded)),
    ("KON-10", Some(Waiting)),
    ("KON-11", Some(Failed)),
    ("KON-12", Some(Waiting)),
    ("KON-13", Some(Waiting)),
    ("KON-14", Some(Stuck)),
    ("KON-16", Some(Running)),
    ("KON-21", Some(Running)),
    ("KON-23", Some(StoppedAtLimit)),
    ("KON-24", None),
  ];
  assert_eq!(phases, want.into());
  let needs_you = tasks
    .keys()
    .filter(|key| phase(&db, key).is_some_and(TaskPhase::needs_you))
    .count();
  assert_eq!(needs_you, 4);
  let readable: usize = tasks
    .values()
    .map(|task| {
      db.runs_of_task(task.id())
        .expect("runs")
        .iter()
        .filter(|run| run.is_ok())
        .count()
    })
    .sum();
  assert_eq!(readable, 13, "KON-12 has three runs; KON-24's cannot be read");
  assert_eq!(seed.seeded.tasks.len(), 24);
  let reported = &seed.seeded.tasks[11];
  assert_eq!(
    (reported.key.as_str(), reported.title.as_str()),
    ("KON-12", "Add retry to webhook sender")
  );
}

#[test]
fn seed_tasks_holds_a_group_and_blocks_a_task() {
  let db = Seed::of("tasks").db();
  let tasks = by_key(&db);
  let group = db
    .task_group_for_task(tasks["KON-17"].id())
    .expect("group")
    .expect("KON-17 is grouped");
  assert_eq!(group.tasks(), [tasks["KON-11"].id(), tasks["KON-17"].id()]);
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::FailedTask)
  );
  assert_eq!(
    db.unfinished_blocking_tasks(tasks["KON-19"].id()).expect("blocking"),
    [tasks["KON-18"].id()]
  );
  let kon_12 = db.runs_of_task(tasks["KON-12"].id()).expect("runs");
  assert_eq!(kon_12.len(), 3);
  let removed = db.worktree_removals(tasks["KON-12"].id()).expect("removals");
  assert_eq!(removed.len(), 1, "the failed first run's worktree was removed");
  let approvals = db.pending_tool_approvals_all().expect("approvals");
  assert_eq!(
    approvals
      .iter()
      .map(|approval| approval.action.as_str())
      .collect::<Vec<_>>(),
    ["cargo add rand"]
  );
}

#[test]
fn seed_task_states_leaves_one_task_in_each_state() {
  let seed = Seed::of("task-states");
  let db = seed.db();
  use TaskPhase::{
    Cancelled, Draft, Failed, Interrupted, Merged, Paused, Queued, Running, StoppedAtLimit, Stuck, Succeeded, Waiting,
  };
  let want = [
    Some(Draft),
    Some(Queued),
    Some(Running),
    Some(Paused),
    Some(StoppedAtLimit),
    Some(StoppedAtLimit),
    Some(Interrupted),
    Some(Failed),
    Some(Cancelled),
    Some(Waiting),
    Some(Waiting),
    Some(Waiting),
    Some(Waiting),
    Some(Stuck),
    Some(Succeeded),
    Some(Merged),
    None,
    Some(Draft),
  ];
  let phases: Vec<Option<TaskPhase>> = (1..=want.len())
    .map(|number| phase(&db, &format!("KON-{number}")))
    .collect();
  assert_eq!(phases, want);
  let plans = |key: &str| {
    let run = latest(&db, key).expect("a run").expect("readable");
    db.plans(run.id()).expect("plans").len()
  };
  assert_eq!(
    (plans("KON-12"), plans("KON-13")),
    (1, 2),
    "plan v1, and v2 after changes were asked for"
  );
  let states: Vec<&str> = seed.seeded.tasks.iter().map(|task| task.state).collect();
  assert_eq!(states[0], "draft, ready to queue");
  assert_eq!(states[16], "latest run unreadable");
}

#[test]
fn seed_conversation_records_a_long_transcript_with_a_mid_turn_message() {
  let db = Seed::of("conversation").db();
  let run = latest(&db, "KON-12").expect("a run").expect("readable");
  let activity = db.activity(run.id(), 0).expect("activity");
  assert!(activity.len() > 60, "a long transcript: {}", activity.len());
  let failed = activity
    .iter()
    .any(|entry| matches!(&entry.event, AgentEvent::CommandFinished { exit_code: Some(code), .. } if *code != 0));
  assert!(failed, "a command that failed");
  assert!(
    activity
      .iter()
      .any(|entry| matches!(entry.event, AgentEvent::Error { .. }))
  );
  assert!(matches!(
    activity.last().map(|entry| &entry.event),
    Some(AgentEvent::CommandStarted { .. })
  ));
  let messages = db.messages(run.task()).expect("messages");
  assert!(
    messages
      .iter()
      .any(|message| message.delivery().and_then(|delivery| delivery.arrival())
        == Some(orchester_core::conversation::Arrival::MidTurn))
  );
  assert!(
    db.pending_tool_approvals(run.id()).expect("approvals").is_empty(),
    "the command was approved"
  );
  assert_eq!(phase(&db, "KON-12"), Some(TaskPhase::Running));
}

/// What the Changes tab reads of `run`: its checkpointed files (or `None` when too large) and what its
/// worktree holds beyond it.
fn changes(repo: &Path, db: &mut ProjectDb, run: &Run) -> (Option<Option<usize>>, WorktreeRead) {
  let checkpointed = run.last_checkpoint().map(|checkpoint| {
    let read = read_revision_diff(
      &RUNNER,
      Path::new(GIT),
      repo,
      db,
      (run.base(), checkpoint.revision()),
      Timestamp::now(),
    )
    .expect("git reads the checkpoint");
    match read {
      RevisionChanges::Read(diff) => Some(Diff::parse(&diff.text).expect("a diff").files().len()),
      RevisionChanges::TooLarge(_) => None,
    }
  });
  let worktree = repo.join(WORKTREES_DIR).join(run.id().to_string());
  let uncommitted = if worktree.is_dir() {
    read_worktree_changes(&RUNNER, Path::new(GIT), &worktree)
      .expect("git reads the worktree")
      .read
  } else {
    WorktreeRead::Clean
  };
  (checkpointed, uncommitted)
}

fn files(read: &WorktreeRead) -> Option<usize> {
  match read {
    WorktreeRead::Read(text) => Some(Diff::parse(text).expect("a diff").files().len()),
    WorktreeRead::Clean | WorktreeRead::TooLarge(_) => None,
  }
}

#[test]
fn seed_changes_has_checkpointed_and_uncommitted_work_in_every_form() {
  let seed = Seed::of("changes");
  let mut db = seed.db();
  let run = |db: &ProjectDb, key: &str| latest(db, key).expect("a run").expect("readable");
  let kon_12 = run(&db, "KON-12");
  let (checkpointed, uncommitted) = changes(seed.repo(), &mut db, &kon_12);
  assert_eq!((checkpointed, files(&uncommitted)), (Some(Some(4)), Some(2)));
  let kon_16 = run(&db, "KON-16");
  assert_eq!(changes(seed.repo(), &mut db, &kon_16), (None, WorktreeRead::Clean));
  let kon_25 = run(&db, "KON-25");
  assert_eq!(
    changes(seed.repo(), &mut db, &kon_25),
    (Some(None), WorktreeRead::Clean)
  );
  let kon_26 = run(&db, "KON-26");
  let (checkpointed, uncommitted) = changes(seed.repo(), &mut db, &kon_26);
  assert_eq!(checkpointed, Some(Some(1)));
  assert!(matches!(uncommitted, WorktreeRead::TooLarge(_)));
  let kon_27 = run(&db, "KON-27");
  let (checkpointed, uncommitted) = changes(seed.repo(), &mut db, &kon_27);
  assert_eq!((checkpointed, files(&uncommitted)), (None, Some(2)));
  let kon_1 = run(&db, "KON-1");
  assert!(!seed.repo().join(WORKTREES_DIR).join(kon_1.id().to_string()).exists());
  assert!(db.worktree_removal(kon_1.id()).expect("removal").is_some());
}

#[test]
fn seed_interrupted_leaves_runs_running_and_a_session_without_an_end() {
  let seed = Seed::of("interrupted");
  let running = seed.db().running_runs().expect("running runs");
  // KON-24's run cannot be read; KON-10 and KON-13 wait for a person, so no agent was cut off there.
  let cut_off = running
    .iter()
    .filter_map(|run| run.as_ref().ok())
    .filter(|run| !run.waits_for_a_person())
    .count();
  assert_eq!(
    (running.len(), cut_off),
    (6, 3),
    "KON-12, KON-16 and KON-21 lost their agents"
  );
  let mut app = AppDb::open(&seed.seeded.app_database).expect("app database");
  let (_, previous) = app.start_session(1, Timestamp::now()).expect("a new session");
  assert_eq!(previous, PreviousSession::Crashed);
}

#[test]
fn seed_unreadable_db_fails_with_schema_too_new() {
  let seed = Seed::of("unreadable-db");
  let opened = ProjectDb::open(seed.repo());
  assert!(
    matches!(opened, Err(StoreError::SchemaTooNew { found, supported }) if found == supported + 1),
    "{:?}",
    opened.err()
  );
}

#[test]
fn seed_live_runs_the_fake_codex_by_its_absolute_path() {
  // The seed only stores the program: any file stands in for it, and a relative path is made absolute.
  let relative = PathBuf::from("Cargo.toml");
  let seed = Seed::with("live", Some(relative.clone())).expect("live seeds");
  let db = seed.db();
  let [connection] = db
    .connections()
    .expect("connections")
    .try_into()
    .expect("one connection");
  assert!(matches!(connection.kind(), ConnectionKind::Agent(_)));
  let program = connection.executable().expect("a program");
  assert!(program.is_absolute());
  assert_eq!(program, std::path::absolute(&relative).expect("absolute"));
  assert_eq!(connection.status(), ConnectionStatus::Connected);
  let profiles: Vec<String> = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .iter()
    .map(|resource| resource.name().as_str().to_owned())
    .collect();
  assert_eq!(profiles, ["Builder", "Explorer", "Planner", "Reviewer"]);
  for resource in db.resources(ResourceKind::AgentProfile).expect("profiles") {
    let profile = db
      .latest_version::<AgentProfile>(resource.id())
      .expect("read")
      .expect("published");
    assert_eq!(profile.body().connection(), connection.id());
  }
  let feature = db
    .resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .into_iter()
    .find(|resource| resource.name().as_str() == "Feature delivery")
    .expect("Feature delivery");
  let pipeline = db
    .latest_version::<Pipeline>(feature.id())
    .expect("read")
    .expect("published");
  let gated: Vec<&str> = pipeline
    .body()
    .steps()
    .iter()
    .filter(|step| step.approval_gate)
    .map(|step| step.key.as_str())
    .collect();
  assert_eq!(gated, ["plan"]);
  let profiles: BTreeMap<_, _> = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .iter()
    .map(|resource| {
      let version = db
        .latest_version::<AgentProfile>(resource.id())
        .expect("read")
        .expect("published");
      (resource.id(), version)
    })
    .collect();
  let connections = db.connections().expect("connections");
  for resource in db.resources(ResourceKind::Pipeline).expect("pipelines") {
    let version = db
      .latest_version::<Pipeline>(resource.id())
      .expect("read")
      .expect("published");
    let problems = preflight(PreflightInput {
      pipeline: Some(version.body()),
      profiles: &profiles,
      tools: &BTreeMap::new(),
      connections: &connections,
    });
    assert!(
      problems.is_empty(),
      "{} can run: {problems:?}",
      resource.name().as_str()
    );
  }
  let tasks = db.tasks().expect("tasks");
  assert_eq!(tasks.len(), 6);
  assert!(
    tasks.iter().all(|task| task.details().missing().is_empty()),
    "every draft is ready to run"
  );
  assert!(
    tasks
      .iter()
      .any(|task| task.details().brief.contains("Escalate `cargo add serde`"))
  );
}

#[test]
fn seed_live_needs_the_fake_codex_and_writes_nothing_without_it() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let repo = folder.path().join("repo-live");
  let request = SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: repo.clone(),
    scenario: scenarios::named("live").expect("live"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  assert!(matches!(kontur::seed(&request), Err(SeedError::FakeCodexRequired)));
  assert!(!repo.exists());
}

#[test]
fn seed_never_writes_into_a_folder_that_holds_anything() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let repo = folder.path().join("repo");
  std::fs::create_dir(&repo).expect("folder");
  std::fs::write(repo.join("notes.txt"), "mine").expect("a file");
  let request = SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: repo.clone(),
    scenario: scenarios::named("empty").expect("empty"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  assert!(matches!(kontur::seed(&request), Err(SeedError::RepoNotEmpty(_))));
  assert!(!repo.join(".git").exists());
  assert_eq!(std::fs::read_to_string(repo.join("notes.txt")).expect("kept"), "mine");
}
