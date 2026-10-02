//! What the task's Permissions row says the agent runs with comes from the launch the runner
//! recorded, never from the run's mutable next-invocation settings.
mod support;

use orchester_core::{
  connection::{AgentProvider, invocation::Platform, stream::InvocationOutcome},
  permission::{AgentPermissions, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands},
  resource::{AgentProfile, ModelId, StepKey},
  run::{Invocation, PermissionsApply, Run},
  store::{ActivityStore, ResourceStore},
  time::Timestamp,
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

/// What the runner enforces for `view`'s settings for the next invocation.
fn expected(view: &RunPermissions) -> AgentPermissions {
  view
    .permissions
    .enforced_on(AgentProvider::CodexCli, Platform::current())
    .expect("enforceable")
    .permissions()
    .clone()
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

/// The invocation after `previous` of the same execution, started at `now` and not launched yet.
fn restart(previous: &Invocation, now: Timestamp) -> Invocation {
  let mut next = previous.clone();
  next.number += 1;
  next.started_at = now;
  next.ended_at = None;
  next.outcome = None;
  next.permissions = None;
  next
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

fn change(
  fixture: &mut Fixture,
  run: &mut Run,
  desired: AgentPermissions,
  (apply, now): (PermissionsApply, Timestamp),
) {
  let profile = run
    .profile_of(&StepKey::new("implement").expect("key"))
    .expect("profile");
  let events = run
    .change_permissions(
      profile,
      desired,
      apply,
      (AgentProvider::CodexCli, Platform::current()),
      now,
    )
    .expect("change");
  fixture.record(run, &events);
}

/// Apply now of `policy(true)` over a first invocation launched with `policy(false)`: the agent
/// stopped at its checkpoint at 40 and its first invocation ended.
fn applied_now() -> (Fixture, Run, Invocation) {
  let (mut fixture, mut run, mut invocation) = started();
  invocation.permissions = launch(policy(false));
  change(&mut fixture, &mut run, policy(true), (PermissionsApply::Now, at(30)));
  let events = run
    .apply_permissions(invocation.execution, None, at(40))
    .expect("checkpoint");
  fixture.record(&run, &events);
  invocation.ended_at = Some(at(40));
  (fixture, run, invocation)
}

#[test]
fn run_policy_next_invocation_enables_and_disables_approvals_without_relabeling_active_policy() {
  for enabled in [false, true] {
    let (mut fixture, mut run, mut invocation) = started();
    invocation.permissions = launch(policy(enabled));
    change(
      &mut fixture,
      &mut run,
      policy(!enabled),
      (PermissionsApply::NextInvocation, at(30)),
    );
    let before = view(&fixture, &run, &[invocation.clone()]);
    assert_eq!(before.invocation_policy, InvocationPolicy::Active(policy(enabled)));
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
    let mut next = restart(&invocation, at(31));
    next.permissions = launch(expected(&before));
    let after = view(&fixture, &run, &[invocation, next]);
    assert_eq!(after.invocation_policy, InvocationPolicy::Active(expected(&before)));
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
  change(&mut fixture, &mut run, policy(true), (PermissionsApply::Now, at(30)));
  let waiting = view(&fixture, &run, &[invocation.clone()]);
  assert_eq!(waiting.state, Some(ChangeState::Pending));
  assert_eq!(waiting.invocation_policy, InvocationPolicy::Active(policy(false)));
  let events = run
    .apply_permissions(invocation.execution, None, at(40))
    .expect("checkpoint");
  fixture.record(&run, &events);
  invocation.ended_at = Some(at(40));
  let mut next = restart(&invocation, at(41));
  next.permissions = launch(expected(&waiting));
  let applied = view(&fixture, &run, &[invocation, next]);
  assert!(matches!(
    applied.state,
    Some(ChangeState::Applied { invocation: 2, .. })
  ));
  assert_eq!(applied.invocation_policy, InvocationPolicy::Active(expected(&waiting)));
}

#[test]
fn run_policy_apply_now_restarting_until_the_new_launch_is_recorded() {
  let (fixture, run, invocation) = applied_now();
  // Stopped at the checkpoint, not started again yet: too late to keep it for the next invocation.
  let stopped = view(&fixture, &run, std::slice::from_ref(&invocation));
  assert_eq!(stopped.state, Some(ChangeState::Restarting));
  assert_eq!(stopped.invocation_policy, InvocationPolicy::Last(policy(false)));
  // The new invocation exists but has not recorded what it launched with.
  let starting = view(&fixture, &run, &[invocation.clone(), restart(&invocation, at(41))]);
  assert_eq!(starting.state, Some(ChangeState::Restarting));
  assert_eq!(starting.invocation_policy, InvocationPolicy::Starting);
}

#[test]
fn run_policy_apply_now_whose_restart_did_not_start_says_so_and_keeps_the_recorded_policy() {
  let (fixture, run, invocation) = applied_now();
  let mut failed = restart(&invocation, at(41));
  failed.ended_at = Some(at(41));
  failed.outcome = Some(InvocationOutcome::NotStarted);
  let view = view(&fixture, &run, &[invocation, failed.clone()]);
  assert_eq!(view.state, Some(ChangeState::RestartFailed));
  // The launch that never happened does not hide the one before it.
  assert_eq!(view.invocation_policy, InvocationPolicy::Last(policy(false)));
  // The Activity panel about the failed launch itself says it never started.
  assert_eq!(InvocationPolicy::of(&failed), InvocationPolicy::NotStarted);
}

#[test]
fn run_policy_a_later_next_invocation_change_replaces_the_applied_one() {
  let (mut fixture, mut run, invocation) = applied_now();
  let mut restarted = restart(&invocation, at(41));
  let launched = expected(&view(&fixture, &run, std::slice::from_ref(&invocation)));
  restarted.permissions = launch(launched.clone());
  let invocations = [invocation, restarted];
  assert!(matches!(
    view(&fixture, &run, &invocations).state,
    Some(ChangeState::Applied { .. })
  ));
  change(
    &mut fixture,
    &mut run,
    policy(false),
    (PermissionsApply::NextInvocation, at(50)),
  );
  let later = view(&fixture, &run, &invocations);
  assert_eq!(later.state, Some(ChangeState::NextInvocation));
  assert_eq!(later.invocation_policy, InvocationPolicy::Active(launched.clone()));
  // Saved back to what the restarted agent runs with: nothing waits, and the earlier Apply now is
  // no longer the change in force, so it is not announced as applied.
  change(
    &mut fixture,
    &mut run,
    policy(true),
    (PermissionsApply::NextInvocation, at(60)),
  );
  let same = view(&fixture, &run, &invocations);
  assert_eq!(same.state, None);
  assert_eq!(same.invocation_policy, InvocationPolicy::Active(launched));
}

#[test]
fn run_policy_starting_unknown_idle_and_completed_states_do_not_claim_desired_policy_ran() {
  let (mut fixture, mut run, mut invocation) = started();
  // Created, its launch not recorded yet: starting, not unknown.
  assert_eq!(
    view(&fixture, &run, &[invocation.clone()]).invocation_policy,
    InvocationPolicy::Starting
  );
  change(
    &mut fixture,
    &mut run,
    policy(true),
    (PermissionsApply::NextInvocation, at(30)),
  );
  assert_eq!(
    view(&fixture, &run, &[invocation.clone()]).state,
    Some(ChangeState::NextInvocation)
  );
  // Ended without a record: launched before launches were recorded.
  invocation.ended_at = Some(at(31));
  let idle = view(&fixture, &run, &[invocation.clone()]);
  assert_eq!(idle.invocation_policy, InvocationPolicy::Unknown);
  assert_eq!(idle.state, Some(ChangeState::NextInvocation));
  fixture.succeed(&mut run, at(40));
  let completed = view(&fixture, &run, &[invocation]);
  assert_eq!(completed.invocation_policy, InvocationPolicy::Unknown);
  assert_eq!(completed.state, None);
  assert_eq!(
    view(&fixture, &run, &[]).invocation_policy,
    InvocationPolicy::NoInvocation
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
  change(
    &mut fixture,
    &mut run,
    desired,
    (PermissionsApply::NextInvocation, at(30)),
  );
  let current = view(&fixture, &run, &[invocation]);
  assert_eq!(current.invocation_policy, InvocationPolicy::Active(actual));
  assert_eq!(current.base.network(), NetworkAccess::Off);
}

#[test]
fn run_policy_idle_settings_and_completed_recorded_policy_remain_distinct() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Not launched", at(10));
  let mut idle = fixture.pending_run(task, at(20));
  change(
    &mut fixture,
    &mut idle,
    policy(true),
    (PermissionsApply::NextInvocation, at(30)),
  );
  let queued = view(&fixture, &idle, &[]);
  assert_eq!(queued.invocation_policy, InvocationPolicy::NoInvocation);
  assert_eq!(queued.state, Some(ChangeState::NextInvocation));
  assert!(queued.permissions.interactive_tool_approvals());

  let (mut fixture, mut run, mut invocation) = started();
  invocation.permissions = launch(policy(false));
  invocation.ended_at = Some(at(31));
  change(
    &mut fixture,
    &mut run,
    policy(true),
    (PermissionsApply::NextInvocation, at(30)),
  );
  fixture.succeed(&mut run, at(40));
  let ended = view(&fixture, &run, &[invocation]);
  assert_eq!(ended.invocation_policy, InvocationPolicy::Last(policy(false)));
  assert_eq!(ended.state, None);
  assert!(ended.base.interactive_tool_approvals());
}
