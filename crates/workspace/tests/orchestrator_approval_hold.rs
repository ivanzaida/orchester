//! The hard rule of auto mode that holds a tool request, as the Orchestrator chat reads it for its decision (the
//! person's decision of 2026-11-11: information, not a refusal): a push, a change that destroys work or reaches outside
//! the worktree, a read of a secret; read from the request, its run and the step's recorded permissions, the stricter
//! where a fact cannot be read.
//! `cargo test -p orchester-workspace --test orchestrator_approval_hold approval_hold`.

mod support;

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  name::Name,
  permission::{
    AgentPermissions, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands, RepositoryAccess,
  },
  resource::{AgentProfile, ModelId as Model, StepKind},
  run::{PendingToolApproval, Run, review::ReviewHold},
  store::{ActivityStore, ConnectionStore, ResourceStore},
};
use orchester_workspace::tasks::approvals::hard_hold;
use support::{Fixture, at};

/// A started run whose agent launched with write access, commands asked before, the network on, and recorded so.
fn launched(fixture: &mut Fixture) -> (Run, orchester_core::id::InvocationId) {
  let task = fixture.task("Retry failed deliveries", at(10));
  let run = fixture.started_run(task, at(20));
  let execution = run
    .current_attempt()
    .and_then(|attempt| attempt.executions().last())
    .expect("a step execution")
    .id();
  let invocation = fixture
    .db
    .start_invocation(
      run.id(),
      execution,
      &Model::new("claude-opus-5-5").expect("model"),
      at(30),
    )
    .expect("an agent launch")
    .id;
  let permissions = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  fixture
    .db
    .save_invocation_permissions(
      invocation,
      &LaunchPermissions::new(permissions, OutsideCommands::PermissionPrompt),
    )
    .expect("launched");
  (run, invocation)
}

/// `run`'s agent asks to run `command`, call `call`.
fn asks(
  fixture: &mut Fixture,
  (run, invocation): &(Run, orchester_core::id::InvocationId),
  command: &str,
  call: u32,
) -> PendingToolApproval {
  let execution = run
    .current_attempt()
    .and_then(|attempt| attempt.executions().last())
    .expect("a step execution")
    .id();
  let request = PendingToolApproval {
    id: format!("{invocation}:call_{call}"),
    run: run.id(),
    execution,
    invocation: *invocation,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: command.to_owned(),
    target: None,
    requested_at: at(40),
    native: None,
  };
  fixture.db.request_tool_approval(&request).expect("the request waits");
  request
}

/// The connection the Builder profile of `run`'s step names, kept as a Claude Code one: its agent's CLI is known.
fn on_claude_code(fixture: &mut Fixture, run: &Run) {
  let step = &run.snapshot().body().steps()[0];
  let StepKind::Agent {
    profile: Some(builder), ..
  } = &step.kind
  else {
    panic!("the fixture's Implement step has a Builder profile");
  };
  let profile = fixture
    .db
    .latest_version::<AgentProfile>(*builder)
    .expect("read")
    .expect("the Builder profile");
  let connection = AgentConnection::new(
    profile.body().connection(),
    Name::new("Claude").expect("name"),
    AgentProvider::ClaudeCodeCli,
    fixture.repo.path().join("claude.exe"),
    at(1),
  )
  .expect("absolute program");
  fixture.db.create_connection(&connection).expect("connection");
}

#[test]
fn approval_hold_holds_a_push_a_destructive_command_and_a_secret_but_not_a_test_run() {
  let mut fixture = Fixture::new();
  let ran = launched(&mut fixture);
  on_claude_code(&mut fixture, &ran.0);
  let root = fixture.repo.path().to_path_buf();
  let outside = root.parent().expect("the repository's folder").join("elsewhere");
  let hostile = [
    ("git push origin master", Some(ReviewHold::Push)),
    ("git push --force origin HEAD", Some(ReviewHold::Push)),
    ("cat ~/.ssh/id_ed25519", Some(ReviewHold::Secrets)),
  ];
  for (call, (command, expected)) in (1..).zip(hostile) {
    let request = asks(&mut fixture, &ran, command, call);
    assert_eq!(hard_hold(&fixture.db, &root, &request), expected, "{command}");
  }
  let wipe = format!("rm -rf {}", outside.display().to_string().replace('\\', "/"));
  let request = asks(&mut fixture, &ran, &wipe, 10);
  assert!(
    hard_hold(&fixture.db, &root, &request).is_some(),
    "a delete outside the worktree holds: {wipe}"
  );
  let tests = asks(&mut fixture, &ran, "cargo test -p webhooks retry", 11);
  assert_eq!(
    hard_hold(&fixture.db, &root, &tests),
    None,
    "a test run in the worktree is the chat's to judge"
  );
}

#[test]
fn approval_hold_holds_every_command_whose_permissions_were_not_recorded() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Retry failed deliveries", at(10));
  let run = fixture.started_run(task, at(20));
  let execution = run
    .current_attempt()
    .and_then(|attempt| attempt.executions().last())
    .expect("a step execution")
    .id();
  let invocation = fixture
    .db
    .start_invocation(
      run.id(),
      execution,
      &Model::new("claude-opus-5-5").expect("model"),
      at(30),
    )
    .expect("an agent launch")
    .id;
  on_claude_code(&mut fixture, &run);
  let request = asks(&mut fixture, &(run, invocation), "cargo test", 1);
  assert_eq!(
    hard_hold(&fixture.db, fixture.repo.path(), &request),
    Some(ReviewHold::PermissionsUnknown)
  );
}
