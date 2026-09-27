//! Command tools (M10a): every part of the command is a validated value, the body round-trips and
//! is validated again when read, bodies of schema 1 are upgraded, and the process starts the
//! program found on the `PATH` or in the worktree with the cleaned environment and the tool's
//! variables. Run with `cargo test -p orchester-core --test command_tool command_tool`.

use std::{
  collections::{BTreeMap, BTreeSet},
  ffi::OsString,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::invocation::Platform,
  resource::{
    CommandAction, CommandArg, CommandEnvironment, CommandError, CommandProgram, CommandTimeout, ExitCodes,
    OutputLimit, ProgramLocation, ResourceBody, Tool, ToolKind, VariableName,
  },
};

fn program(value: &str) -> CommandProgram {
  CommandProgram::new(value).expect("valid program")
}

fn args(values: &[&str]) -> Vec<CommandArg> {
  values
    .iter()
    .map(|value| CommandArg::new(value).expect("valid argument"))
    .collect()
}

fn variable(name: &str) -> VariableName {
  VariableName::new(name).expect("valid name")
}

fn full_command() -> CommandAction {
  CommandAction::new(program("cargo"), args(&["test", "--workspace", "name with space"]))
    .with_timeout(CommandTimeout::from_secs(90).expect("valid timeout"))
    .with_exit_codes(ExitCodes::new(BTreeSet::from([0, 3])).expect("valid codes"))
    .with_environment(
      CommandEnvironment::new(BTreeMap::from([(variable("RUST_LOG"), "info".to_owned())])).expect("valid variables"),
    )
    .with_output_limit(OutputLimit::from_kib(64).expect("valid limit"))
}

#[test]
fn command_tool_program_is_a_name_a_worktree_path_or_an_absolute_path() {
  assert_eq!(CommandProgram::new("  "), Err(CommandError::BlankProgram));
  assert!(matches!(
    CommandProgram::new("cargo\ntest"),
    Err(CommandError::ControlCharacter(_))
  ));
  assert_eq!(program(" cargo ").as_str(), "cargo");
  assert_eq!(program("cargo").location(), ProgramLocation::OnPath("cargo"));
  assert_eq!(
    program("scripts/check.sh").location(),
    ProgramLocation::InWorktree(Path::new("scripts/check.sh"))
  );
  for escaping in [
    "../outside/check.sh",
    "scripts/../../check.sh",
    "scripts\\..\\..\\x.cmd",
  ] {
    assert!(
      matches!(CommandProgram::new(escaping), Err(CommandError::OutsideWorktree(_))),
      "{escaping}"
    );
  }
  let absolute = std::env::temp_dir().join("check");
  let absolute = absolute.to_str().expect("a Unicode temporary folder");
  assert_eq!(
    program(absolute).location(),
    ProgramLocation::Absolute(Path::new(absolute))
  );
}

#[test]
fn command_tool_arguments_limits_codes_and_variables_are_validated() {
  assert_eq!(CommandArg::new(""), Err(CommandError::EmptyArgument));
  assert!(matches!(
    CommandArg::new("a\tb"),
    Err(CommandError::ControlCharacter(_))
  ));
  assert!(matches!(
    CommandArg::new(&"x".repeat(5000)),
    Err(CommandError::TooLong(_))
  ));
  assert_eq!(
    CommandArg::new("with \"quotes\" and spaces").map(|arg| arg.as_str().len()),
    Ok(24)
  );
  assert_eq!(CommandTimeout::from_secs(0), Err(CommandError::Timeout));
  assert_eq!(CommandTimeout::from_secs(24 * 3600 + 1), Err(CommandError::Timeout));
  assert_eq!(
    CommandTimeout::from_secs(24 * 3600).map(CommandTimeout::secs),
    Ok(86_400)
  );
  assert_eq!(ExitCodes::new(BTreeSet::new()), Err(CommandError::NoExitCodes));
  assert_eq!(ExitCodes::new((0..40).collect()), Err(CommandError::TooManyExitCodes));
  assert_eq!(OutputLimit::from_kib(0), Err(CommandError::OutputLimit));
  assert_eq!(OutputLimit::from_kib(16 * 1024 + 1), Err(CommandError::OutputLimit));
  for invalid in ["", "1ST", "RUST-LOG", "A B"] {
    assert!(VariableName::new(invalid).is_err(), "{invalid}");
  }
  assert_eq!(
    CommandEnvironment::new(BTreeMap::from([
      (variable("Path_Extra"), "a".into()),
      (variable("PATH_EXTRA"), "b".into()),
    ])),
    Err(CommandError::DuplicateVariable("Path_Extra".into()))
  );
  assert_eq!(
    CommandEnvironment::new(BTreeMap::from([(variable("A"), "a\0b".into())])),
    Err(CommandError::VariableValue("A".into()))
  );
}

#[test]
fn command_tool_body_round_trips_and_is_validated_again_when_read() {
  let tool = Tool::command("Run the project's tests.", full_command());
  assert_eq!(tool.kind(), ToolKind::Command);
  assert!(tool.access().is_none());
  let json = serde_json::to_string(&tool).expect("serialize");
  assert_eq!(serde_json::from_str::<Tool>(&json).expect("read back"), tool);
  let json: serde_json::Value = serde_json::from_str(&json).expect("json");
  assert_eq!(json["action"]["command"]["timeout_secs"], 90);
  assert_eq!(
    json["action"]["command"]["expected_exit_codes"],
    serde_json::json!([0, 3])
  );
  for (field, value) in [
    ("timeout_secs", serde_json::json!(0)),
    ("expected_exit_codes", serde_json::json!([])),
    ("output_limit_kib", serde_json::json!(0)),
    ("program", serde_json::json!("../escape")),
  ] {
    let mut broken = json.clone();
    broken["action"]["command"][field] = value;
    assert!(serde_json::from_value::<Tool>(broken).is_err(), "{field}");
  }
}

#[test]
fn command_tool_bodies_of_schema_1_get_the_command_defaults() {
  let stored = serde_json::json!({
    "action": {"command": {"program": "cargo", "args": ["test"]}},
    "access": {
      "callers": {},
      "environment": "Staging",
      "approval": "never",
      "window": "until_step_ends",
      "denied": "fail_step"
    }
  });
  let upgraded = <Tool as ResourceBody>::upgrade(1, stored).expect("schema 1 upgrades");
  let tool: Tool = serde_json::from_value(upgraded).expect("reads as schema 2");
  let orchester_core::resource::ToolAction::Command(command) = tool.action() else {
    panic!("a command tool");
  };
  assert_eq!(command, &CommandAction::new(program("cargo"), args(&["test"])));
  assert_eq!(command.timeout(), CommandTimeout::DEFAULT);
  assert!(command.exit_codes().contains(0));
  assert_eq!(command.output_limit(), OutputLimit::DEFAULT);
  assert!(tool.access().is_some(), "the policy it had is kept");
  assert!(<Tool as ResourceBody>::upgrade(3, serde_json::json!({})).is_none());
}

#[test]
fn command_tool_command_line_quotes_arguments_with_spaces() {
  assert_eq!(
    full_command().command_line(),
    "cargo test --workspace \"name with space\""
  );
}

#[test]
fn command_tool_candidates_follow_the_path_or_the_worktree() {
  let root = std::env::temp_dir();
  let (one, two, worktree) = (root.join("one"), root.join("two"), root.join("worktree"));
  let path = std::env::join_paths([&one, &two]).expect("join");
  let cargo = CommandAction::new(program("cargo"), Vec::new());
  assert_eq!(
    cargo.program_candidates(&worktree, Some(&path), Platform::MacOs),
    [one.join("cargo"), two.join("cargo")]
  );
  let on_windows = cargo.program_candidates(&worktree, Some(&path), Platform::Windows);
  assert_eq!(
    on_windows[..4],
    ["cargo.exe", "cargo.cmd", "cargo.bat", "cargo.com"].map(|name| one.join(name))
  );
  assert_eq!(on_windows.len(), 8);
  let script = CommandAction::new(program("scripts/check.cmd"), Vec::new());
  assert_eq!(
    script.program_candidates(&worktree, Some(&path), Platform::Windows),
    [worktree.join("scripts/check.cmd")]
  );
  assert!(cargo.program_candidates(&worktree, None, Platform::MacOs).is_empty());
}

#[test]
fn command_tool_process_runs_in_the_worktree_with_the_cleaned_environment_and_its_variables() {
  let worktree = std::env::temp_dir().join("worktree");
  let found = PathBuf::from("/usr/bin/cargo");
  let environment = [
    ("PATH", "/usr/bin"),
    ("GH_TOKEN", "must not pass"),
    ("rust_log", "debug"),
  ]
  .map(|(name, value)| (OsString::from(name), OsString::from(value)));
  let process = full_command().process(&found, &worktree, environment);
  assert_eq!(process.program(), found);
  assert_eq!(process.args(), ["test", "--workspace", "name with space"]);
  assert_eq!(process.current_dir(), Some(worktree.as_path()));
  assert!(!process.inherits_env());
  assert_eq!(process.stdin(), None);
  assert_eq!(
    process.env(),
    [
      ("PATH".to_owned(), "/usr/bin".to_owned()),
      ("RUST_LOG".to_owned(), "info".to_owned())
    ]
  );
}
