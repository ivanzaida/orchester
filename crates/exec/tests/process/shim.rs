//! Windows command scripts: npm launchers are replaced by what they start, and any
//! other script only gets arguments that `cmd.exe` passes on unchanged. The launcher texts are
//! copies of the ones npm (`codex.cmd` and Claude Code 2.1's `claude.cmd`, via `cmd-shim`) and
//! corepack (`pnpm.cmd`) installed; the programs they start are not needed (`launcher_program.rs`
//! starts one).

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::{Command, CommandError, CommandRunner},
  connection::GitHubHost,
  pull_request::{NewPullRequest, gh_check_rollups_command, gh_pr_create_command},
  repository::GitHubRepo,
  revision::Revision,
  run::BranchName,
};
use orchester_exec::launch::{self, Via, native_shim_program, node_shim_script, passes_through_cmd};

use super::RUNNER;

const CODEX_SHIM: &str = include_str!("../fixtures/shims/codex.cmd");
const PNPM_SHIM: &str = include_str!("../fixtures/shims/pnpm.cmd");
const NPM_SHIM: &str = include_str!("../fixtures/shims/npm.cmd");
const CLAUDE_SHIM: &str = include_str!("../fixtures/shims/claude.cmd");
/// A script that is no npm launcher: it prints its arguments, then its standard input.
const ECHO_SCRIPT: &str = "@echo off\r\necho ARGS %*\r\nfindstr \"^\"\r\n";
const QUERY: &str = "query($owner:String!){repository(owner:$owner,name:\"x\"){id}}";

fn relative(path: &str) -> PathBuf {
  path.split('/').collect()
}

#[test]
fn process_shim_scripts_of_npm_launchers_are_recognised() {
  assert_eq!(
    node_shim_script(CODEX_SHIM),
    Some(relative("node_modules/@openai/codex/bin/codex.js"))
  );
  assert_eq!(
    node_shim_script(PNPM_SHIM),
    Some(relative("node_modules/corepack/dist/pnpm.js"))
  );
  assert_eq!(
    node_shim_script(&CODEX_SHIM.replace("\r\n", "\n")),
    node_shim_script(CODEX_SHIM)
  );
}

#[test]
fn process_shim_scripts_of_other_scripts_are_not_guessed() {
  // npm's own launcher picks its script at run time.
  assert_eq!(node_shim_script(NPM_SHIM), None);
  assert_eq!(node_shim_script(ECHO_SCRIPT), None);
  // `%_prog%` that is not Node.js, and two different scripts.
  let other_program = CODEX_SHIM.replace("SET \"_prog=%dp0%\\node.exe\"", "SET \"_prog=%dp0%\\bun.exe\"");
  assert_eq!(node_shim_script(&other_program), None);
  let two = PNPM_SHIM.replacen("pnpm.js", "yarn.js", 1);
  assert_eq!(node_shim_script(&two), None);
}

#[test]
fn process_shim_programs_of_npm_launchers_are_recognised() {
  assert_eq!(
    native_shim_program(CLAUDE_SHIM),
    Some(relative("node_modules/@anthropic-ai/claude-code/bin/claude.exe"))
  );
  assert_eq!(
    native_shim_program(&CLAUDE_SHIM.replace('\n', "\r\n")),
    native_shim_program(CLAUDE_SHIM)
  );
  assert_eq!(
    native_shim_program("@SETLOCAL\r\n\"%~dp0\\bin\\tool.EXE\" %*\r\n"),
    Some(relative("bin/tool.EXE"))
  );
  // A launcher of a Node.js script is not one of a program, and the other way round.
  assert_eq!(native_shim_program(CODEX_SHIM), None);
  assert_eq!(node_shim_script(CLAUDE_SHIM), None);
}

#[test]
fn process_shim_programs_of_other_scripts_are_not_guessed() {
  let launch_line = CLAUDE_SHIM.lines().last().expect("a launch line");
  for text in [
    // A command before or after the launch line.
    CLAUDE_SHIM.replace(":start\n", ":start\necho starting\n"),
    format!("{CLAUDE_SHIM}echo done\n"),
    // `%dp0%` that the script never sets.
    format!("@ECHO off\n{launch_line}\n"),
    // Not every argument passed on, no program file, a variable in the path, a path not quoted.
    CLAUDE_SHIM.replace("%*", "%1"),
    CLAUDE_SHIM.replace("claude.exe", "claude.js"),
    CLAUDE_SHIM.replace("bin\\claude", "%TOOL%\\claude"),
    CLAUDE_SHIM.replace("\"%dp0%", "%dp0%").replace(".exe\"", ".exe"),
    ECHO_SCRIPT.to_owned(),
    NPM_SHIM.to_owned(),
  ] {
    assert_eq!(native_shim_program(&text), None, "{text}");
  }
}

#[test]
fn process_shim_programs_stay_inside_the_launcher_folder() {
  const PROGRAM: &str = "node_modules\\@anthropic-ai\\claude-code\\bin\\claude.exe";
  assert_eq!(
    native_shim_program("@SETLOCAL\r\n\"%~dp0\\..\\tool\\bin\\tool.exe\" %*\r\n"),
    Some(relative("../tool/bin/tool.exe"))
  );
  // `Path::join` would leave the folder for a drive (relative to that drive's current folder),
  // a root or a share; `.` and empty parts are refused as well.
  for elsewhere in [
    "C:\\Windows\\System32\\cmd.exe",
    "C:cmd.exe",
    "\\Windows\\System32\\cmd.exe",
    "\\\\server\\share\\cmd.exe",
    "node_modules\\.\\claude.exe",
    "node_modules\\\\claude.exe",
  ] {
    let text = CLAUDE_SHIM.replace(PROGRAM, elsewhere);
    assert_eq!(native_shim_program(&text), None, "{elsewhere}");
  }
}

#[test]
fn process_shim_scripts_stay_inside_the_launcher_folder() {
  const SCRIPT: &str = "node_modules\\@openai\\codex\\bin\\codex.js";
  assert_eq!(
    node_shim_script("@SETLOCAL\r\nnode  \"%~dp0\\..\\lib\\cli.js\" %*\r\n"),
    Some(relative("../lib/cli.js"))
  );
  for elsewhere in [
    "C:\\Users\\x\\cli.js",
    "C:cli.js",
    "\\lib\\cli.js",
    "\\\\server\\share\\cli.js",
    "node_modules\\.\\cli.js",
    "node_modules\\\\cli.js",
  ] {
    let text = CODEX_SHIM.replace(SCRIPT, elsewhere);
    assert_eq!(node_shim_script(&text), None, "{elsewhere}");
  }
}

#[test]
fn process_shim_program_preamble_lines_are_compared_as_written() {
  // `cmd.exe` keeps the trailing space: `dp0` would not be the launcher's folder.
  let spaced = CLAUDE_SHIM.replace("SET dp0=%~dp0\n", "SET dp0=%~dp0 \n");
  assert_eq!(native_shim_program(&spaced), None);
  let indented = CLAUDE_SHIM.replace("\nSETLOCAL\n", "\n  SETLOCAL\n");
  assert_eq!(native_shim_program(&indented), None);
}

/// `codex.cmd` next to its script, with or without Node.js beside it.
fn installed_codex(folder: &Path, local_node: bool) -> PathBuf {
  let script = folder.join(relative("node_modules/@openai/codex/bin/codex.js"));
  fs::create_dir_all(script.parent().expect("a folder")).expect("script folder");
  fs::write(&script, "").expect("script");
  if local_node {
    fs::write(folder.join("node.exe"), "").expect("node");
  }
  let shim = folder.join("codex.cmd");
  fs::write(&shim, CODEX_SHIM).expect("launcher");
  shim
}

#[test]
fn process_shim_launch_runs_node_with_the_script() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let shim = installed_codex(folder.path(), true);
  let script = folder.path().join(relative("node_modules/@openai/codex/bin/codex.js"));
  let launch = launch::resolve(&shim);
  assert_eq!(launch.program(), folder.path().join("node.exe"));
  assert_eq!(launch.leading_args(), [script.to_string_lossy()]);
  assert_eq!(launch.via(), &Via::NodeScript { script });
  fs::remove_file(folder.path().join("node.exe")).expect("node removed");
  assert_eq!(launch::resolve(&shim).program(), Path::new("node"));
}

#[test]
fn process_shim_without_its_script_runs_through_cmd() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let shim = folder.path().join("codex.cmd");
  fs::write(&shim, CODEX_SHIM).expect("launcher");
  assert_eq!(launch::resolve(&shim).via(), &Via::CommandScript);
  let claude = folder.path().join("claude.cmd");
  fs::write(&claude, CLAUDE_SHIM).expect("launcher");
  assert_eq!(launch::resolve(&claude).via(), &Via::CommandScript);
  assert_eq!(launch::resolve(Path::new("C:/tools/gh.exe")).via(), &Via::Direct);
}

#[test]
fn process_shim_arguments_that_cmd_would_change_are_refused() {
  for safe in [
    "--version",
    "api",
    "owner=ivanzaida",
    "Accept: application/vnd.github.diff",
    "a...b",
  ] {
    assert!(passes_through_cmd(safe), "{safe}");
  }
  for unsafe_arg in [
    "",
    "a\"b",
    "50%",
    "a&b",
    "a|b",
    "a^b",
    "(x)",
    "wow!",
    "a\nb",
    "C:\\dir\\",
    QUERY,
  ] {
    assert!(!passes_through_cmd(unsafe_arg), "{unsafe_arg}");
  }
  let folder = tempfile::tempdir().expect("temporary directory");
  let script = folder.path().join("gh.cmd");
  fs::write(&script, ECHO_SCRIPT).expect("script");
  let raw_query = Command::new(&script, ["api", "graphql", "-f", &format!("query={QUERY}")]);
  assert_eq!(
    RUNNER.run(&raw_query),
    Err(CommandError::UnsafeScriptArgument { position: 3 })
  );
  let output = RUNNER
    .run(&Command::new(&script, ["--version"]))
    .expect("the script runs");
  assert_eq!(output.stdout.trim(), "ARGS --version");
}

#[test]
fn process_shim_gets_the_graphql_query_on_standard_input() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let gh = folder.path().join("gh.cmd");
  fs::write(&gh, ECHO_SCRIPT).expect("script");
  let repo = GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("a repository");
  let head = Revision::new(&"8f3a2c1d".repeat(5)).expect("a revision");
  let command = gh_check_rollups_command(&gh, &repo, &[head]);
  let output = RUNNER.run(&command).expect("the arguments pass through cmd.exe");
  assert!(output.success(), "{}", output.stderr);
  let mut lines = output.stdout.lines();
  assert_eq!(
    lines.next().map(str::trim_end),
    Some("ARGS api graphql --hostname github.com --input -")
  );
  let body = lines.collect::<String>();
  assert!(body.contains("repository(owner:$owner,name:$name)"), "{body}");
  assert!(body.contains("\"owner\":\"ivanzaida\""), "{body}");
}

/// A pull request's title (`KEY · Title`), body and branches would be refused as arguments of a `gh.cmd`: they
/// are the request body on standard input.
#[test]
fn process_shim_gets_a_new_pull_request_on_standard_input() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let gh = folder.path().join("gh.cmd");
  fs::write(&gh, ECHO_SCRIPT).expect("script");
  let repo = GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("a repository");
  let branch = |name: &str| BranchName::new(name).expect("a branch");
  let new = NewPullRequest {
    base: branch("main"),
    head: branch("orchester/kon-12-run-1"),
    title: "KON-12 · Add retry".to_owned(),
    body: "Retry the webhook sender.\n\n## Acceptance criteria\n\n1. 100% of \"failed\" deliveries retry".to_owned(),
    draft: true,
  };
  let command = gh_pr_create_command(&gh, &repo, &new);
  let output = RUNNER.run(&command).expect("the arguments pass through cmd.exe");
  assert!(output.success(), "{}", output.stderr);
  let mut lines = output.stdout.lines();
  assert_eq!(
    lines.next().map(str::trim_end),
    Some("ARGS api --method POST --hostname github.com repos/ivanzaida/kontur/pulls --input -")
  );
  let body = lines.collect::<String>();
  assert!(body.contains("\"head\":\"orchester/kon-12-run-1\""), "{body}");
  assert!(body.contains("\"draft\":true"), "{body}");
}

/// Claude Code's `--json-schema` value: JSON on one line, full of quotes and braces.
const SCHEMA: &str = r#"{"type":"object","properties":{"outcome":{"enum":["done","failed"]}},"required":["outcome"]}"#;

#[test]
fn process_shim_json_arguments_reach_a_node_launcher_and_never_cmd() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let args = ["--json-schema".to_owned(), SCHEMA.to_owned()];
  // npm's launcher starts Node.js directly, which gets the argument as the OS passes it.
  let shim = installed_codex(folder.path(), true);
  assert_eq!(launch::resolve(&shim).check_args(&args), Ok(()));
  // Any other script would hand it to cmd.exe, which changes it: refused before it starts.
  let script = folder.path().join("claude.cmd");
  fs::write(&script, ECHO_SCRIPT).expect("script");
  assert_eq!(
    RUNNER.run(&Command::new(&script, args.iter().map(String::as_str))),
    Err(CommandError::UnsafeScriptArgument { position: 1 })
  );
}
