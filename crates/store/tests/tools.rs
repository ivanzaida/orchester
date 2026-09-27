//! Command tools in the project database (M10a): their bodies round-trip through draft and publish
//! and older bodies are upgraded when read; a command tool's run is an invocation of its own kind,
//! and a step that fails for its command keeps that reason.
//! Run with `cargo test -p orchester-store --test tools tools`.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, ToolCommandResult},
  resource::{
    CommandAction, CommandArg, CommandEnvironment, CommandProgram, CommandTimeout, Draft, ExitCodes, OutputLimit,
    ResourceKind, Slug, Tool, ToolAction, VariableName,
  },
  run::{InvocationKind, StepFailure},
  store::{ActivityStore, ResourceStore, RunStore},
};
use rusqlite::Connection;

use self::support::{at, model, name, started_run};

fn command_tool() -> Tool {
  let action = CommandAction::new(
    CommandProgram::new("cargo").expect("valid program"),
    ["test", "--workspace"]
      .map(|arg| CommandArg::new(arg).expect("valid argument"))
      .to_vec(),
  )
  .with_timeout(CommandTimeout::from_secs(300).expect("valid timeout"))
  .with_exit_codes(ExitCodes::new(BTreeSet::from([0, 2])).expect("valid codes"))
  .with_environment(
    CommandEnvironment::new(BTreeMap::from([(
      VariableName::new("RUST_LOG").expect("valid name"),
      "warn".to_owned(),
    )]))
    .expect("valid variables"),
  )
  .with_output_limit(OutputLimit::from_kib(512).expect("valid limit"));
  Tool::command("Runs the workspace tests.", action)
}

#[test]
fn tools_command_tool_round_trips_through_draft_and_publish() {
  let (_repo, mut db, ..) = started_run();
  let tool = command_tool();
  let id = db
    .create_resource(Slug::new("run-tests").expect("slug"), name("Run tests"), &tool, at(10))
    .expect("create")
    .id();
  assert_eq!(db.draft::<Tool>(id).expect("read").expect("draft").body(), &tool);
  let version = db.publish_draft::<Tool>(id, at(11)).expect("publish");
  assert_eq!(version.body(), &tool);
  assert_eq!(
    db.latest_version::<Tool>(id).expect("read").expect("version").body(),
    &tool
  );
  let edited = Tool::command(
    "",
    CommandAction::new(CommandProgram::new("git").expect("program"), Vec::new()),
  );
  db.save_draft(id, &Draft::new(edited.clone(), Some(version.number()), at(12)))
    .expect("save");
  assert_eq!(db.draft::<Tool>(id).expect("read").expect("draft").body(), &edited);
  assert_eq!(db.resources(ResourceKind::Tool).expect("list").len(), 1);
}

#[test]
fn tools_bodies_of_schema_1_are_upgraded_when_read() {
  let (repo, mut db, ..) = started_run();
  let id = db
    .create_resource(
      Slug::new("checks").expect("slug"),
      name("Checks"),
      &command_tool(),
      at(10),
    )
    .expect("create")
    .id();
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  let schema_1 = serde_json::json!({
    "schema": 1,
    "body": {
      "action": {"command": {"program": "git", "args": ["--version"]}},
      "access": {
        "callers": {},
        "environment": "Staging",
        "approval": "never",
        "window": "until_step_ends",
        "denied": "fail_step"
      }
    }
  });
  raw
    .execute("UPDATE resource_drafts SET body = ?1", [schema_1.to_string()])
    .expect("store a schema 1 body");
  let draft = db.draft::<Tool>(id).expect("read").expect("draft");
  let ToolAction::Command(command) = draft.body().action() else {
    panic!("a command tool");
  };
  assert_eq!(command.command_line(), "git --version");
  assert_eq!(command.timeout(), CommandTimeout::DEFAULT);
  assert!(draft.body().access().is_some());
}

#[test]
fn tools_command_invocations_are_recorded_as_commands() {
  let (_repo, mut db, run, execution) = started_run();
  let agent = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("agent");
  let mut command = db
    .start_command_invocation(run.id(), execution, at(20))
    .expect("command");
  assert_eq!((command.number, &command.kind), (2, &InvocationKind::Command));
  assert_eq!(command.model(), None);
  command.ended_at = Some(at(35));
  command.outcome = Some(InvocationOutcome::TimedOut);
  db.save_invocation(&command).expect("save");
  let events = [
    (
      at(21),
      AgentEvent::ToolCommandStarted {
        tool: "Run tests".into(),
        command: "cargo test".into(),
      },
    ),
    (
      at(35),
      AgentEvent::ToolCommandFinished {
        tool: "Run tests".into(),
        command: "cargo test".into(),
        result: ToolCommandResult::TimedOut,
        exit_code: None,
        output: "running 3 tests".into(),
        omitted_bytes: 12,
      },
    ),
  ];
  assert_eq!(db.append_activity(&command, &events).expect("append"), 2);
  let stored = db.invocations(run.id()).expect("read");
  assert_eq!(stored, [agent, command]);
  assert_eq!(stored[0].model(), Some(&model()));
  let activity = db.activity(run.id(), 0).expect("activity");
  assert_eq!(
    activity.iter().map(|entry| entry.event.clone()).collect::<Vec<_>>(),
    events.map(|(_, event)| event)
  );
  // Active time counts the command's run like an agent's.
  assert_eq!(stored[1].active_millis(at(99)), 15);
}

#[test]
fn tools_a_step_failed_by_its_command_keeps_the_reason() {
  let (_repo, mut db, mut run, execution) = started_run();
  let mut events = run.start_step(execution, at(8)).expect("start step");
  events.extend(
    run
      .fail_step(execution, StepFailure::CommandFailed, at(9))
      .expect("fail"),
  );
  db.append_events(&run, &events).expect("append");
  let stored = db.run(run.id()).expect("read").expect("run");
  let failure = stored
    .execution(execution)
    .and_then(orchester_core::run::StepExecution::failure);
  assert_eq!(failure, Some(StepFailure::CommandFailed));
}
