//! A profile's approval mode (`docs/design/approval-modes.md`): Manual by default and for every profile saved before
//! it existed, kept with the profile's version, Manual stricter than Autopilot, and Manual → Autopilot a widening.

use std::collections::BTreeMap;

use orchester_core::{
  id::ConnectionId,
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, ApprovalMode, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    StepCommands, StepPermissions, widening::Widening,
  },
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, ResourceBody},
};

fn builder() -> AgentProfile {
  AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    "Implements the approved plan.".into(),
    PromptText::new("You implement {{task.title}}.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("valid model"),
    ProviderSettings::new(BTreeMap::new()).expect("no settings"),
  )
}

fn allowed(commands: &[&str]) -> CommandPermission {
  let prefixes = commands
    .iter()
    .map(|command| CommandPrefix::new(command).expect("a command prefix"))
    .collect();
  CommandPermission::Allowed(AllowedCommands::new(prefixes).expect("an allowed list"))
}

fn permissions(commands: CommandPermission, mode: ApprovalMode) -> AgentPermissions {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off);
  permissions.set_approval_mode(mode);
  permissions
}

/// The three command permissions profiles had before approval modes existed.
fn old_commands() -> [CommandPermission; 3] {
  [
    CommandPermission::AskBefore,
    allowed(&["cargo test", "git status"]),
    CommandPermission::AnyInWorktree,
  ]
}

#[test]
fn approval_mode_profile_defaults_to_manual() {
  assert_eq!(ApprovalMode::default(), ApprovalMode::Manual);
  assert_eq!(builder().permissions().approval_mode(), ApprovalMode::Manual);
  assert_eq!(
    AgentPermissions::new(RepositoryAccess::ReadOnly, allowed(&[]), NetworkAccess::Off).approval_mode(),
    ApprovalMode::Manual
  );
  assert_eq!(
    AgentPermissions::before_permissions().approval_mode(),
    ApprovalMode::Manual
  );
}

#[test]
fn approval_mode_profile_is_versioned_with_the_permissions() {
  let list = allowed(&["cargo test"]);
  let autopilot = builder().with_permissions(permissions(list.clone(), ApprovalMode::Autopilot));
  let json = serde_json::to_value(&autopilot).expect("serializes");
  assert_eq!(json["permissions"]["approval_mode"], "autopilot");
  assert_eq!(serde_json::from_value::<AgentProfile>(json).expect("reads"), autopilot);
  let manual = builder().with_permissions(permissions(list.clone(), ApprovalMode::Manual));
  let json = serde_json::to_value(&manual).expect("serializes");
  assert!(
    json["permissions"].get("approval_mode").is_none(),
    "Manual keeps the shape before approval modes: {json}"
  );
  assert_eq!(serde_json::from_value::<AgentProfile>(json).expect("reads"), manual);
  assert_eq!(
    manual.permissions().commands(),
    &list,
    "Manual's allowed commands are the profile's list"
  );
  assert_eq!(AgentProfile::SCHEMA, 5);
}

#[test]
fn approval_mode_profile_manual_is_stricter_than_autopilot() {
  assert!(ApprovalMode::Manual.is_stricter_than(ApprovalMode::Autopilot));
  assert!(!ApprovalMode::Autopilot.is_stricter_than(ApprovalMode::Manual));
  assert!(!ApprovalMode::Manual.is_stricter_than(ApprovalMode::Manual));
  assert!(!ApprovalMode::Autopilot.is_stricter_than(ApprovalMode::Autopilot));
  let profile = permissions(allowed(&["cargo test"]), ApprovalMode::Autopilot);
  let step = StepPermissions {
    commands: StepCommands::Only(AllowedCommands::default()),
    ..StepPermissions::default()
  };
  assert_eq!(
    profile.narrowed(RepositoryAccess::ReadOnly, &step).approval_mode(),
    ApprovalMode::Autopilot,
    "a step narrows what runs, not who answers"
  );
}

#[test]
fn approval_mode_profile_autopilot_widens_and_manual_never_does() {
  for commands in old_commands() {
    let manual = permissions(commands.clone(), ApprovalMode::Manual);
    let autopilot = permissions(commands, ApprovalMode::Autopilot);
    assert_eq!(autopilot.widenings_from(&manual), [Widening::Autopilot]);
    assert!(
      manual.widenings_from(&autopilot).is_empty(),
      "switching to Manual never asks"
    );
    assert!(autopilot.widenings_from(&autopilot).is_empty());
  }
}

#[test]
fn approval_mode_profile_migrates_every_old_shape_to_manual_keeping_its_commands() {
  for commands in old_commands() {
    let current = builder().with_permissions(permissions(commands.clone(), ApprovalMode::Manual));
    let stored = serde_json::to_value(&current).expect("serializes");
    for schema in [3, 4] {
      let upgraded = AgentProfile::upgrade(schema, stored.clone()).expect("upgrades");
      let read: AgentProfile = serde_json::from_value(upgraded).expect("reads");
      assert_eq!(
        read.permissions().approval_mode(),
        ApprovalMode::Manual,
        "schema {schema}"
      );
      assert_eq!(
        read.permissions().commands(),
        &commands,
        "schema {schema} keeps its commands"
      );
    }
    let mut schema_2 = stored.clone();
    schema_2.as_object_mut().expect("a body").remove("mcp_servers");
    let read: AgentProfile =
      serde_json::from_value(AgentProfile::upgrade(2, schema_2).expect("upgrades")).expect("reads");
    assert_eq!(read, current, "schema 2");
  }
  let mut schema_1 = serde_json::to_value(builder()).expect("serializes");
  let fields = schema_1.as_object_mut().expect("a body");
  fields.remove("permissions");
  fields.remove("mcp_servers");
  let read: AgentProfile =
    serde_json::from_value(AgentProfile::upgrade(1, schema_1).expect("upgrades")).expect("reads");
  assert_eq!(read.permissions().approval_mode(), ApprovalMode::Manual, "schema 1");
  assert_eq!(read.permissions().commands(), &CommandPermission::AnyInWorktree);
}
