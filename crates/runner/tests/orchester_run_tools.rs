//! Orchester's tools besides the files end to end through the relay, with the fake CLIs: `get_task` answers the run's
//! task, `report_progress` keeps the step's line, `save_memory` keeps a task memory record, `find_references` lists
//! where a name is used and says the code index is off, `delete_file` and `move_file` change the worktree, and
//! `run_check` runs the command tool whose access rules offer it to the step, without asking as its rules say. Needs
//! `git`. Run with `cargo test -p orchester-runner --features test-agent --test orchester_run_tools
//! orchester_run_tools`.

mod support;

use std::collections::BTreeMap;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  memory::{MemoryFilter, MemoryStatus, MemoryStore, ScopeFilter},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{
    AccessPolicy, AccessWindow, AllowedSteps, ApprovalMode, CommandAction, CommandArg, CommandProgram, DeniedBehavior,
    Slug, Tool, ToolAction,
  },
  run::{Run, RunStatus},
  store::{ResourceStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{Setup, TestProject, project, reported, scheduler_with, tools::TEST_COMMAND, wait_for};

/// Publishes the check `checks`: the test command printing three lines, offered to every step of the task's
/// pipeline, approving no call.
fn offer_check(project: &TestProject) {
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  let pipeline = db
    .task(project.task)
    .expect("read")
    .and_then(|task| task.details().pipeline)
    .expect("the task's pipeline");
  let action = CommandAction::new(
    CommandProgram::new(TEST_COMMAND).expect("a program"),
    ["print", "3"]
      .map(|arg| CommandArg::new(arg).expect("an argument"))
      .to_vec(),
  );
  let access = AccessPolicy::new(
    BTreeMap::from([(pipeline, AllowedSteps::All)]),
    Name::new("Local").expect("a name"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("a policy");
  let tool = Tool::new("Runs the checks", ToolAction::Command(action), Some(access)).expect("a tool");
  let now = Timestamp::now();
  let id = db
    .create_resource(
      Slug::new("checks").expect("a slug"),
      Name::new("Checks").expect("a name"),
      &tool,
      now,
    )
    .expect("create the check")
    .id();
  db.publish_draft::<Tool>(id, now).expect("publish the check");
}

async fn finished(project: &TestProject) -> (Run, ProjectDb) {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id: RunId = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.pending_tool_approvals(id).expect("requests").is_empty(),
    "nothing asked"
  );
  (run, db)
}

/// What every CLI's calls left: the progress line, the memory record, and a worktree without the moved file.
fn assert_records(project: &TestProject, run: &Run, db: &ProjectDb) {
  let execution = run.current_attempt().expect("an attempt").executions()[0].id();
  let progress = db.step_progress(execution).expect("read").expect("reported");
  assert_eq!(progress.0.as_str(), "checking the tools");
  let filter = MemoryFilter {
    scope: ScopeFilter::Task(project.task),
    status: None,
    pinned_only: false,
  };
  let memories = db.memories(&filter).expect("read");
  let kept = memories
    .iter()
    .find(|memory| memory.content().key().map(|key| key.as_str()) == Some("readme"))
    .expect("the note");
  assert_eq!(kept.status(), MemoryStatus::Active);
  assert_eq!(kept.content().text(), "The README greets.");
  let worktree = project
    .repo
    .path()
    .join(".orchester")
    .join("worktrees")
    .join(run.id().to_string());
  assert!(!worktree.join("notes").join("a.md").exists());
  assert!(!worktree.join("notes").join("b.md").exists());
}

fn permissions(commands: CommandPermission) -> AgentPermissions {
  AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::On)
}

/// Claude Code calls every tool by its allow rule: none asks, the check included, since its rules approve no call.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_run_tools_serve_claude_code() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(CommandPermission::Allowed(AllowedCommands::default()))),
    ..Setup::new("Use the run tools.")
  });
  offer_check(&project);
  let (run, db) = finished(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let id = run.id();
  let listed = reported(&db, id, "orchester-tools: ").pop().expect("listed");
  for tool in [
    "delete_file",
    "move_file",
    "find_references",
    "run_check",
    "get_task",
    "save_memory",
    "report_progress",
  ] {
    assert!(listed.contains(tool), "{tool}: {listed}");
  }
  let decisions = reported(&db, id, "orchester-permission: ");
  assert!(
    decisions.iter().all(|line| line.ends_with("allow-by-rule")),
    "{decisions:#?}"
  );
  let calls = reported(&db, id, "orchester-call: ");
  let expect = [
    ("get_task ok", "# ORC-1 · Say hello"),
    ("report_progress ok", "reported"),
    ("save_memory ok", "(active"),
    ("find_references ok", "Definitions (code index):"),
    ("create_file ok", "created notes/a.md"),
    ("move_file ok", "moved notes/a.md to notes/b.md"),
    ("delete_file ok", "deleted notes/b.md"),
    ("run_check ok", "exit code: 0"),
  ];
  assert_eq!(calls.len(), expect.len(), "{calls:#?}");
  for (call, (start, holds)) in calls.iter().zip(expect) {
    assert!(
      call.starts_with(start) && call.contains(holds),
      "{start} … {holds}: {call}"
    );
  }
  assert!(calls[7].ends_with("line 3"), "{calls:#?}");
  assert_records(&project, &run, &db);
}

/// Codex calls the same tools through its configuration's server, every one approved there, and the runner runs each.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_run_tools_serve_codex() {
  let project = project(&Setup {
    permissions: Some(permissions(CommandPermission::AnyInWorktree)),
    ..Setup::new("Use the run tools.")
  });
  offer_check(&project);
  let (run, db) = finished(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let id = run.id();
  let calls = reported(&db, id, "orchester-call: ");
  let starts: Vec<&str> = calls
    .iter()
    .map(|call| call.split(':').next().unwrap_or_default())
    .collect();
  assert_eq!(
    starts,
    [
      "get_task ok",
      "report_progress ok",
      "save_memory ok",
      "find_references ok",
      "create_file ok",
      "move_file ok",
      "delete_file ok",
      "run_check ok"
    ],
    "{calls:#?}"
  );
  let answers = reported(&db, id, "orchester-answer| ");
  for told in [
    "## Acceptance criteria",
    "Uses:",
    "README.md",
    "[the code index is off for this run: definitions are not told apart from uses]",
    "line 3",
  ] {
    assert!(answers.iter().any(|line| line.contains(told)), "{told}: {answers:#?}");
  }
  assert_records(&project, &run, &db);
}
