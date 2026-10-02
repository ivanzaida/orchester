//! Native commands: splitting a command line on each platform, the git a command runs with, and which
//! commands run, ask a person or are refused.

use orchester_core::{
  native::{
    command::{
      CommandForm, CommandPolicy, CommandPolicyError, CommandRefusal, CommandSyntax, CommandVerdict, ParsedCommand,
      SyntaxError,
    },
    tools::{RunCommand, ToolInput, parse_call},
  },
  permission::{CommandPermission, RepositoryAccess},
};
use serde_json::json;

use crate::support::{Layout, allowed, text};

fn split(syntax: CommandSyntax, command: &str) -> Result<Vec<String>, SyntaxError> {
  syntax.split(command)
}

fn words(list: &[&str]) -> Vec<String> {
  list.iter().map(|word| (*word).to_owned()).collect()
}

fn run_command(input: &serde_json::Value) -> RunCommand {
  match parse_call("run_command", input).expect("run_command") {
    ToolInput::RunCommand(command) => command,
    other => panic!("{other:?}"),
  }
}

fn verdict(layout: &Layout, permission: &CommandPermission, command: &str) -> CommandVerdict {
  let policy = CommandPolicy::new(permission).expect("policy");
  policy.classify(
    &run_command(&json!({"command": command})),
    &layout.scope(RepositoryAccess::Write),
  )
}

fn runs(layout: &Layout, permission: &CommandPermission, command: &str) -> bool {
  match verdict(layout, permission, command) {
    CommandVerdict::Run(_) => true,
    CommandVerdict::Ask(_) => false,
    CommandVerdict::Refuse(refusal) => panic!("{command}: refused, {refusal}"),
  }
}

#[test]
fn native_command_posix_split() {
  let posix = |command| split(CommandSyntax::Posix, command);
  assert_eq!(posix("git log  --oneline"), Ok(words(&["git", "log", "--oneline"])));
  assert_eq!(
    posix(r#"echo 'a b' "c d" e\ f"#),
    Ok(words(&["echo", "a b", "c d", "e f"]))
  );
  assert_eq!(
    posix(r#"echo "a\"b\$c\d" '' x"#),
    Ok(words(&["echo", r#"a"b$c\d"#, "", "x"]))
  );
  assert_eq!(posix(r#"rg "a|b" '$(x)'"#), Ok(words(&["rg", "a|b", "$(x)"])));
  for (command, operator) in [
    ("ls | wc", "|"),
    ("a; b", ";"),
    ("a && b", "&"),
    ("echo x > out", ">"),
    ("cat < in", "<"),
    ("echo $(id)", "$("),
    (r#"echo "${HOME}""#, "${"),
    ("echo `id`", "`"),
    ("(cd x)", "("),
    ("ls\nrm x", "newline"),
  ] {
    assert_eq!(
      posix(command),
      Err(SyntaxError::Operator(operator.to_owned())),
      "{command}"
    );
  }
  assert_eq!(posix("echo 'open"), Err(SyntaxError::UnclosedQuote));
  assert_eq!(posix(r#"echo "open"#), Err(SyntaxError::UnclosedQuote));
  assert_eq!(posix("   "), Err(SyntaxError::Blank));
  assert_eq!(posix("'' x"), Err(SyntaxError::Blank));
}

#[test]
fn native_command_windows_split() {
  let windows = |command| split(CommandSyntax::Windows, command);
  assert_eq!(windows("git log --oneline"), Ok(words(&["git", "log", "--oneline"])));
  assert_eq!(
    windows(r#""C:\Program Files\Git\bin\git.exe" status"#),
    Ok(words(&[r"C:\Program Files\Git\bin\git.exe", "status"]))
  );
  assert_eq!(windows(r#"x a\"b"#), Ok(words(&["x", r#"a"b"#])));
  assert_eq!(windows(r#"x a\\"b c""#), Ok(words(&["x", r"a\b c"])));
  assert_eq!(windows(r#"x "a""b" """#), Ok(words(&["x", r#"a"b"#, ""])));
  assert_eq!(windows(r"x a\\b c\"), Ok(words(&["x", r"a\\b", r"c\"])));
  assert_eq!(windows(r#"rg "a|b" 'c'"#), Ok(words(&["rg", "a|b", "'c'"])));
  assert_eq!(windows("ls | more"), Err(SyntaxError::Operator("|".to_owned())));
  assert_eq!(windows("a&b"), Err(SyntaxError::Operator("&".to_owned())));
  assert_eq!(windows("dir > out"), Err(SyntaxError::Operator(">".to_owned())));
  assert_eq!(windows(r#"x "open"#), Err(SyntaxError::UnclosedQuote));
  assert_eq!(windows(""), Err(SyntaxError::Blank));
}

#[test]
fn native_command_hardened_git() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::ReadOnly);
  let parsed = |command: &str| {
    ParsedCommand::parse(&run_command(&json!({"command": command})), CommandSyntax::Posix, &scope).expect("parsed")
  };
  assert_eq!(
    parsed("git status --short").program_args(),
    Some(words(&[
      "git",
      "-c",
      "core.fsmonitor=false",
      "--no-pager",
      "--no-optional-locks",
      "status",
      "--short"
    ]))
  );
  assert_eq!(
    parsed("git diff HEAD").program_args(),
    Some(words(&[
      "git",
      "-c",
      "core.fsmonitor=false",
      "--no-pager",
      "--no-optional-locks",
      "diff",
      "--no-ext-diff",
      "--no-textconv",
      "HEAD"
    ]))
  );
  assert_eq!(
    parsed("git blame src/main.rs").program_args(),
    Some(words(&[
      "git",
      "-c",
      "core.fsmonitor=false",
      "--no-pager",
      "--no-optional-locks",
      "blame",
      "--no-textconv",
      "src/main.rs"
    ])),
    "blame runs a textconv program by default"
  );
  assert_eq!(parsed("cargo test").program_args(), Some(words(&["cargo", "test"])));
  let shell = ParsedCommand::parse(
    &run_command(&json!({"command": "ls | wc", "shell": true})),
    CommandSyntax::Posix,
    &scope,
  )
  .expect("shell");
  assert_eq!(shell.form(), &CommandForm::Shell);
  assert_eq!(shell.program_args(), None);
  assert_eq!(shell.text(), "ls | wc");
}

/// Windows and macOS find `Git` as `git`, so it gets the same hardening.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn native_command_hardened_git_in_any_case() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::ReadOnly);
  let command = ParsedCommand::parse(
    &run_command(&json!({"command": "Git status"})),
    CommandSyntax::Posix,
    &scope,
  )
  .expect("parsed");
  assert_eq!(
    command.program_args(),
    Some(words(&[
      "Git",
      "-c",
      "core.fsmonitor=false",
      "--no-pager",
      "--no-optional-locks",
      "status"
    ]))
  );
}

#[test]
fn native_command_allowed_list() {
  let layout = Layout::new();
  let list = allowed(&["cargo test", "git commit"]);
  assert!(runs(&layout, &list, "cargo test"));
  assert!(runs(&layout, &list, "cargo test -p orchester-core"));
  assert!(runs(&layout, &list, r#"git commit -m "a message with spaces""#));
  assert!(!runs(&layout, &list, "cargo tester"));
  assert!(
    !runs(&layout, &list, r#""cargo test" x"#),
    "an argument with spaces matches no word"
  );
  assert!(!runs(&layout, &list, "npm install"));
}

#[test]
fn native_command_known_safe_needs_a_list_entry() {
  let layout = Layout::new();
  assert!(
    !runs(&layout, &allowed(&[]), "git status"),
    "an empty list asks about every command"
  );
  assert!(!runs(&layout, &CommandPermission::AskBefore, "git status"));
  assert!(!runs(&layout, &CommandPermission::AskBefore, "cargo test"));
  let list = allowed(&["cargo test"]);
  assert!(runs(&layout, &list, "git status"));
  assert!(runs(&layout, &list, "cat src/main.rs"));
  assert!(runs(
    &layout,
    &list,
    &format!(r#"cat "{}""#, text(&layout.index.join("map.md")))
  ));
  assert!(!runs(&layout, &list, "git push"));
  assert!(!runs(&layout, &list, "cargo metadata"));
}

#[test]
fn native_command_known_safe_paths_stay_readable() {
  let layout = Layout::new();
  let list = allowed(&["cargo test"]);
  let secret = text(&layout.outside.join("secret.txt"));
  assert!(!runs(&layout, &list, &format!(r#"cat "{secret}""#)));
  assert!(!runs(&layout, &list, &format!(r#"rg "--ignore-file={secret}" x"#)));
  assert!(!runs(&layout, &list, "cat ../../../../outside/secret.txt"));
  assert!(!runs(&layout, &list, "ls ../../../.."));
  assert!(runs(&layout, &list, "cat missing.txt"), "a missing file reads nothing");
}

#[test]
fn native_command_shell_and_refusals() {
  let layout = Layout::new();
  let list = allowed(&["ls"]);
  let policy = CommandPolicy::new(&list).expect("policy");
  let scope = layout.scope(RepositoryAccess::Write);
  let shell = policy.classify(&run_command(&json!({"command": "ls", "shell": true})), &scope);
  assert!(matches!(shell, CommandVerdict::Ask(_)), "a shell command always asks");
  let outside = policy.classify(
    &run_command(&json!({"command": "ls", "workdir": text(&layout.outside)})),
    &scope,
  );
  assert!(matches!(outside, CommandVerdict::Refuse(CommandRefusal::Workdir(_))));
  let piped = policy.classify(&run_command(&json!({"command": "ls | wc"})), &scope);
  assert_eq!(
    piped,
    CommandVerdict::Refuse(CommandRefusal::Syntax(SyntaxError::Operator("|".to_owned())))
  );
  assert_eq!(
    CommandPolicy::new(&CommandPermission::AnyInWorktree),
    Err(CommandPolicyError::AnyCommandUnavailable)
  );
}
