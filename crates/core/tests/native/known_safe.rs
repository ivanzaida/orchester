//! The known-safe read-only command table, by its argument rules.

use orchester_core::permission::known_safe::{is_known_safe, path_candidates};

fn safe(command: &str) -> bool {
  let argv: Vec<String> = command.split(' ').map(str::to_owned).collect();
  is_known_safe(&argv)
}

#[test]
fn native_known_safe_plain_programs() {
  for command in [
    "ls -la src",
    "cat src/main.rs",
    "head -n 20 a",
    "tail a",
    "wc -l a",
    "pwd",
    "echo hi",
    "which git",
    "where git",
    "file a",
    "stat a",
  ] {
    assert!(safe(command), "{command}");
  }
  for command in [
    "rm -rf a",
    "sed -i s/a/b/ a",
    "cp a b",
    "python -c x",
    "./cat a",
    "bin/ls",
    "npm test",
  ] {
    assert!(!safe(command), "{command}");
  }
}

#[test]
fn native_known_safe_git() {
  for command in [
    "git status --short",
    "git log --oneline -5",
    "git diff HEAD~1",
    "git show HEAD:src/main.rs",
    "git rev-parse HEAD",
    "git ls-files",
    "git blame src/main.rs",
    "git grep -n needle",
    "git branch",
    "git branch -a -v",
    "git branch --list feature/*",
    "git --version",
  ] {
    assert!(safe(command), "{command}");
  }
  for command in [
    "git",
    "git commit -m x",
    "git push",
    "git -c core.pager=evil log",
    "git -C .. status",
    "git log -c",
    "git diff --output=x",
    "git log --output x",
    "git diff --ext-diff",
    "git show --textconv",
    "git grep -O needle",
    "git grep -iO needle",
    "git grep --open-files-in-pager=vim x",
    "git branch new-branch",
    "git branch -D main",
    "git branch --sort=x",
    "git config core.pager",
  ] {
    assert!(!safe(command), "{command}");
  }
}

#[test]
fn native_known_safe_rg_and_find() {
  for command in [
    "rg needle src",
    "rg -n -i needle",
    "rg --files",
    "find . -name *.rs",
    "find src -type f",
  ] {
    assert!(safe(command), "{command}");
  }
  for command in [
    "rg --pre cat x",
    "rg --pre=cat x",
    "rg --pre-glob *.gz x",
    "rg --search-zip x",
    "rg -z x",
    "rg -iz x",
    "rg --hostname-bin=x y",
    "find . -exec rm {}",
    "find . -execdir x",
    "find . -ok x",
    "find . -delete",
    "find . -fprint out",
    "find . -fls out",
    "file -C -m magic",
  ] {
    assert!(!safe(command), "{command}");
  }
}

#[test]
fn native_known_safe_leaves_out_cargo() {
  for command in ["cargo metadata", "cargo tree", "cargo --version", "cargo build"] {
    assert!(
      !safe(command),
      "{command}: rustc and the toolchain file can be replaced from the worktree"
    );
  }
}

#[cfg(windows)]
#[test]
fn native_known_safe_windows_program_names() {
  assert!(safe("GIT.EXE status"));
  assert!(safe("Cat.exe a"));
  assert!(!safe(r"C:\tools\cat.exe a"));
}

#[test]
fn native_known_safe_path_candidates() {
  let argv: Vec<String> = ["rg", "--ignore-file=/etc/x", "-f/etc/y", "needle", "--", "-n"]
    .map(str::to_owned)
    .into();
  assert_eq!(
    path_candidates(&argv),
    [
      "--ignore-file=/etc/x",
      "/etc/x",
      "-f/etc/y",
      "/etc/y",
      "needle",
      "--",
      "-n"
    ]
  );
}
