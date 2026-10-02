//! Actual launch policy must remain independent of mutable next-invocation settings.
mod support;

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{AgentPermissions, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands},
  resource::{AgentProfile, ModelId, StepKey},
  run::{Invocation, PermissionsApply, Run},
  store::{ActivityStore, ResourceStore},
};
use orchester_desktop::tasks::permissions::{ChangeState, InvocationPolicy, RunPermissions, run_permissions};
use support::{Fixture, at};

fn policy(ask: bool) -> AgentPermissions {
  let mut policy = AgentPermissions::before_permissions();
  policy.set_commands(if ask {
    CommandPermission::AskBefore
  } else {
    CommandPermission::AnyInWorktree
  });
  policy.set_interactive_tool_approvals(ask);
  policy
}

/// A launch recorded by the runner for a Codex invocation.
fn launch(permissions: AgentPermissions) -> Option<LaunchPermissions> {
  Some(LaunchPermissions::new(permissions, OutsideCommands::SandboxApproval))
}

/// What the runner records for the next invocation of `view`'s desired settings.
fn enforced(view: &RunPermissions) -> Option<LaunchPermissions> {
  launch(
    view
      .permissions
      .enforced_on(AgentProvider::CodexCli, Platform::current())
      .expect("enforceable")
      .permissions()
      .clone(),
  )
}

fn started() -> (Fixture, Run, Invocation) {
  let mut fixture = Fixture::new();
  let task = fixture.task("Permission display", at(10));
  let run = fixture.started_run(task, at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("test").expect("model"), at(21))
    .expect("invocation");
  (fixture, run, invocation)
}

/// The permissions `invocation` launched with, as the row shows them.
fn launched(invocation: &Invocation) -> Option<AgentPermissions> {
  invocation
    .permissions
    .as_ref()
    .map(|launch| launch.permissions().clone())
}

fn view(fixture: &Fixture, run: &Run, invocations: &[Invocation]) -> RunPermissions {
  let step = StepKey::new("implement").expect("key");
  let profile = run.profile_of(&step).expect("profile");
  let version = fixture
    .db
    .latest_version::<AgentProfile>(profile)
    .expect("read")
    .expect("profile");
  run_permissions(
    &fixture.db,
    (run, invocations),
    &step,
    (profile, &version),
    Some(AgentProvider::CodexCli),
  )
  .expect("resolve")
  .expect("agent permissions")
}

fn change(fixture: &mut Fixture, run: &mut Run, desired: AgentPermissions, apply: PermissionsApply) {
  let profile = run
    .profile_of(&StepKey::new("implement").expect("key"))
    .expect("profile");
  let events = run
    .change_permissions(
      profile,
      desired,
      apply,
      (AgentProvider::CodexCli, Platform::current()),
      at(30),
    )
    .expect("change");
  fixture.record(run, &events);
}

#[test]
fn run_policy_next_invocation_enables_and_disables_approvals_without_relabeling_active_policy() {
  for enabled in [false, true] {
    let (mut fixture, mut run, mut invocation) = started();
    invocation.permissions = launch(policy(enabled));
    let original = Some(policy(enabled));
    change(
      &mut fixture,
      &mut run,
      policy(!enabled),
      PermissionsApply::NextInvocation,
    );
    let before = view(&fixture, &run, &[invocation.clone()]);
    assert_eq!(before.invocation_policy, InvocationPolicy::Active(original.clone()));
    assert_eq!(before.state, Some(ChangeState::NextInvocation));
    assert_eq!(before.base.interactive_tool_approvals(), !enabled);
    assert_eq!(
      *before.base.commands(),
      if enabled {
        CommandPermission::AnyInWorktree
      } else {
        CommandPermission::AskBefore
      }
    );
    let mut next = invocation.clone();
    next.number += 1;
    next.permissions = enforced(&before);
    let after = view(&fixture, &run, &[invocation, next.clone()]);
    assert_eq!(after.invocation_policy, InvocationPolicy::Active(launched(&next)));
    assert_eq!(after.state, None);
    assert_eq!(
      fixture
        .db
        .latest_version::<AgentProfile>(before.profile_id)
        .expect("profile")
        .expect("version")
        .body()
        .permissions(),
      &before.profile_permissions
    );
  }
}

#[test]
fn run_policy_apply_now_keeps_actual_policy_until_recorded_checkpoint_launch() {
  let (mut fixture, mut run, mut invocation) = started();
  invocation.permissions = launch(policy(false));
  change(&mut fixture, &mut run, policy(true), PermissionsApply::Now);
  let waiting = view(&fixture, &run, &[invocation.clone()]);
  assert_eq!(waiting.state, Some(ChangeState::Pending));
  assert_eq!(
    waiting.invocation_policy,
    InvocationPolicy::Active(launched(&invocation))
  );
  let events = run
    .apply_permissions(invocation.execution, None, at(40))
    .expect("checkpoint");
  fixture.record(&run, &events);
  invocation.ended_at = Some(at(40));
  let mut next = invocation.clone();
  next.started_at = at(41);
  next.ended_at = None;
  next.number = 2;
  next.permissions = enforced(&waiting);
  let applied = view(&fixture, &run, &[invocation, next.clone()]);
  assert!(matches!(
    applied.state,
    Some(ChangeState::Applied { invocation: 2, .. })
  ));
  assert_eq!(applied.invocation_policy, InvocationPolicy::Active(launched(&next)));
}

#[test]
fn run_policy_unknown_history_idle_and_completed_states_do_not_claim_desired_policy_ran() {
  let (mut fixture, mut run, mut invocation) = started();
  assert_eq!(
    view(&fixture, &run, &[invocation.clone()]).invocation_policy,
    InvocationPolicy::Active(None)
  );
  change(&mut fixture, &mut run, policy(true), PermissionsApply::NextInvocation);
  assert_eq!(
    view(&fixture, &run, &[invocation.clone()]).state,
    Some(ChangeState::NextInvocation)
  );
  invocation.ended_at = Some(at(31));
  let idle = view(&fixture, &run, &[invocation.clone()]);
  assert_eq!(idle.invocation_policy, InvocationPolicy::Last(None));
  assert_eq!(idle.state, Some(ChangeState::NextInvocation));
  fixture.succeed(&mut run, at(40));
  let completed = view(&fixture, &run, &[invocation]);
  assert_eq!(completed.invocation_policy, InvocationPolicy::Last(None));
  assert_eq!(completed.state, None);
  assert_eq!(
    view(&fixture, &run, &[]).invocation_policy,
    InvocationPolicy::NotStarted
  );
}

#[test]
fn run_policy_recorded_network_on_is_displayed_even_when_desired_network_off() {
  let (mut fixture, mut run, mut invocation) = started();
  let mut actual = policy(false);
  actual.set_network(NetworkAccess::On);
  invocation.permissions = launch(actual.clone());
  let mut desired = policy(true);
  desired.set_network(NetworkAccess::Off);
  change(&mut fixture, &mut run, desired, PermissionsApply::NextInvocation);
  let current = view(&fixture, &run, &[invocation]);
  assert_eq!(current.invocation_policy, InvocationPolicy::Active(Some(actual)));
  assert_eq!(current.base.network(), NetworkAccess::Off);
}

#[test]
fn run_policy_idle_settings_and_completed_recorded_policy_remain_distinct() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Not launched", at(10));
  let mut idle = fixture.pending_run(task, at(20));
  change(&mut fixture, &mut idle, policy(true), PermissionsApply::NextInvocation);
  let queued = view(&fixture, &idle, &[]);
  assert_eq!(queued.invocation_policy, InvocationPolicy::NotStarted);
  assert_eq!(queued.state, Some(ChangeState::NextInvocation));
  assert!(queued.permissions.interactive_tool_approvals());

  let (mut fixture, mut run, mut invocation) = started();
  invocation.permissions = launch(policy(false));
  invocation.ended_at = Some(at(31));
  change(&mut fixture, &mut run, policy(true), PermissionsApply::NextInvocation);
  fixture.succeed(&mut run, at(40));
  let ended = view(&fixture, &run, &[invocation]);
  assert_eq!(ended.invocation_policy, InvocationPolicy::Last(Some(policy(false))));
  assert_eq!(ended.state, None);
  assert!(ended.base.interactive_tool_approvals());
}
