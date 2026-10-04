//! A run with a tool step end to end, headlessly, on the real runner: Implement on
//! `orchester-fake-codex`, then Checks running a command tool (`orchester-test-command`). The task
//! page lists the tool step with its tool and no tokens, and the Activity tab shows the command
//! and its exit code.

use std::{env, path::Path};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{CommandAction, CommandArg, CommandProgram, PromptText, Slug, Step, StepKey, StepKind, Tool},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  conversation::open_group,
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, finished_on_page, has, init_repository, programs, seed_steps},
};

/// The harmless test program, built by [`programs`] beside the fake Codex CLI.
fn test_command(fake_codex: &Path) -> String {
  fake_codex
    .with_file_name("orchester-test-command")
    .with_extension(env::consts::EXE_EXTENSION)
    .to_str()
    .expect("a Unicode target folder")
    .to_owned()
}

/// A published command tool "Print lines" running the test program.
fn seed_tool(repo: &Path, program: &str) -> ResourceId {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let now = Timestamp::now();
  let action = CommandAction::new(
    CommandProgram::new(program).expect("program"),
    ["print", "2"]
      .map(|arg| CommandArg::new(arg).expect("argument"))
      .to_vec(),
  );
  let tool = db
    .create_resource(
      Slug::new("print-lines").expect("slug"),
      Name::new("Print lines").expect("name"),
      &Tool::command("", action),
      now,
    )
    .expect("tool")
    .id();
  db.publish_draft::<Tool>(tool, now).expect("publish the tool");
  tool
}

fn tool_step(tool: ResourceId) -> Step {
  Step {
    key: StepKey::new("checks").expect("key"),
    name: Name::new("Checks").expect("name"),
    kind: StepKind::Tool { tool: Some(tool) },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: [StepKey::new("implement").expect("key")].into(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

#[test]
fn tool_run_shows_the_command_step_and_its_output() {
  let (runner, fake) = programs();
  let program = test_command(&fake);
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    let tool = seed_tool(repo, &program);
    seed_steps(repo, &fake, |builder| {
      vec![
        agent_step(builder, "implement", "Implement", "Append one line.", &[]),
        tool_step(tool),
      ]
    });
  });
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  finished_on_page(&mut desktop);
  assert!(shows(&mut desktop, "task-progress-steps", "Checks · Print lines"));
  assert!(!shows(&mut desktop, "task-progress-steps", "0 tokens"));
  desktop.click("task-tab-conversation");
  desktop.wait_until("the timeline", |desktop| {
    has(desktop, "activity-step-header") && shows(desktop, "conversation", "02 Checks · tool · ")
  });
  // The tool step's divider says a tool ran it, with no model and no tokens; its command is the
  // tool's own row, under the tool's name; the rail says it passed.
  let rows = crate::conversation::texts(&desktop, "conversation-agent-tools");
  assert!(
    rows
      .iter()
      .any(|row| row.contains("Print lines") && row.contains("tool · ")),
    "{rows:?}"
  );
  assert!(shows(&mut desktop, "conversation-rail-step-2", "Tool · passed"));
  // Picked in the rail, the tool step opens on its command's group.
  desktop.click("conversation-rail-step-2");
  open_group(&mut desktop, "Ran ");
  desktop.wait_until("the command entry", |desktop| has(desktop, "activity-entry"));
  assert!(shows(&mut desktop, "conversation", "exit 0"));
  assert!(shows(&mut desktop, "conversation", "orchester-test-command"));
}
