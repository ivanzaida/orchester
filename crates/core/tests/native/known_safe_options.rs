//! The known-safe table admits only the options it lists: help viewers, files of paths, values hidden
//! in short-option clusters and Windows `where` search forms all ask.

use orchester_core::permission::known_safe::{GIT_SUBCOMMANDS, PROGRAMS, readable_arguments};

use crate::known_safe::safe;

fn arguments(command: &str) -> Option<Vec<String>> {
  let argv: Vec<String> = command.split(' ').map(str::to_owned).collect();
  readable_arguments(&argv).map(|args| args.into_iter().map(str::to_owned).collect())
}

/// Every program and git subcommand the table knows, as the command that starts it.
fn every_command() -> Vec<String> {
  PROGRAMS
    .iter()
    .filter(|program| **program != "git")
    .map(|program| (*program).to_owned())
    .chain(GIT_SUBCOMMANDS.iter().map(|subcommand| format!("git {subcommand}")))
    .collect()
}

#[test]
fn native_known_safe_unlisted_options_ask() {
  for command in every_command() {
    assert!(safe(&command), "{command} alone");
    for option in ["--orchester-unknown", "-Y", "--help"] {
      let with = format!("{command} {option}");
      // `echo` prints an option it does not know as written.
      assert_eq!(safe(&with), command == "echo", "{with}");
    }
  }
  assert!(safe("echo ---"));
}

#[test]
fn native_known_safe_git_help_asks() {
  for subcommand in GIT_SUBCOMMANDS {
    let command = format!("git {subcommand} --help");
    assert!(!safe(&command), "{command} starts a help viewer");
  }
}

#[test]
fn native_known_safe_files_of_paths_ask() {
  for command in [
    "find . -files0-from list",
    "find -files0-from list",
    "wc --files0-from=list",
    "wc --files0-from list",
    "file -f list",
    "file --files-from list",
    "file -bf list",
  ] {
    assert!(!safe(command), "{command} reads the paths a file names");
  }
}

#[test]
fn native_known_safe_values_in_clusters_are_checked() {
  let args = arguments("rg -nf/etc/x needle").expect("known-safe");
  assert!(args.contains(&"/etc/x".to_owned()), "{args:?}");
  let args = arguments("head -n20 a").expect("known-safe");
  assert!(args.contains(&"20".to_owned()), "{args:?}");
  let args = arguments("git grep -e/etc/y").expect("known-safe");
  assert!(args.contains(&"/etc/y".to_owned()), "{args:?}");
  assert!(!safe("file -bm/etc/passwd a"), "a magic file is not listed");
  assert!(!safe("rg -nL x"), "a letter after a flag is still checked");
}

#[test]
fn native_known_safe_find_values() {
  assert_eq!(
    arguments("find . -mtime -1 -newer a").expect("known-safe"),
    [".", "-mtime", "-1", "-newer", "a"]
  );
  assert!(safe("find src -name *.rs -o -type d -print"));
  assert!(!safe("find . -name"), "a value is missing");
  assert!(!safe("find . -printf %p"));
}

#[test]
fn native_known_safe_where_takes_bare_names() {
  for command in ["where git", "where /Q git", "where /f *.exe"] {
    assert!(safe(command), "{command}");
  }
  for command in [
    "where $PATH:id_rsa",
    r"where C:\Users:id_rsa",
    "where src:*.rs",
    "where /R . id_rsa",
  ] {
    assert!(!safe(command), "{command}");
  }
}

#[test]
fn native_known_safe_head_tail_counts() {
  for command in [
    "tail -n 20 a",
    "tail -n20 a",
    "tail -n +5 a",
    "tail -20 a",
    "head -c 1K a",
    "head -n -5 a",
    "head --lines=10 a",
    "tail --bytes 2KiB a",
    "tail -qn5 a b",
  ] {
    assert!(safe(command), "{command}");
  }
  for command in [
    "tail -cf a",
    "tail -nf a",
    "tail -c f a",
    "tail --lines=1f a",
    "tail --bytes x a",
    "tail +1f a",
    "tail +f a",
    "tail -1cf a",
    "tail -- +5lf",
    "head -n 1x a",
  ] {
    assert!(
      !safe(command),
      "{command}: a count that is not a number, or the follow form"
    );
  }
}
