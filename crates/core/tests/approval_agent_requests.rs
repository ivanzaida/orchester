//! Requests of a Claude Code agent that reached the person although nothing in them was theirs to decide (ORC-627,
//! seen on 2026-11-02 on macOS with Approve automatically on), by their exact command lines: reading its own task
//! output, the code-signing queries a prefix allows, `git log -L` on a file of the worktree and listing rustup's
//! targets. Each is read to the end, placed in the run's roots, kept within the step's limits, and answered by
//! Autopilot, the step's allowed commands and Approve automatically.
//! `cargo test -p orchester-core --test approval_agent_requests approval_agent_requests`.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
    roots::{AgentHomes, RootsCheck, RunRoots, ToolRequest},
  },
  run::{
    AnswerKind, ApprovalSubject, AutoVerdict, ModeStep, RequestAnswer, auto_verdict, autopilot_check, autopilot_step,
    permitted,
    review::{StepLimit, step_limit},
  },
};
use tempfile::TempDir;

const BRANCH: &str = "orchester/orc-627-run-1";
const USER: u32 = 501;

/// A main checkout in a folder whose name holds a space, as `/Volumes/External SSD/…` does, with the run's worktree
/// in it and `crates/desktop-app/Cargo.toml` there; a Builder that edits, with its network off and `security
/// find-identity` among its allowed commands.
struct Fixture {
  _root: TempDir,
  worktree: PathBuf,
  elsewhere: PathBuf,
  roots: RunRoots,
  scope: CommandScope,
  permissions: AgentPermissions,
}

impl Fixture {
  fn new() -> Self {
    let dir = tempfile::tempdir().expect("temporary folder");
    let root = std::fs::canonicalize(dir.path()).expect("canonical");
    let root = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\"));
    let main = root.join("External SSD").join("orchester");
    let worktree = main.join(".orchester").join("worktrees").join("w1");
    let elsewhere = root.join("elsewhere");
    for folder in [
      &worktree.join("crates").join("desktop-app"),
      &elsewhere,
      &root.join("temp"),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("crates").join("desktop-app").join("Cargo.toml"), "").expect("file");
    let roots = RunRoots::new(Platform::current(), &worktree, BRANCH)
      .expect("worktree")
      .with_main_checkout(&main)
      .expect("main checkout")
      .with_agent_temp(
        AgentProvider::ClaudeCodeCli,
        AgentHomes {
          temp: &root.join("temp"),
          config: &root.join("config"),
          user: Some(USER),
        },
      );
    let scope = CommandScope::new(&worktree, &[], Some(&root.join("home")), Some(&root.join("temp")));
    let allowed = AllowedCommands::new(vec![CommandPrefix::new("security find-identity").expect("prefix")])
      .expect("allowed commands");
    let permissions = AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(allowed),
      NetworkAccess::Off,
    );
    Self {
      _root: dir,
      worktree,
      elsewhere,
      roots,
      scope,
      permissions,
    }
  }

  fn command(&self, line: &str) -> ClassifiedCommand {
    ClassifiedCommand::classify(line, &self.scope)
  }

  fn subject(&self, line: &str) -> ApprovalSubject {
    ApprovalSubject::Command(self.command(line))
  }

  /// What Autopilot does with `line`: by the roots, within the step's limits.
  fn autopilot(&self, line: &str) -> ModeStep {
    let subject = self.subject(line);
    let limit = step_limit(&subject, Some(&self.permissions), &[]);
    autopilot_step(&autopilot_check(&self.roots, &subject), limit)
  }

  /// `line` is read to the end as a read that installs nothing and reads no credential, Autopilot approves it by its
  /// rule, and Approve automatically may approve it.
  fn assert_answered(&self, line: &str) {
    let command = self.command(line);
    assert_eq!(command.category(), CommandCategory::ReadOnly, "{line}");
    assert!(!command.reads_secrets() && !command.installs(), "{line}");
    assert_eq!(
      self.autopilot(line),
      ModeStep::Approve(RequestAnswer::by(AnswerKind::Autopilot)),
      "{line}"
    );
    assert_eq!(
      auto_verdict(&self.subject(line)),
      AutoVerdict::Approve(Some(CommandCategory::ReadOnly)),
      "{line}"
    );
  }
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

/// Claude Code's background commands write their output to `/tmp/claude-<uid>/<worktree slug>/<session>/tasks/`
/// on macOS, where its `tail` reads it: the slug is the worktree with every character but a letter or digit, a
/// space too, as `-`.
#[cfg(target_os = "macos")]
#[test]
fn approval_agent_requests_reads_its_own_task_output() {
  let fixture = Fixture::new();
  let slug: String = fixture
    .worktree
    .to_string_lossy()
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
    .collect();
  assert!(slug.contains("-External-SSD-"), "{slug}");
  let output = format!("/private/tmp/claude-{USER}/{slug}/3f0c2a1e-session/tasks/bq5x2k.output");
  fixture.assert_answered(&format!("tail -n 40 {output}"));
  assert_eq!(
    fixture
      .roots
      .check(&ToolRequest::Files(&[output.replacen("/private", "", 1)])),
    RootsCheck::Inside
  );
  // Another worktree's task output is not the run's.
  let other = format!("/private/tmp/claude-{USER}/-Users-ana-other/s/tasks/b.output");
  assert!(matches!(
    autopilot_check(&fixture.roots, &fixture.subject(&format!("tail -n 40 {other}"))),
    RootsCheck::Outside { .. }
  ));
}

/// `security find-identity` lists the signing identities, no private key: a prefix of it in the allowed commands
/// runs it. `spctl --assess` reads the files it names, which are placed. The rest of `security` still reads a
/// credential store, and the rest of `spctl` changes the system's policy.
#[test]
fn approval_agent_requests_code_signing_queries_are_read_to_the_end() {
  let fixture = Fixture::new();
  let identities = "security find-identity -v -p codesigning";
  fixture.assert_answered(identities);
  assert_eq!(
    permitted(&fixture.subject(identities), &fixture.permissions),
    Some(CommandCategory::ReadOnly)
  );
  let assess = fixture.command("spctl -a -vv -t exec /Volumes/Orchester/Orchester.app");
  assert_eq!(assess.category(), CommandCategory::ReadOnly);
  assert!(!assess.reads_secrets());
  // The app it assesses is placed: outside the roots it comes to the person as such.
  let app = format!("{}/Orchester.app", slashed(&fixture.elsewhere));
  assert!(matches!(
    autopilot_check(&fixture.roots, &fixture.subject(&format!("spctl -a -vv -t exec {app}"))),
    RootsCheck::Outside { .. }
  ));
  std::fs::create_dir_all(fixture.worktree.join("target").join("Orchester.app")).expect("folder");
  fixture.assert_answered("spctl -a -vv -t exec target/Orchester.app");
  fixture.assert_answered("spctl --assess --verbose --type exec target/Orchester.app");

  for line in ["security dump-keychain", "security find-generic-password -s x -w"] {
    let command = fixture.command(line);
    assert!(command.reads_secrets(), "{line}");
    assert_eq!(command.category(), CommandCategory::Unknown, "{line}");
    assert_eq!(permitted(&fixture.subject(line), &fixture.permissions), None, "{line}");
  }
  for line in [
    "security find-identity -v -x codesigning",
    "spctl --master-disable",
    "spctl --add --label x target/Orchester.app",
    "spctl -a",
  ] {
    assert_eq!(fixture.command(line).category(), CommandCategory::Unknown, "{line}");
  }
}

/// The range of `git log -L` is no path: only its file, after the last colon, is placed.
#[test]
fn approval_agent_requests_git_log_line_range_is_no_path() {
  let fixture = Fixture::new();
  fixture.assert_answered(
    r#"git log --format="%h %ad %s" --date=iso -L "/^version/,+1:crates/desktop-app/Cargo.toml" --no-patch"#,
  );
  fixture.assert_answered("git log -L/^version/,+1:crates/desktop-app/Cargo.toml --no-patch");
  fixture.assert_answered("git log -L :main:crates/desktop-app/Cargo.toml -- crates");
  let outside = slashed(&fixture.elsewhere.join("Cargo.toml"));
  let check = autopilot_check(
    &fixture.roots,
    &fixture.subject(&format!("git log -L \"1,2:{outside}\" --no-patch")),
  );
  assert!(
    matches!(&check, RootsCheck::Outside { path } if path.ends_with("elsewhere/Cargo.toml")),
    "{check:?}"
  );
}

/// Listing or querying a toolchain or packages installs nothing; adding a target does.
#[test]
fn approval_agent_requests_listing_installs_nothing() {
  let fixture = Fixture::new();
  for line in [
    "rustup target list --installed",
    "rustup target list",
    "rustup toolchain list",
    "rustup component list --installed",
    "rustup show",
    "cargo --version",
    "brew list",
  ] {
    fixture.assert_answered(line);
    assert_eq!(
      step_limit(&fixture.subject(line), Some(&fixture.permissions), &[]),
      StepLimit::Within,
      "{line}"
    );
  }
  for line in [
    "rustup target add aarch64-apple-darwin",
    "rustup toolchain install nightly",
  ] {
    assert!(fixture.command(line).installs(), "{line}");
  }
}
