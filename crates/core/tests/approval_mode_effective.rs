//! Approval modes (`docs/design/approval-modes.md`): a run's step is in Autopilot only when its profile says Autopilot,
//! the project allows it and the run was not switched to Manual, else in Manual with the first reason; only a person
//! allows Autopilot; Autopilot answers what auto-approval approves, and leaves the rest to the person.
//! `cargo test -p orchester-core --test approval_mode_effective approval_mode_effective`.

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  permission::{
    ApprovalMode,
    category::{CommandCategory, CommandScope},
  },
  run::{
    AllowAutopilot, Answerer, ApprovalSubject, AutoAsk, AutoVerdict, AutopilotPersonOnly, EffectiveMode,
    ManualBecause, PendingToolApproval, ToolReach, auto_approves, auto_verdict,
  },
  time::Timestamp,
};
use serde_json::json;
use tempfile::TempDir;

struct Fixture {
  _root: TempDir,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("kontur");
    let temp = root.path().join("temp");
    for folder in [&worktree, &temp, &home.join("dependency").join("src")] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&temp));
    Self { _root: root, scope }
  }

  fn command(&self, command: &str) -> ApprovalSubject {
    let approval = PendingToolApproval {
      id: "invocation:1".to_owned(),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation: InvocationId::generate(),
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: "command".to_owned(),
      action: command.to_owned(),
      target: None,
      requested_at: Timestamp::from_unix_millis(0),
      native: None,
    };
    ApprovalSubject::of(&approval, &self.scope)
  }
}

/// Whether Orchester answers `subject` itself in a run whose own choice is `chosen`, whose profiles do not approve
/// automatically, in `mode`, as the runner decides.
fn answered(subject: &ApprovalSubject, chosen: Option<bool>, mode: EffectiveMode) -> bool {
  auto_approves(chosen, mode.is_autopilot()) && matches!(auto_verdict(subject), AutoVerdict::Approve(_))
}

#[test]
fn approval_mode_effective_is_autopilot_only_when_profile_project_and_run_all_let_it() {
  use ApprovalMode::{Autopilot, Manual};
  let (allowed, disallowed) = (AllowAutopilot::ON, AllowAutopilot::OFF);
  let cases = [
    (Autopilot, allowed, false, EffectiveMode::Autopilot),
    (Autopilot, allowed, true, EffectiveMode::Manual(ManualBecause::Run)),
    (Autopilot, disallowed, false, EffectiveMode::Manual(ManualBecause::Project)),
    (Autopilot, disallowed, true, EffectiveMode::Manual(ManualBecause::Project)),
    (Manual, allowed, false, EffectiveMode::Manual(ManualBecause::Profile)),
    (Manual, allowed, true, EffectiveMode::Manual(ManualBecause::Profile)),
    (Manual, disallowed, false, EffectiveMode::Manual(ManualBecause::Profile)),
    (Manual, disallowed, true, EffectiveMode::Manual(ManualBecause::Profile)),
  ];
  for (profile, project, switched, expected) in cases {
    let mode = EffectiveMode::of(profile, project, switched);
    assert_eq!(mode, expected, "{profile:?} · {project:?} · switched {switched}");
    assert_eq!(mode.is_autopilot(), expected == EffectiveMode::Autopilot);
  }
}

#[test]
fn approval_mode_effective_defaults_are_manual_by_the_profile_with_autopilot_disallowed() {
  assert_eq!(ApprovalMode::default(), ApprovalMode::Manual);
  assert_eq!(AllowAutopilot::default(), AllowAutopilot::OFF);
  assert!(!AllowAutopilot::default().is_on());
  assert_eq!(EffectiveMode::default(), EffectiveMode::Manual(ManualBecause::Profile));
}

#[test]
fn approval_mode_effective_names_the_mode_and_its_reason_for_the_chip() {
  assert_eq!(
    serde_json::to_value(EffectiveMode::Autopilot).expect("json"),
    json!({"mode": "autopilot"})
  );
  for (because, word) in [
    (ManualBecause::Profile, "profile"),
    (ManualBecause::Project, "project"),
    (ManualBecause::Run, "run"),
  ] {
    assert_eq!(
      serde_json::to_value(EffectiveMode::Manual(because)).expect("json"),
      json!({"mode": "manual", "because": word})
    );
  }
}

#[test]
fn approval_mode_effective_widening_gate_lets_only_a_person_allow_autopilot() {
  let off = AllowAutopilot::OFF;
  assert_eq!(off.changed(true, Answerer::Person), Ok(AllowAutopilot::ON));
  assert_eq!(off.changed(true, Answerer::Automation), Err(AutopilotPersonOnly));
  // Asking for what it is changes nothing.
  let on = AllowAutopilot::ON;
  assert_eq!(on.changed(true, Answerer::Automation), Ok(on));
  // Turning it off is anyone's.
  for by in [Answerer::Person, Answerer::Automation] {
    assert_eq!(on.changed(false, by), Ok(AllowAutopilot::OFF));
    assert_eq!(off.changed(false, by), Ok(AllowAutopilot::OFF));
  }
}

#[test]
fn approval_mode_effective_autopilot_answers_what_auto_approval_approves_from_the_first_request() {
  let fixture = Fixture::new();
  let mcp = ApprovalSubject::McpTool {
    server: "pencil".to_owned(),
    tool: "batch_design".to_owned(),
    reach: ToolReach::Any,
  };
  assert!(answered(&mcp, None, EffectiveMode::Autopilot));
  for (command, category) in [
    ("cargo test", CommandCategory::Worktree),
    ("git status", CommandCategory::ReadOnly),
    // A read of a dependency outside the worktree: auto-approval approves plain reads wherever they read.
    ("ls ../dependency/src", CommandCategory::ReadOnly),
  ] {
    let subject = fixture.command(command);
    assert_eq!(
      auto_verdict(&subject),
      AutoVerdict::Approve(Some(category)),
      "{command}"
    );
    assert!(answered(&subject, None, EffectiveMode::Autopilot), "{command}");
  }
}

#[test]
fn approval_mode_effective_autopilot_leaves_what_auto_approval_leaves_to_a_person() {
  let fixture = Fixture::new();
  for (command, ask) in [
    ("curl https://example.dev", AutoAsk::Network),
    ("git fetch origin", AutoAsk::Network),
    ("echo x > ../outside.txt", AutoAsk::OutsideWrite),
    ("cat ~/.ssh/id_rsa", AutoAsk::Secrets),
    ("npm install left-pad", AutoAsk::Installs),
    ("git reset --hard origin/main", AutoAsk::Destructive),
    ("./regen.sh", AutoAsk::Unknown),
  ] {
    let subject = fixture.command(command);
    assert_eq!(auto_verdict(&subject), AutoVerdict::Ask(ask), "{command}");
    assert!(!answered(&subject, None, EffectiveMode::Autopilot), "{command}");
  }
}

#[test]
fn approval_mode_effective_manual_answers_nothing_itself_and_the_runs_own_choice_wins() {
  let fixture = Fixture::new();
  let subject = fixture.command("cargo test");
  for because in [ManualBecause::Profile, ManualBecause::Project, ManualBecause::Run] {
    assert!(!answered(&subject, None, EffectiveMode::Manual(because)), "{because:?}");
  }
  assert!(!answered(&subject, Some(false), EffectiveMode::Autopilot), "the run's own Stop");
  assert!(
    answered(&subject, Some(true), EffectiveMode::Manual(ManualBecause::Profile)),
    "Approve everything in this run"
  );
}
