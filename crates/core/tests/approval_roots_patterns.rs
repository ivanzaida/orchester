//! Autopilot's boundary for searches (ORC-612): the pattern of `rg`, `grep` and `git grep` is no path, so a regular
//! expression with backslashes (`\b`, `\(`) and `|` is not placed at the drive's root on Windows; their path operands
//! still are. `cargo test -p orchester-core --test approval_roots_patterns roots_pattern_args`.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::invocation::Platform,
  permission::{
    category::{ClassifiedCommand, CommandScope},
    roots::{RootsCheck, RunRoots, ToolRequest},
  },
};
use tempfile::TempDir;

/// A run's worktree `w1` with `src/lib.rs`, and a folder `elsewhere` outside it.
struct Fixture {
  _root: TempDir,
  worktree: PathBuf,
  elsewhere: PathBuf,
  roots: RunRoots,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let dir = tempfile::tempdir().expect("temporary folder");
    let root = std::fs::canonicalize(dir.path()).expect("canonical");
    let root = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\"));
    let worktree = root.join("w1");
    let elsewhere = root.join("elsewhere");
    for folder in [&worktree.join("src"), &elsewhere, &root.join("temp")] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("src").join("lib.rs"), "").expect("file");
    std::fs::write(elsewhere.join("notes.md"), "").expect("file");
    let roots = RunRoots::new(Platform::current(), &worktree, "orchester/orc-1-run-1").expect("worktree");
    let scope = CommandScope::new(&worktree, &[], Some(&root.join("home")), Some(&root.join("temp")));
    Self {
      _root: dir,
      worktree,
      elsewhere,
      roots,
      scope,
    }
  }

  /// `line` with `{WT}` and `{ELSE}` as the platform writes them, and `{WT/}` and `{ELSE/}` with forward slashes.
  fn command(&self, line: &str) -> RootsCheck {
    let line = line
      .replace("{WT/}", &slashed(&self.worktree))
      .replace("{ELSE/}", &slashed(&self.elsewhere))
      .replace("{WT}", &self.worktree.to_string_lossy())
      .replace("{ELSE}", &self.elsewhere.to_string_lossy());
    let command = ClassifiedCommand::classify(&line, &self.scope);
    self.roots.check(&ToolRequest::Command(&command))
  }

  fn outside(&self, path: &Path) -> RootsCheck {
    RootsCheck::Outside {
      path: path.to_path_buf(),
    }
  }
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

#[test]
fn roots_pattern_args_in_the_worktree_are_inside() {
  let fixture = Fixture::new();
  for line in [
    r#"rg -n "\ballow\(&|\ballow\(served|let_orchestrator\(" src"#,
    r#"rg -n '\bfn \w+\(' "{WT}""#,
    r#"rg -n '\bfn \w+\(' {WT/}/src"#,
    r#"rg -e '\bx\(' -e 'y|z' src {WT/}"#,
    r#"rg -ne'\bx\(' src"#,
    r#"rg --regexp='\bx|y' src"#,
    r#"rg --glob '*.rs' -i '\bx\(' src"#,
    r#"rg -n -- '\b-x\(' src"#,
    r#"grep -rn "\bfn \w\+(" src"#,
    r#"grep -n -e '\(a\|b\)' "{WT}/src/lib.rs""#,
    r#"grep -C3 -E '\bx|y' {WT/}/src/lib.rs"#,
    r#"egrep -n '\bx|\(y\)' src/lib.rs"#,
    r#"git grep -n "\bfoo\(" -- src"#,
    r#"git grep -e '\bfoo|bar' src"#,
    r#"git -C src grep -n '\bx\(' lib.rs"#,
  ] {
    assert_eq!(fixture.command(line), RootsCheck::Inside, "{line}");
  }
}

#[test]
fn roots_pattern_args_with_a_path_outside_are_outside() {
  let fixture = Fixture::new();
  let notes = fixture.elsewhere.join("notes.md");
  for (line, path) in [
    (r#"rg -n "\bfoo\(" "{ELSE}""#, &fixture.elsewhere),
    (r#"rg -n "\bfoo|bar" src {ELSE/}"#, &fixture.elsewhere),
    (r#"rg -e '\bx\(' "{ELSE}""#, &fixture.elsewhere),
    // No pattern among the operands: a file of patterns, or a listing.
    (r#"rg -f "{ELSE}/notes.md" src"#, &notes),
    (r#"rg --files {ELSE/}"#, &fixture.elsewhere),
    (r#"grep -n "\bfoo|bar" "{ELSE}/notes.md""#, &notes),
    (r#"grep -e '\(a\|b\)' {ELSE/}/notes.md"#, &notes),
    (r#"git grep -n '\bx\(' -- {ELSE/}"#, &fixture.elsewhere),
  ] {
    assert_eq!(fixture.command(line), fixture.outside(path), "{line}");
  }
}

/// An option the search's grammar does not know could take the pattern's place: every argument is placed as before,
/// so on Windows the backslash word still is no path inside the roots.
#[cfg(windows)]
#[test]
fn roots_pattern_args_not_told_are_placed() {
  let fixture = Fixture::new();
  for line in [
    r#"grep --frobnicate "\bx\(" src"#,
    // BSD `grep -C` takes no separate value: `3` would be the pattern and `\bx\(` a file.
    r#"grep -C 3 "\bx\(" src"#,
    // git takes `--` for the pattern when no `-e` comes before it.
    r#"git grep -- "\bx\(" src"#,
  ] {
    assert_ne!(fixture.command(line), RootsCheck::Inside, "{line}");
  }
}
