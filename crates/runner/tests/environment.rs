//! The variables Project settings › Environment adds, which reach the agent CLI and the command
//! tools of every run, end to end with `orchester-fake-codex` and `orchester-test-command`. Needs
//! `git`. Run with `cargo test -p orchester-runner --features test-agent --test environment
//! environment`.

mod support;

use std::collections::BTreeMap;

use orchester_core::{
  connection::stream::AgentEvent,
  id::IdempotencyKey,
  resource::{CommandEnvironment, VariableName},
  run::RunStatus,
  store::{ActivityStore, SettingsStore},
};
use orchester_store::ProjectDb;

use self::support::{Setup, StepSpec, TestProject, project, reported, scheduler, tools::ToolSpec, wait_for};

async fn run_to_the_end(project: &TestProject) -> orchester_core::run::Run {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
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
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let names = reported(&db, run.id(), "env: ");
  assert!(
    names[0].split(',').any(|name| name == "CARGO_TARGET_DIR"),
    "the agent CLI sees it: {names:?}"
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
    output.lines().any(|line| line == format!("CARGO_TARGET_DIR={target}")),
    "{output}"
  );
  // Only what the project added passes; the rest of the environment stays cleaned.
  assert!(output.lines().any(|line| line == "GH_TOKEN is not set"), "{output}");
}
