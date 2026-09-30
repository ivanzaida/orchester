//! Permissions changed for one run (02.J): they stand in for a profile's permissions in this run,
//! steps still narrow them, the connection's CLI must enforce them, and Apply now restarts the
//! profile's agents at work until each has restarted or the change is kept for the next
//! invocation.

use std::collections::BTreeMap;

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  id::ResourceId,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess, StepCommands,
    StepNetwork, StepPermissions,
    capability::{PermissionOption, PermissionProblem, Unavailable},
  },
  resource::{Pipeline, StepKind},
  run::{Checkpoint, PermissionsApply, PublishedVersions, Run, RunError, RunNumber, RunSnapshot},
};

use crate::fixture::{Logged, agent_step, at, key, new_run, profile, revision, version};

const CODEX_MAC: (AgentProvider, Platform) = (AgentProvider::CodexCli, Platform::MacOs);
const CODEX_WINDOWS: (AgentProvider, Platform) = (AgentProvider::CodexCli, Platform::Windows);
const CLAUDE: (AgentProvider, Platform) = (AgentProvider::ClaudeCodeCli, Platform::Windows);

fn commands(values: &[&str]) -> AllowedCommands {
  AllowedCommands::new(
    values
      .iter()
      .map(|value| CommandPrefix::new(value).expect("command"))
      .collect(),
  )
  .expect("list")
}

/// Edit files, any command in the worktree, network on: what Codex enforces everywhere.
fn wider() -> AgentPermissions {
  AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  )
}

fn builder(logged: &Logged) -> ResourceId {
  logged
    .run
    .profile_of(&key("research"))
    .expect("research runs on the builder")
}

#[test]
fn run_permissions_replace_the_profile_s_for_its_steps_only() {
  let mut logged = Logged::started();
  let builder = builder(&logged);
  let profile_permissions = AgentPermissions::before_permissions();
  logged.apply(|run| run.change_permissions(builder, wider(), PermissionsApply::NextInvocation, CODEX_MAC, at(12)));
  let run = &logged.run;
  assert_eq!(
    run.step_permissions(&key("implement"), &profile_permissions),
    Some(wider()),
    "a step of the profile uses the run's permissions"
  );
  assert_eq!(
    run.step_permissions(&key("review"), &profile_permissions),
    Some(profile_permissions.clone()),
    "a step of another profile keeps its profile's"
  );
  let changed = run.changed_permissions(builder).expect("changed");
  assert_eq!(changed.permissions(), &wider());
  assert_eq!(changed.changed_at(), at(12));
  assert!(
    !changed.is_pending(),
    "the next invocation takes them; nothing restarts"
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

/// A run whose `implement` step narrows the profile to read-only, `git` and network off.
fn narrowing_run() -> (Logged, ResourceId) {
  let builder = ResourceId::generate();
  let mut implement = agent_step("implement", builder, &[]);
  implement.kind = StepKind::Agent {
    profile: Some(builder),
    repository: RepositoryAccess::ReadOnly,
    model: None,
    permissions: StepPermissions {
      commands: StepCommands::Only(commands(&["git"])),
      network: StepNetwork::Off,
      ..StepPermissions::default()
    },
  };
  let pipeline = version(1, Pipeline::new(vec![implement]).expect("pipeline"));
  let profiles = BTreeMap::from([(builder, version(1, profile("Builder", "model-b")))]);
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  let snapshot = RunSnapshot::build(
    ResourceId::generate(),
    &pipeline,
    PublishedVersions {
      profiles: &profiles,
      tools: &tools,
      instructions: &instructions,
      skills: &BTreeMap::new(),
    },
  )
  .expect("published");
  let (run, created) = Run::create(
    orchester_core::id::RunId::generate(),
    RunNumber::new(std::num::NonZeroU32::MIN),
    new_run(snapshot),
    at(10),
  )
  .expect("run");
  let mut logged = Logged {
    run,
    log: vec![created],
  };
  logged.apply(|run| run.start(at(11)));
  (logged, builder)
}

#[test]
fn run_permissions_are_narrowed_by_each_step() {
  let (mut logged, builder) = narrowing_run();
  let widened = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(commands(&["git", "pnpm build"])),
    NetworkAccess::On,
  );
  logged.apply(|run| run.change_permissions(builder, widened, PermissionsApply::NextInvocation, CLAUDE, at(12)));
  let effective = logged
    .run
    .step_permissions(&key("implement"), &AgentPermissions::before_permissions())
    .expect("agent step");
  assert_eq!(
    effective,
    AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::Allowed(commands(&["git"])),
      NetworkAccess::Off,
    ),
    "the step still narrows what the run widened"
  );
}

#[test]
fn run_permissions_the_connection_cannot_enforce_are_refused() {
  let mut logged = Logged::started();
  let builder = builder(&logged);
  let listed = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(commands(&["pnpm test"])),
    NetworkAccess::On,
  );
  assert_eq!(
    logged
      .run
      .change_permissions(builder, listed, PermissionsApply::NextInvocation, CODEX_MAC, at(12)),
    Err(RunError::PermissionsNotEnforceable {
      step: key("research"),
      problem: PermissionProblem::NotEnforceable {
        option: PermissionOption::AllowedCommands,
        reason: Unavailable::CannotLimitCommands,
      },
    })
  );
  assert!(matches!(
    logged
      .run
      .change_permissions(builder, wider(), PermissionsApply::NextInvocation, CLAUDE, at(12)),
    Err(RunError::PermissionsNotEnforceable { .. })
  ));
  let ask = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::On,
  );
  assert!(matches!(
    logged
      .run
      .change_permissions(builder, ask, PermissionsApply::NextInvocation, CLAUDE, at(12)),
    Err(RunError::PermissionsNotEnforceable { .. })
  ));
  assert!(logged.run.changed_permissions(builder).is_none(), "nothing changed");
  // Network off that the CLI cannot enforce stays on, as for a profile: the change is kept.
  let network_off = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  logged.apply(|run| {
    run.change_permissions(
      builder,
      network_off,
      PermissionsApply::NextInvocation,
      CODEX_WINDOWS,
      at(12),
    )
  });
}

#[test]
fn run_permissions_name_a_profile_of_the_run_and_a_live_run() {
  let mut logged = Logged::started();
  let stranger = ResourceId::generate();
  assert_eq!(
    logged
      .run
      .change_permissions(stranger, wider(), PermissionsApply::NextInvocation, CODEX_MAC, at(12)),
    Err(RunError::ProfileNotInRun(stranger))
  );
  let builder = builder(&logged);
  logged.apply(|run| run.cancel(at(13)));
  assert!(matches!(
    logged
      .run
      .change_permissions(builder, wider(), PermissionsApply::NextInvocation, CODEX_MAC, at(14)),
    Err(RunError::InvalidRunTransition { .. })
  ));
}

#[test]
fn run_permissions_apply_now_restarts_the_agent_at_its_checkpoint() {
  let mut logged = Logged::started();
  let builder = builder(&logged);
  assert_eq!(
    logged
      .run
      .change_permissions(builder, wider(), PermissionsApply::Now, CODEX_MAC, at(12)),
    Err(RunError::NoAgentToRestart(builder)),
    "no agent of the profile is at work yet"
  );
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.change_permissions(builder, wider(), PermissionsApply::Now, CODEX_MAC, at(13)));
  assert!(logged.run.restarts_for_permissions(research));
  assert!(logged.run.changed_permissions(builder).expect("changed").is_pending());
  logged.apply(|run| run.record_checkpoint(research, revision('c'), at(14)));
  let checkpoint = logged.run.last_checkpoint().map(Checkpoint::id);
  logged.apply(|run| run.apply_permissions(research, checkpoint, at(15)));
  let changed = logged.run.changed_permissions(builder).expect("changed");
  assert!(!changed.is_pending());
  let applied = changed.applied().expect("applied");
  assert_eq!(
    (applied.execution, applied.checkpoint, applied.at),
    (research, checkpoint, at(15))
  );
  assert_eq!(
    logged.run.apply_permissions(research, None, at(16)),
    Err(RunError::NotRestartingForPermissions(research)),
    "an agent restarts once per change"
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn run_permissions_kept_for_next_invocation_cancel_the_restart() {
  let mut logged = Logged::started();
  let builder = builder(&logged);
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.change_permissions(builder, wider(), PermissionsApply::Now, CODEX_MAC, at(13)));
  logged.apply(|run| run.keep_permissions_for_next_invocation(builder, at(14)));
  let changed = logged.run.changed_permissions(builder).expect("changed");
  assert!(!changed.is_pending());
  assert_eq!(changed.permissions(), &wider(), "the change itself stays");
  assert_eq!(
    logged.run.keep_permissions_for_next_invocation(builder, at(15)),
    Err(RunError::NoPendingPermissions(builder))
  );
  assert_eq!(
    logged.run.apply_permissions(research, None, at(15)),
    Err(RunError::NotRestartingForPermissions(research))
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn run_permissions_apply_now_restarts_every_agent_of_the_profile_beside_others() {
  let mut logged = Logged::started();
  logged.through_plan();
  logged.pass("implement");
  let (checks, review) = (logged.exec("checks"), logged.exec("review"));
  logged.apply(|run| run.start_step(checks, at(30)));
  logged.apply(|run| run.start_step(review, at(30)));
  let builder = builder(&logged);
  let reviewer = logged.run.profile_of(&key("review")).expect("reviewer");
  logged.apply(|run| run.change_permissions(builder, wider(), PermissionsApply::Now, CODEX_MAC, at(31)));
  assert!(logged.run.restarts_for_permissions(checks));
  assert!(
    !logged.run.restarts_for_permissions(review),
    "another profile's agent keeps working"
  );
  logged.apply(|run| run.change_permissions(reviewer, wider(), PermissionsApply::Now, CODEX_MAC, at(32)));
  assert!(logged.run.restarts_for_permissions(review));
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn run_permissions_restart_is_dropped_once_the_agent_stops_otherwise() {
  let mut logged = Logged::started();
  let builder = builder(&logged);
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.change_permissions(builder, wider(), PermissionsApply::Now, CODEX_MAC, at(13)));
  logged.apply(|run| run.complete_step(research, at(14)));
  assert!(
    !logged.run.changed_permissions(builder).expect("changed").is_pending(),
    "a finished step has nothing to restart"
  );
  let plan = logged.exec("plan");
  logged.apply(|run| run.start_step(plan, at(15)));
  logged.apply(|run| run.change_permissions(builder, wider(), PermissionsApply::Now, CODEX_MAC, at(16)));
  logged.apply(|run| run.request_pause(at(17)));
  assert_eq!(
    logged
      .run
      .change_permissions(builder, wider(), PermissionsApply::Now, CODEX_MAC, at(18)),
    Err(RunError::NoAgentToRestart(builder)),
    "a run waiting to pause restarts nothing"
  );
  logged.apply(|run| run.pause(at(18)));
  assert!(!logged.run.changed_permissions(builder).expect("changed").is_pending());
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}
