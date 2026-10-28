//! Auto mode's *approve everything in each run* (ORC-199): from a run's start it answers as Approve everything in
//! this run would, only while the run uses auto mode and the setting is on; what auto-approval leaves to a person
//! still asks; turning either off stops it; only a person turns it on, or automation a person lets change agent
//! permissions. `cargo test -p orchester-core --test approve_everything approve_everything`.

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  permission::category::{CommandCategory, CommandScope},
  run::{
    Answerer, ApprovalSubject, ApproveEverything, ApproveEverythingPersonOnly, AutoAsk, AutoVerdict,
    PendingToolApproval, ToolReach, auto_approves, auto_verdict,
  },
  time::Timestamp,
};
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
/// automatically, with `setting` and the run's `auto_mode`, as the runner decides.
fn answered(subject: &ApprovalSubject, chosen: Option<bool>, (setting, auto_mode): (ApproveEverything, bool)) -> bool {
  auto_approves(chosen, setting.covers(auto_mode)) && matches!(auto_verdict(subject), AutoVerdict::Approve(_))
}

#[test]
fn approve_everything_is_off_until_someone_turns_it_on() {
  let setting = ApproveEverything::default();
  assert_eq!(setting, ApproveEverything::OFF);
  assert!(!setting.is_on());
  assert_eq!(setting.turned_on_by(), None);
  assert!(!setting.covers(true));
}

#[test]
fn approve_everything_answers_from_the_runs_first_request_while_auto_mode_and_the_setting_are_on() {
  let fixture = Fixture::new();
  let on = ApproveEverything::on(Answerer::Person);
  let mcp = ApprovalSubject::McpTool {
    server: "pencil".to_owned(),
    tool: "batch_design".to_owned(),
    reach: ToolReach::Any,
  };
  // A run the person never chose for, whose first request this is.
  assert!(answered(&mcp, None, (on, true)));
  for (command, category) in [
    ("cargo test", CommandCategory::Worktree),
    ("git status", CommandCategory::ReadOnly),
    // A read of a dependency outside the worktree: auto-approval approves plain reads wherever they read.
    ("ls ../dependency/src", CommandCategory::ReadOnly),
  ] {
    let subject = fixture.command(command);
    assert_eq!(auto_verdict(&subject), AutoVerdict::Approve(Some(category)), "{command}");
    assert!(answered(&subject, None, (on, true)), "{command}");
  }
}

#[test]
fn approve_everything_leaves_what_auto_approval_leaves_to_a_person() {
  let fixture = Fixture::new();
  let on = ApproveEverything::on(Answerer::Person);
  for (command, ask) in [
    ("curl https://example.dev", AutoAsk::Network),
    ("git fetch origin", AutoAsk::Network),
    ("echo x > ../outside.txt", AutoAsk::OutsideWrite),
    ("cat ~/.ssh/id_rsa", AutoAsk::Secrets),
    ("npm install left-pad", AutoAsk::Installs),
    ("git reset --hard", AutoAsk::Destructive),
    ("./regen.sh", AutoAsk::Unknown),
  ] {
    let subject = fixture.command(command);
    assert_eq!(auto_verdict(&subject), AutoVerdict::Ask(ask), "{command}");
    assert!(!answered(&subject, None, (on, true)), "{command}");
  }
}

/// The request that waited for the person on 2026-10-28: a read of a dependency's checkout under the home folder.
#[test]
fn approve_everything_answers_a_read_of_a_dependency_under_home() {
  let fixture = Fixture::new();
  let subject = fixture.command(r#"ls "$HOME/.cargo/git/checkouts/orchester-1a2b/3c4d/crates/api/src""#);
  assert_eq!(auto_verdict(&subject), AutoVerdict::Approve(Some(CommandCategory::ReadOnly)));
  assert!(answered(&subject, None, (ApproveEverything::on(Answerer::Person), true)));
}

#[test]
fn approve_everything_stops_with_auto_mode_or_the_setting_off_and_the_runs_own_stop_wins() {
  let fixture = Fixture::new();
  let subject = fixture.command("cargo test");
  let on = ApproveEverything::on(Answerer::Automation);
  assert!(answered(&subject, None, (on, true)));
  assert!(!answered(&subject, None, (on, false)), "the run's auto mode turned off");
  assert!(!answered(&subject, None, (ApproveEverything::OFF, true)), "the setting turned off");
  assert!(!answered(&subject, Some(false), (on, true)), "the run's own Stop");
  assert!(answered(&subject, Some(true), (ApproveEverything::OFF, false)), "Approve everything in this run");
}

#[test]
fn approve_everything_is_turned_on_by_a_person_or_by_automation_a_person_lets_change_permissions() {
  let off = ApproveEverything::OFF;
  assert_eq!(
    off.changed(true, Answerer::Person, false),
    Ok(ApproveEverything::on(Answerer::Person))
  );
  assert_eq!(
    off.changed(true, Answerer::Automation, false),
    Err(ApproveEverythingPersonOnly)
  );
  assert_eq!(
    off.changed(true, Answerer::Automation, true),
    Ok(ApproveEverything::on(Answerer::Automation))
  );
  // Asking for what it is changes nothing, not even who turned it on.
  let person = ApproveEverything::on(Answerer::Person);
  assert_eq!(person.changed(true, Answerer::Automation, false), Ok(person));
  // Turning it off is anyone's.
  for (by, may) in [(Answerer::Person, false), (Answerer::Automation, false)] {
    assert_eq!(person.changed(false, by, may), Ok(ApproveEverything::OFF));
    assert_eq!(off.changed(false, by, may), Ok(ApproveEverything::OFF));
  }
}
