//! The variables Project settings › Environment adds, which reach the agent CLI and the command
//! tools of every run, end to end with `orchester-fake-codex` and `orchester-test-command`. Needs
//! `git`. Run with `cargo test -p orchester-runner --features test-agent --test environment
//! environment`.

mod support;

use std::collections::BTreeMap;

use orchester_core::{
  connection::stream::AgentEvent,
  id::IdempotencyKey,
  resource::{CommandEnvironment, Draft, ResourceKind, Tool, ToolAction, VariableName},
  run::{PreflightProblem, RunStatus},
  store::{ActivityStore, ResourceStore, RunStore, SettingsStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use self::support::{Setup, StepSpec, TestProject, project, reported, scheduler, tools::ToolSpec, wait_for};

async fn run_to_the_end(project: &TestProject) -> orchester_core::run::Run {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_additions_reach_the_agent_cli_and_the_command_tool() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Append one line.", &[]),
      StepSpec::new("checks", "Checks", "", &["implement"]).command(ToolSpec::test_command(&[
        "env",
        "CARGO_TARGET_DIR",
        "env",
        "GH_TOKEN",
      ])),
    ],
    ..Setup::new("Say hello.")
  };
  let project = project(&setup);
  let root = project.repo.path();
  let target = root.join(".orchester").join("target");
  let target = target.to_str().expect("a Unicode temporary path").to_owned();
  let name = VariableName::new("CARGO_TARGET_DIR").expect("valid name");
  let environment = CommandEnvironment::new(BTreeMap::from([(name, target.clone())])).expect("valid variables");
  ProjectDb::open(root)
    .expect("open")
    .save_project_environment(&environment)
    .expect("save the project's environment");
  let run = run_to_the_end(&project).await;
  let effective = std::path::Path::new(&target)
    .join("worktrees")
    .join(run.id().to_string());
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let names = reported(&db, run.id(), "env: ");
  assert!(
    names[0].split(',').any(|name| name == "CARGO_TARGET_DIR"),
    "the agent CLI sees it: {names:?}"
  );
  assert_eq!(
    reported(&db, run.id(), "cargo target: "),
    vec![effective.to_string_lossy().into_owned()],
    "agent and subsequent check use the same worktree target"
  );
  let output = db
    .activity(run.id(), 0)
    .expect("activity")
    .into_iter()
    .find_map(|entry| match entry.event {
      AgentEvent::ToolCommandFinished { output, .. } => Some(output),
      _ => None,
    })
    .expect("the command finished");
  assert!(
    output
      .lines()
      .any(|line| line == format!("CARGO_TARGET_DIR={}", effective.display())),
    "{output}"
  );
  // Only what the project added passes; the rest of the environment stays cleaned.
  assert!(output.lines().any(|line| line == "GH_TOKEN is not set"), "{output}");
  assert_eq!(
    db.project_environment()
      .expect("stored configuration")
      .get("CARGO_TARGET_DIR"),
    Some(target.as_str()),
    "the configured build root is preserved"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_parallel_worktrees_have_distinct_cargo_targets() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("left", "Left", "Write own file.", &[]),
      StepSpec::new("right", "Right", "Write own file.", &[]),
    ],
    ..Setup::new("Build separately.")
  });
  let root = project.repo.path();
  let target = root.join(".orchester/target");
  let environment = CommandEnvironment::new(BTreeMap::from([(
    VariableName::new("CARGO_TARGET_DIR").expect("name"),
    target.to_string_lossy().into_owned(),
  )]))
  .expect("environment");
  let mut db = ProjectDb::open(root).expect("open");
  db.save_project_environment(&environment).expect("save");
  let run = run_to_the_end(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let mut actual = reported(&db, run.id(), "cargo target: ");
  actual.sort();
  let mut expected: Vec<_> = run.attempts()[0]
    .executions()
    .iter()
    .map(|execution| {
      target
        .join("worktrees")
        .join(execution.id().to_string())
        .to_string_lossy()
        .into_owned()
    })
    .collect();
  expected.sort();
  assert_eq!(actual, expected);
  assert_ne!(actual[0], actual[1]);
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_relative_cargo_target_stays_in_the_worktree() {
  let project = project(&Setup::new("Say hello."));
  let environment = CommandEnvironment::new(BTreeMap::from([(
    VariableName::new("CARGO_TARGET_DIR").expect("name"),
    "target".to_owned(),
  )]))
  .expect("environment");
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  db.save_project_environment(&environment).expect("save");
  let run = run_to_the_end(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(reported(&db, run.id(), "cargo target: "), vec!["target"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_command_tool_target_override_is_scoped_to_its_worktree() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("checks", "Checks", "", &[]).command(ToolSpec::test_command(&["env", "CARGO_TARGET_DIR"])),
    ],
    ..Setup::new("Build in an isolated directory.")
  });
  let root = project.repo.path();
  let target = root.join(".orchester/tool-target");
  let environment = CommandEnvironment::new(BTreeMap::from([(
    VariableName::new("CARGO_TARGET_DIR").expect("name"),
    target.to_string_lossy().into_owned(),
  )]))
  .expect("environment");
  let mut db = ProjectDb::open(root).expect("open");
  let tool = db.resources(ResourceKind::Tool).expect("tools")[0].id();
  let version = db.latest_version::<Tool>(tool).expect("read").expect("version");
  let ToolAction::Command(action) = version.body().action() else {
    panic!("the fixture uses a command tool");
  };
  let body = Tool::command("", action.clone().with_environment(environment));
  db.save_draft(tool, &Draft::new(body, Some(version.number()), Timestamp::now()))
    .expect("save draft");
  db.publish_draft::<Tool>(tool, Timestamp::now()).expect("publish");
  let run = run_to_the_end(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let expected = target.join("worktrees").join(run.id().to_string());
  let output = db
    .activity(run.id(), 0)
    .expect("activity")
    .into_iter()
    .find_map(|entry| match entry.event {
      AgentEvent::ToolCommandFinished { output, .. } => Some(output),
      _ => None,
    })
    .expect("command output");
  assert!(
    output
      .lines()
      .any(|line| line == format!("CARGO_TARGET_DIR={}", expected.display())),
    "{output}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_space_preflight_reports_an_unmeasurable_target_before_creating_a_run() {
  let project = project(&Setup::new("Do one task."));
  let root = project.repo.path();
  let blocker = root.join("not-a-directory");
  std::fs::write(&blocker, "x").expect("write blocker");
  let target = blocker.join("target");
  let environment = CommandEnvironment::new(BTreeMap::from([(
    VariableName::new("CARGO_TARGET_DIR").expect("name"),
    target.to_string_lossy().into_owned(),
  )]))
  .expect("valid environment");
  let mut db = ProjectDb::open(root).expect("open");
  db.save_project_environment(&environment).expect("save environment");
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::BuildSpaceUnavailable { target }],
    })
  );
  assert!(db.runs_of_task(project.task).expect("read runs").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_space_preflight_rejects_a_dangling_target_link() {
  let project = project(&Setup::new("Do one task."));
  let root = project.repo.path();
  let target = root.join("shared-target-link");
  let missing = root.join("missing-target-volume");
  #[cfg(windows)]
  {
    // A junction needs no symlink privilege on Windows. Remove its destination after creation.
    std::fs::create_dir(&missing).expect("create junction destination");
    let output = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(&target)
      .arg(&missing)
      .output()
      .expect("create junction");
    assert!(
      output.status.success(),
      "mklink /J: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir(&missing).expect("remove junction destination");
  }
  #[cfg(unix)]
  std::os::unix::fs::symlink(&missing, &target).expect("create dangling target symlink");
  assert!(
    std::fs::symlink_metadata(&target).is_ok(),
    "the link itself still exists"
  );
  let environment = CommandEnvironment::new(BTreeMap::from([(
    VariableName::new("CARGO_TARGET_DIR").expect("name"),
    target.to_string_lossy().into_owned(),
  )]))
  .expect("valid environment");
  let mut db = ProjectDb::open(root).expect("open");
  db.save_project_environment(&environment).expect("save environment");
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::BuildSpaceUnavailable { target }],
    })
  );
  assert!(db.runs_of_task(project.task).expect("read runs").is_empty());
}
