//! The variables Project settings › Environment adds, which reach the agent CLI and the command
//! tools of every run unchanged, end to end with `orchester-fake-codex` and
//! `orchester-test-command`. A variable that names a folder is passed as written, like any other.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test environment
//! environment`.

mod support;

use std::collections::BTreeMap;

use orchester_core::{
  connection::stream::AgentEvent,
  id::{IdempotencyKey, RunId},
  resource::{CommandEnvironment, Draft, ResourceKind, Tool, ToolAction, VariableName},
  run::{Run, RunStatus},
  store::{ActivityStore, ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{Setup, StepSpec, TestProject, project, reported, scheduler, tools::ToolSpec, wait_for};

async fn run_to_the_end(project: &TestProject) -> Run {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await
}

/// `BUILD_DIR` set to an absolute folder beside the worktrees.
fn build_dir(project: &TestProject) -> (String, CommandEnvironment) {
  let folder = project.repo.path().join(".orchester").join("build");
  let folder = folder.to_str().expect("a Unicode temporary path").to_owned();
  let name = VariableName::new("BUILD_DIR").expect("valid name");
  let environment = CommandEnvironment::new(BTreeMap::from([(name, folder.clone())])).expect("valid variables");
  (folder, environment)
}

/// What the run's command tool printed.
fn command_output(db: &ProjectDb, run: RunId) -> String {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .find_map(|entry| match entry.event {
      AgentEvent::ToolCommandFinished { output, .. } => Some(output),
      _ => None,
    })
    .expect("the command finished")
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_additions_reach_the_agent_cli_and_the_command_tool() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Append one line.", &[]),
      StepSpec::new("checks", "Checks", "", &["implement"]).command(ToolSpec::test_command(&[
        "env",
        "BUILD_DIR",
        "env",
        "GH_TOKEN",
      ])),
    ],
    ..Setup::new("Say hello.")
  };
  let project = project(&setup);
  let root = project.repo.path();
  let (folder, environment) = build_dir(&project);
  ProjectDb::open(root)
    .expect("open")
    .save_project_environment(&environment)
    .expect("save the project's environment");
  let run = run_to_the_end(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, run.id(), "build dir: "), vec![folder.clone()]);
  let output = command_output(&db, run.id());
  assert!(
    output.lines().any(|line| line == format!("BUILD_DIR={folder}")),
    "{output}"
  );
  // Only what the project added passes; the rest of the environment stays cleaned.
  assert!(output.lines().any(|line| line == "GH_TOKEN is not set"), "{output}");
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_parallel_worktrees_get_the_same_value() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("left", "Left", "Write own file.", &[]),
      StepSpec::new("right", "Right", "Write own file.", &[]),
    ],
    ..Setup::new("Build separately.")
  });
  let (folder, environment) = build_dir(&project);
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  db.save_project_environment(&environment).expect("save");
  let run = run_to_the_end(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(reported(&db, run.id(), "build dir: "), vec![folder.clone(), folder]);
}

#[tokio::test(flavor = "multi_thread")]
async fn environment_command_tool_variables_pass_unchanged() {
  let project = project(&Setup {
    steps: vec![StepSpec::new("checks", "Checks", "", &[]).command(ToolSpec::test_command(&["env", "BUILD_DIR"]))],
    ..Setup::new("Build in a folder of its own.")
  });
  let (folder, environment) = build_dir(&project);
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
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
  let output = command_output(&db, run.id());
  assert!(
    output.lines().any(|line| line == format!("BUILD_DIR={folder}")),
    "{output}"
  );
}
