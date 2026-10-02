//! An npm launcher whose command is a program file (Claude Code's `claude.cmd` since 2.1) is
//! replaced by that program, so arguments `cmd.exe` would change reach it as they were given. The
//! program here is `orchester-print-args`, copied to where the launcher looks for Claude Code.
#![cfg(windows)]

use std::{
  fs,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::command::{Command, CommandError, CommandRunner};
use orchester_exec::{
  ProcessRunner,
  launch::{self, Via},
};

const RUNNER: ProcessRunner = ProcessRunner::new(Duration::from_secs(30));
const PRINT_ARGS: &str = env!("CARGO_BIN_EXE_orchester-print-args");
const CLAUDE_SHIM: &str = include_str!("fixtures/shims/claude.cmd");
const CODEX_SHIM: &str = include_str!("fixtures/shims/codex.cmd");
/// Claude Code's `--json-schema` value: JSON on one line, full of quotes and braces.
const SCHEMA: &str = r#"{"type":"object","properties":{"outcome":{"enum":["done","failed"]}},"required":["outcome"]}"#;
/// The arguments of the failed run: `--setting-sources ""` (argument 2 here) and the schema.
const ARGS: [&str; 7] = [
  "-p",
  "--setting-sources",
  "",
  "--json-schema",
  SCHEMA,
  "50% & \"quoted\" ^!",
  "C:\\dir\\",
];

fn relative(path: &str) -> PathBuf {
  path.split('/').collect()
}

/// `claude.cmd` as npm installs it, with its program; returns the launcher and the program.
fn installed_claude(folder: &Path) -> (PathBuf, PathBuf) {
  let program = folder.join(relative("node_modules/@anthropic-ai/claude-code/bin/claude.exe"));
  fs::create_dir_all(program.parent().expect("a folder")).expect("program folder");
  fs::copy(PRINT_ARGS, &program).expect("program");
  let shim = folder.join("claude.cmd");
  fs::write(&shim, CLAUDE_SHIM.replace('\n', "\r\n")).expect("launcher");
  (shim, program)
}

fn printed(args: &[&str]) -> String {
  args.iter().map(|arg| format!("{arg:?}\n")).collect()
}

#[test]
fn launcher_program_starts_in_place_of_its_launcher() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let (shim, program) = installed_claude(folder.path());
  let launch = launch::resolve(&shim);
  assert_eq!(launch.program(), program);
  assert!(launch.leading_args().is_empty());
  assert_eq!(launch.via(), &Via::LauncherProgram);
  let output = RUNNER.run(&Command::new(&shim, ARGS)).expect("the program runs");
  assert!(output.success(), "{}", output.stderr);
  assert_eq!(output.stdout, printed(&ARGS));
}

#[test]
fn launcher_program_that_is_missing_leaves_the_launcher_to_cmd() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let shim = folder.path().join("claude.cmd");
  fs::write(&shim, CLAUDE_SHIM).expect("launcher");
  assert_eq!(launch::resolve(&shim).via(), &Via::CommandScript);
  assert_eq!(
    RUNNER.run(&Command::new(&shim, ARGS)),
    Err(CommandError::UnsafeScriptArgument { position: 2 })
  );
}

#[test]
fn launcher_program_of_another_script_shape_is_not_started() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let (shim, _) = installed_claude(folder.path());
  fs::write(&shim, CLAUDE_SHIM.replace(":start\n", ":start\necho starting\n")).expect("launcher");
  assert_eq!(launch::resolve(&shim).via(), &Via::CommandScript);
  assert_eq!(
    RUNNER.run(&Command::new(&shim, ["--json-schema", SCHEMA])),
    Err(CommandError::UnsafeScriptArgument { position: 1 })
  );
}

#[test]
fn launcher_program_beside_a_node_launcher_leaves_that_one_to_node() {
  let folder = tempfile::tempdir().expect("temporary directory");
  installed_claude(folder.path());
  let script = folder.path().join(relative("node_modules/@openai/codex/bin/codex.js"));
  fs::create_dir_all(script.parent().expect("a folder")).expect("script folder");
  fs::write(&script, "").expect("script");
  let codex = folder.path().join("codex.cmd");
  fs::write(&codex, CODEX_SHIM).expect("launcher");
  let launch = launch::resolve(&codex);
  assert_eq!(launch.program(), Path::new("node"));
  assert_eq!(launch.leading_args(), [script.to_string_lossy()]);
  assert_eq!(launch.via(), &Via::NodeScript { script });
}
