//! The bypasses the security review of `f6bbd46a` found, and the older one on master: a dependency's folder
//! is code nobody here wrote. A program that loads settings or code from the folder it runs in (git's
//! `core.fsmonitor`, Cargo's `build.rustc-wrapper`) is Unknown anywhere but the worktree and its writable
//! folders; a reader that follows the links in a folder it walks (`diff -r`, `tree -l`, `du -L`) is Unknown
//! everywhere; only plain readers read the dependency sources as they read the worktree, and a `.git` there is
//! a credential location. Each line here must ask, never be approved by auto-approval or run by itself.
//! `cargo test -p orchester-core --test command_review_bypasses command_review`.

use std::path::Path;

use orchester_core::{
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
  },
  run::{ApprovalSubject, AutoVerdict, auto_verdict, permitted},
};
use tempfile::TempDir;

const CHECKOUT: &str = "~/.cargo/git/checkouts/lurq-4f1c/0a1b2c3";
const CRATE: &str = "~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.43.0";

/// A home folder with `.ssh`, a worktree, a writable folder, a folder elsewhere and Cargo's sources: a git
/// checkout whose dependency committed a `.cargo/config.toml` and a link `keys` to `~/.ssh`, and a crate with a
/// `.git/config` that names a program.
struct Fixture {
  root: TempDir,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("projects").join("kontur");
    let checkout = home.join(".cargo/git/checkouts/lurq-4f1c/0a1b2c3");
    let krate = home.join(".cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.43.0");
    for folder in [
      &worktree.join("src"),
      &worktree.join("empty"),
      &home.join(".ssh"),
      &checkout.join(".cargo"),
      &checkout.join("src"),
      &krate.join(".git"),
      &krate.join("src"),
      &root.path().join("build"),
      &root.path().join("elsewhere"),
      &root.path().join("temp"),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    for (file, text) in [
      (worktree.join("src/lib.rs"), "pub fn x() {}"),
      (home.join(".ssh/id_rsa"), "key"),
      (checkout.join("src/lib.rs"), "pub fn x() {}"),
      (
        checkout.join(".cargo/config.toml"),
        "[build]\nrustc-wrapper = \"./evil\"\n",
      ),
      (krate.join("src/lib.rs"), "pub fn x() {}"),
      (krate.join(".git/config"), "[core]\n\tfsmonitor = ./evil\n"),
    ] {
      std::fs::write(file, text).expect("file");
    }
    link_folder(&checkout.join("keys"), &home.join(".ssh"));
    link_folder(&worktree.join("keys"), &home.join(".ssh"));
    let scope = CommandScope::new(
      &worktree,
      &[root.path().join("build")],
      Some(&home),
      Some(&root.path().join("temp")),
    );
    Self { root, scope }
  }

  /// `line` with `{CHECKOUT}`, `{CRATE}`, `{BUILD}` (the writable folder) and `{ELSEWHERE}` filled in.
  fn command(&self, line: &str) -> ClassifiedCommand {
    let line = line
      .replace("{CHECKOUT}", CHECKOUT)
      .replace("{CRATE}", CRATE)
      .replace("{BUILD}", &slashed(&self.root.path().join("build")))
      .replace("{ELSEWHERE}", &slashed(&self.root.path().join("elsewhere")));
    ClassifiedCommand::classify(&line, &self.scope)
  }

  /// Whether `line` runs without a person: auto-approval approves it, or a builder's step that edits files
  /// runs it by itself.
  fn runs_unasked(&self, line: &str) -> bool {
    let subject = ApprovalSubject::Command(self.command(line));
    let entries = [
      "cargo test",
      "cargo metadata",
      "cargo tree",
      "git status",
      "git log",
      "git diff",
    ]
    .iter()
    .map(|entry| CommandPrefix::new(entry).expect("prefix"))
    .collect();
    let list = CommandPermission::Allowed(AllowedCommands::new(entries).expect("allowed commands"));
    let permissions = AgentPermissions::new(RepositoryAccess::Write, list, NetworkAccess::On);
    matches!(auto_verdict(&subject), AutoVerdict::Approve(_)) || permitted(&subject, &permissions).is_some()
  }

  fn assert_asks(&self, lines: &[String], category: CommandCategory) {
    for line in lines {
      assert_eq!(self.command(line).category(), category, "`{line}`");
      assert!(!self.runs_unasked(line), "`{line}` must ask");
    }
  }
}

fn link_folder(link: &Path, target: &Path) {
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("link");
  #[cfg(windows)]
  {
    // `cmd` reads a `/` in a path as a switch.
    let native = |path: &Path| path.to_string_lossy().replace('/', "\\");
    let made = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(native(link))
      .arg(native(target))
      .output()
      .expect("mklink starts");
    assert!(made.status.success(), "junction: {made:?}");
  }
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

fn lines(template: &str, fills: &[&str]) -> Vec<String> {
  fills.iter().map(|fill| template.replace("{}", fill)).collect()
}

#[test]
fn command_review_cargo_in_a_dependency_checkout_is_unknown() {
  let fixture = Fixture::new();
  let lines = lines(
    "cd {CHECKOUT} && cargo {}",
    &["metadata --format-version 1", "tree", "pkgid", "locate-project"],
  );
  fixture.assert_asks(&lines, CommandCategory::Unknown);
}

#[test]
fn command_review_readers_that_follow_links_are_unknown() {
  let fixture = Fixture::new();
  let lines: Vec<String> = [
    "diff -rN {CHECKOUT} empty",
    "diff --recursive {CHECKOUT} empty",
    "tree -l {CHECKOUT}",
    "du -L {CHECKOUT}",
    "du --dereference -a {CHECKOUT}",
    // In the worktree too: a link the agent made leads out of it.
    "diff -r src empty",
    "diff -rN keys empty",
    "tree -l .",
  ]
  .iter()
  .map(|line| (*line).to_owned())
  .collect();
  fixture.assert_asks(&lines, CommandCategory::Unknown);
  // The same readers that do not follow links read as before; outside the worktree they reach outside.
  assert_eq!(
    fixture.command("diff -r --no-dereference src empty").category(),
    CommandCategory::ReadOnly
  );
  assert_eq!(fixture.command("du -sh src").category(), CommandCategory::ReadOnly);
  let outside = fixture.command("diff -rN --no-dereference {CHECKOUT} empty");
  assert!(outside.parts().iter().any(|part| part.reaches_outside()));
  // A link named on the line is followed to where it leads.
  assert!(fixture.command("cat {CHECKOUT}/keys/id_rsa").reads_secrets());
}

#[test]
fn command_review_git_in_a_crate_is_unknown() {
  let fixture = Fixture::new();
  let lines = lines(
    "cd {CRATE} && git {}",
    &[
      "status",
      "diff",
      "log --oneline -3",
      "grep -n fn",
      "rev-parse HEAD",
      "config --list",
      "submodule status",
    ],
  );
  fixture.assert_asks(&lines, CommandCategory::Unknown);
  for line in ["cat {CRATE}/.git/config", "grep -rn fsmonitor {CRATE}/.git"] {
    assert!(
      fixture.command(line).reads_secrets(),
      "`{line}` reads a repository's own files"
    );
  }
}

#[test]
fn command_review_programs_that_load_their_folders_settings_run_only_inside() {
  let fixture = Fixture::new();
  let lines = lines(
    "cd {ELSEWHERE} && {}",
    &[
      "cargo metadata --format-version 1",
      "git status",
      "git log -3",
      "cargo test",
      "npm test",
      "make",
      "python -c 'print(1)'",
    ],
  );
  fixture.assert_asks(&lines, CommandCategory::Unknown);
  // In the worktree and a writable folder their settings are the agent's own.
  for line in [
    "cargo metadata --format-version 1",
    "git status",
    "cd {BUILD} && git status",
  ] {
    let command = fixture.command(line);
    assert_ne!(command.category(), CommandCategory::Unknown, "`{line}`");
    assert!(fixture.runs_unasked(line), "`{line}` runs");
  }
}

#[test]
fn command_review_plain_readers_still_read_dependency_sources() {
  let fixture = Fixture::new();
  for line in [
    "cat {CHECKOUT}/src/lib.rs",
    "rg -n 'pub fn' {CHECKOUT}",
    "grep -rn 'pub fn' {CRATE}/src | head -5",
    "ls -R {CHECKOUT}",
  ] {
    let command = fixture.command(line);
    assert_eq!(command.category(), CommandCategory::ReadOnly, "`{line}`");
    assert!(!command.reads_secrets(), "`{line}`");
    assert!(command.parts().iter().all(|part| !part.reaches_outside()), "`{line}`");
  }
  // After a `cd` there, the folder the line works in is outside the worktree.
  let moved = fixture.command("cd {CHECKOUT} && cat src/lib.rs");
  assert_eq!(moved.category(), CommandCategory::ReadOnly);
  assert!(moved.parts().iter().any(|part| part.reaches_outside()));
}
