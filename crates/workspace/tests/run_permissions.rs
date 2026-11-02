//! A run's permissions chip and its menu (ORC-319, `docs/design/approval-modes.md`, 15f `CimC4`): what the chip says
//! and what each of the menu's ways does in every mode; each change is made for the run alone and recorded for its
//! Conversation; widening — *Approve everything in this run* — is a person's alone: automation's asking for it is
//! refused, changes nothing, and the refusal is recorded too.
//!
//! `cargo test -p orchester-workspace --test run_permissions`

mod support;

use orchester_core::{
  permission::{
    AgentPermissions, ApprovalMode, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands,
    RepositoryAccess,
  },
  resource::ModelId,
  run::{
    AllowAutopilot, Answerer, EffectiveMode, ManualBecause, PermissionEventKind, RuleScope, Run, RunPermissionChange,
  },
  store::ActivityStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  run_approvals::{PermissionChangeError, RunApprovals, change_permissions_in},
  run_permissions::{ChipMode, NotAutopilot, Offer, PermissionsMenu},
};
use support::{Fixture, at};

/// A started run whose agent launched with a profile in `mode`, in a project that allows Autopilot as `allows` says.
fn run_in(fixture: &mut Fixture, mode: ApprovalMode, allows: bool) -> Run {
  let task = fixture.task("Add retry to webhook sender", at(10));
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
      &ModelId::new("claude-opus-5-5").expect("model"),
      at(30),
    )
    .expect("an agent launch");
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  );
  permissions.set_approval_mode(mode);
  fixture
    .db
    .save_invocation_permissions(
      invocation.id,
      &LaunchPermissions::new(permissions, OutsideCommands::PermissionPrompt),
    )
    .expect("launched");
  let allow = if allows {
    AllowAutopilot::ON
  } else {
    AllowAutopilot::OFF
  };
  fixture.db.save_allow_autopilot(allow).expect("the project's setting");
  run
}

fn approvals(fixture: &Fixture, run: &Run) -> RunApprovals {
  RunApprovals::read(&ProjectDb::open(fixture.repo.path()).expect("open"), run).expect("the run's approvals")
}

fn change(
  fixture: &Fixture,
  run: &Run,
  change: RunPermissionChange,
  by: Answerer,
) -> Result<(), PermissionChangeError> {
  change_permissions_in(fixture.repo.path(), run.id(), change, by)
}

#[test]
fn run_permissions_automation_cannot_widen_a_run_and_the_refusal_is_recorded() {
  let mut fixture = Fixture::new();
  let run = run_in(&mut fixture, ApprovalMode::Autopilot, true);
  let refused = change(
    &fixture,
    &run,
    RunPermissionChange::ApproveEverything,
    Answerer::Automation,
  );
  assert!(
    matches!(refused, Err(PermissionChangeError::PersonOnly(_))),
    "{refused:?}"
  );
  let kept = approvals(&fixture, &run);
  assert_eq!(kept.chosen, None, "nothing changed");
  assert!(!kept.approves_everything());
  assert_eq!(kept.events.len(), 1);
  assert_eq!(kept.events[0].by, Answerer::Automation);
  assert_eq!(kept.events[0].kind, PermissionEventKind::Refused);
  // The menu's foot then says that only a person widens a run; a person's widening is taken.
  change(&fixture, &run, RunPermissionChange::ApproveEverything, Answerer::Person).expect("a person widens");
  assert!(approvals(&fixture, &run).approves_everything());
}

#[test]
fn run_permissions_a_person_approves_everything_and_anyone_stops_it_back_to_the_mode() {
  let mut fixture = Fixture::new();
  let run = run_in(&mut fixture, ApprovalMode::Autopilot, true);
  change(&fixture, &run, RunPermissionChange::ApproveEverything, Answerer::Person).expect("widened");
  let widened = approvals(&fixture, &run);
  assert_eq!(widened.chosen, Some(true));
  assert_eq!(
    widened
      .events
      .iter()
      .map(|event| (event.by, &event.kind))
      .collect::<Vec<_>>(),
    [(
      Answerer::Person,
      &PermissionEventKind::Everything {
        was: EffectiveMode::Autopilot
      }
    )]
  );
  change(&fixture, &run, RunPermissionChange::ApproveEverything, Answerer::Person).expect("again");
  assert_eq!(
    approvals(&fixture, &run).events.len(),
    1,
    "a change that changes nothing records nothing"
  );
  change(
    &fixture,
    &run,
    RunPermissionChange::StopApprovingEverything,
    Answerer::Automation,
  )
  .expect("narrowed");
  let stopped = approvals(&fixture, &run);
  assert_eq!(stopped.chosen, None, "back to Autopilot, which answers again");
  assert!(stopped.approved_by_autopilot());
  assert_eq!(stopped.events.len(), 2);
  assert_eq!(stopped.events[1].by, Answerer::Automation);
  assert_eq!(
    stopped.events[1].kind,
    PermissionEventKind::Stopped {
      back_to: EffectiveMode::Autopilot
    }
  );
}

#[test]
fn run_permissions_manual_for_this_run_narrows_for_good_and_stops_approving_everything() {
  let mut fixture = Fixture::new();
  let run = run_in(&mut fixture, ApprovalMode::Autopilot, true);
  change(&fixture, &run, RunPermissionChange::ApproveEverything, Answerer::Person).expect("widened");
  change(&fixture, &run, RunPermissionChange::ManualForRun, Answerer::Automation).expect("narrowed");
  let narrowed = approvals(&fixture, &run);
  assert_eq!(narrowed.mode, EffectiveMode::Manual(ManualBecause::Run));
  assert_eq!(narrowed.chosen, Some(false), "nothing approves automatically in Manual");
  assert_eq!(narrowed.switched.map(|switch| switch.by), Some(Answerer::Automation));
  assert_eq!(
    narrowed.events.last().map(|event| &event.kind),
    Some(&PermissionEventKind::Stopped {
      back_to: EffectiveMode::Manual(ManualBecause::Run)
    })
  );
  let menu = PermissionsMenu::of(&narrowed);
  assert_eq!(menu.chip, ChipMode::ManualRun);
  assert_eq!(menu.autopilot, Offer::Unavailable(NotAutopilot::Switched));
}

#[test]
fn run_permissions_revoking_a_rule_records_what_it_answered_once() {
  let mut fixture = Fixture::new();
  let run = run_in(&mut fixture, ApprovalMode::Manual, false);
  let scope = RuleScope::SimilarMcpTool {
    server: "pencil".to_owned(),
    tool: "batch_design".to_owned(),
    folder: None,
    check: None,
  };
  let rule = fixture.db.add_approval_rule(run.id(), &scope, at(40)).expect("a rule");
  change(&fixture, &run, RunPermissionChange::Revoke(rule.id), Answerer::Person).expect("revoked");
  change(&fixture, &run, RunPermissionChange::Revoke(rule.id), Answerer::Person).expect("again");
  let kept = approvals(&fixture, &run);
  assert!(kept.rules.iter().all(|rule| !rule.is_active()));
  assert_eq!(
    kept.events.iter().map(|event| &event.kind).collect::<Vec<_>>(),
    [&PermissionEventKind::Revoked { scope }]
  );
}

/// The menu of a run in `mode`, approving everything or not.
fn menu(mode: EffectiveMode, everything: bool) -> PermissionsMenu {
  PermissionsMenu::of(&RunApprovals {
    mode,
    chosen: everything.then_some(true),
    ..RunApprovals::default()
  })
}

#[test]
fn run_permissions_the_chip_and_the_menu_follow_the_mode() {
  let autopilot = menu(EffectiveMode::Autopilot, false);
  assert_eq!(autopilot.chip, ChipMode::Autopilot);
  assert_eq!(autopilot.profile, ApprovalMode::Autopilot);
  assert_eq!(autopilot.manual, Offer::Change(RunPermissionChange::ManualForRun));
  assert_eq!(autopilot.autopilot, Offer::Current);
  assert_eq!(
    autopilot.everything,
    Offer::Change(RunPermissionChange::ApproveEverything)
  );
  assert_eq!(autopilot.nothing_wider, None);

  let manual = menu(EffectiveMode::Manual(ManualBecause::Profile), false);
  assert_eq!(manual.chip, ChipMode::Manual);
  assert_eq!(manual.profile, ApprovalMode::Manual);
  assert_eq!(manual.manual, Offer::Current);
  assert_eq!(manual.autopilot, Offer::Unavailable(NotAutopilot::ProfileManual));

  let project = menu(EffectiveMode::Manual(ManualBecause::Project), false);
  assert_eq!(project.chip, ChipMode::ManualProject);
  assert_eq!(project.profile, ApprovalMode::Autopilot);
  assert_eq!(project.autopilot, Offer::Unavailable(NotAutopilot::ProjectOff));

  // Approving everything in Autopilot: Autopilot and Manual for this run both narrow.
  let wide = menu(EffectiveMode::Autopilot, true);
  assert_eq!(wide.chip, ChipMode::Everything);
  assert_eq!(wide.everything, Offer::Current);
  assert_eq!(
    wide.autopilot,
    Offer::Change(RunPermissionChange::StopApprovingEverything)
  );
  assert_eq!(wide.manual, Offer::Change(RunPermissionChange::ManualForRun));
  assert_eq!(wide.nothing_wider, None);

  // Approving everything in Manual: nothing is wider, Autopilot is left out, and Manual stops it.
  let widest = menu(EffectiveMode::Manual(ManualBecause::Profile), true);
  assert_eq!(widest.chip, ChipMode::Everything);
  assert_eq!(widest.autopilot, Offer::Hidden);
  assert_eq!(
    widest.manual,
    Offer::Change(RunPermissionChange::StopApprovingEverything)
  );
  assert_eq!(widest.nothing_wider, Some(NotAutopilot::ProfileManual));
}

#[test]
fn run_permissions_only_approving_everything_widens() {
  for change in [
    RunPermissionChange::ManualForRun,
    RunPermissionChange::StopApprovingEverything,
    RunPermissionChange::Revoke(orchester_core::run::ApprovalRuleId::new(1)),
  ] {
    assert!(!change.widens(), "{change:?}");
    assert_eq!(change.asked_by(Answerer::Automation), Ok(change));
  }
  assert!(RunPermissionChange::ApproveEverything.widens());
  assert!(
    RunPermissionChange::ApproveEverything
      .asked_by(Answerer::Automation)
      .is_err()
  );
  assert_eq!(
    RunPermissionChange::ApproveEverything.asked_by(Answerer::Person),
    Ok(RunPermissionChange::ApproveEverything)
  );
}
