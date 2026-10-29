//! Auto mode's hard rules for commands that write nothing, in a step that only reads (ORC-80): a read, and a lookup
//! over the network that keeps nothing (`cargo search`), go to the reviewer; a lookup or read that may write a tool's
//! own cache outside the worktree or a lock file still holds (the person's decision of 2026-10-23), as do writing
//! forms, redirects, aliases and manifests elsewhere, and the network rule while the step's network is off. In a step
//! that may write, a cache write is no write outside the folders.
//! `cargo test -p orchester-core --test auto_mode_lookups lookups`.

use std::path::PathBuf;

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  permission::{
    AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess,
    category::{CommandCategory, CommandScope},
  },
  resource::{CommandEnvironment, VariableName},
  run::{
    ApprovalSubject, PendingToolApproval,
    review::{HoldRules, PushFacts, ReviewHold, review_hold},
  },
  time::Timestamp,
};
use tempfile::TempDir;

/// A Cargo package's worktree in a home folder.
struct Fixture {
  root: TempDir,
  worktree: PathBuf,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let worktree = root.path().join("home").join("terrane");
    for folder in [&worktree, &root.path().join("temp")] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("Cargo.toml"), "[package]\nname = \"terrane\"\n").expect("manifest");
    Self { root, worktree }
  }

  fn scope(&self) -> CommandScope {
    CommandScope::new(
      &self.worktree,
      &[],
      Some(&self.root.path().join("home")),
      Some(&self.root.path().join("temp")),
    )
  }

  fn command(&self, command: &str, scope: &CommandScope) -> ApprovalSubject {
    let approval = PendingToolApproval {
      id: "invocation:1".to_owned(),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation: InvocationId::generate(),
      agent: "Explorer".to_owned(),
      command: true,
      scope: None,
      server: "command".to_owned(),
      action: command.to_owned(),
      target: None,
      requested_at: Timestamp::now(),
      native: None,
    };
    ApprovalSubject::of(&approval, scope)
  }

  /// The hold of `command` run in `scope`, in a step with `repository` and `network`.
  fn held_in(
    &self,
    scope: &CommandScope,
    command: &str,
    (repository, network): (RepositoryAccess, NetworkAccess),
  ) -> Option<ReviewHold> {
    let permissions = AgentPermissions::new(repository, CommandPermission::AskBefore, network);
    let rules = HoldRules {
      permissions: Some(&permissions),
      push: PushFacts {
        branch: Some("orc/ter-1"),
        head: Some("orc/ter-1"),
        origin_kept: true,
      },
      escalation: false,
      build_output: &[],
    };
    review_hold(&self.command(command, scope), &rules)
  }

  /// The hold of `command` in a read-only step whose network is on, as Research's Explorer runs.
  fn held(&self, command: &str) -> Option<ReviewHold> {
    self.held_in(&self.scope(), command, (RepositoryAccess::ReadOnly, NetworkAccess::On))
  }
}

#[test]
fn lookups_that_write_nothing_go_to_the_reviewer_in_a_read_only_step() {
  let fixture = Fixture::new();
  for command in [
    "cargo search lurq --limit 3",
    "cargo search lurq",
    "cargo locate-project",
    "cargo version",
    "cargo --version",
    "cargo -V",
    "cargo --list",
    "cargo verify-project",
    "cargo metadata --no-deps --format-version 1",
    "cargo metadata --frozen --format-version 1",
    "cargo tree --frozen",
    "cargo tree --offline --locked",
    "cargo --frozen tree",
    "cargo pkgid --frozen",
    "git log --oneline -5",
    "git show HEAD",
    "git diff",
    "git status",
    "git ls-remote origin",
    "git ls-remote --heads --tags --refs origin",
    "git ls-remote --sort=-version:refname --symref -q origin",
    "git remote show origin",
    "rustup show",
    "pip index versions lurq --no-cache-dir",
    "cargo search lurq --limit 3 2>/dev/null",
    "cargo search lurq && git status",
  ] {
    assert_eq!(fixture.held(command), None, "{command}");
  }
}

#[test]
fn lookups_that_may_write_a_tool_cache_hold_in_a_read_only_step() {
  let fixture = Fixture::new();
  for command in [
    "cargo info serde",
    "cargo info serde --offline",
    "cargo metadata --format-version 1",
    "cargo metadata --offline",
    "cargo metadata --locked",
    "cargo metadata -- --frozen",
    "cargo tree",
    "cargo tree --offline",
    "cargo pkgid",
    "npm view react",
    "npm outdated",
    "pip index versions lurq",
    "cargo search lurq && cargo tree",
  ] {
    assert_eq!(fixture.held(command), Some(ReviewHold::ReadOnlyStep), "{command}");
  }
}

#[test]
fn lookups_keep_the_writing_forms_and_the_other_hard_rules() {
  let fixture = Fixture::new();
  // A Cargo form Orchester does not read to the end holds as a write in a read-only step (ORC-339).
  for (command, hold) in [
    ("cargo fmt", ReviewHold::ReadOnlyStep),
    ("cargo fix --allow-dirty", ReviewHold::ReadOnlyStep),
    ("cargo add serde", ReviewHold::ReadOnlyStep),
    ("cargo install ripgrep", ReviewHold::ReadOnlyStep),
    ("cargo publish", ReviewHold::ReadOnlyStep),
    ("cargo search lurq > crates.txt", ReviewHold::ReadOnlyStep),
    ("cargo search lurq >> notes/crates.txt", ReviewHold::ReadOnlyStep),
    ("cargo s lurq", ReviewHold::ReadOnlyStep),
    (
      "cargo metadata --frozen --manifest-path ../other/Cargo.toml",
      ReviewHold::ReadOnlyStep,
    ),
    ("cargo search lurq --config net.offline=true", ReviewHold::ReadOnlyStep),
    ("git fetch origin", ReviewHold::ReadOnlyStep),
    ("git remote update", ReviewHold::ReadOnlyStep),
    ("cargo search lurq; cat ~/.cargo/credentials.toml", ReviewHold::Secrets),
  ] {
    assert_eq!(fixture.held(command), Some(hold), "{command}");
  }
  // A git option Orchester does not read (one that starts a program, or a shortening of one) goes to the reviewer,
  // which asks about a command that may start a program or reach the network (ORC-339).
  for command in [
    "git ls-remote --upload-pack=x origin",
    // Git takes a shortening of an option's name: `--upload` is `--upload-pack`.
    "git ls-remote --upload=x .",
    "git ls-remote --upload x .",
    "git ls-remote --exe=x origin",
    "git ls-remote --hea origin",
  ] {
    assert_eq!(fixture.held(command), None, "{command}");
  }
}

#[test]
fn lookups_hold_while_the_step_network_is_off() {
  let fixture = Fixture::new();
  let offline = (RepositoryAccess::ReadOnly, NetworkAccess::Off);
  for command in [
    "cargo search lurq --limit 3",
    "git ls-remote origin",
    "git remote show origin",
  ] {
    assert_eq!(
      fixture.held_in(&fixture.scope(), command, offline),
      Some(ReviewHold::NetworkOff),
      "{command}"
    );
  }
  assert_eq!(
    fixture.held_in(&fixture.scope(), "cargo locate-project", offline),
    None,
    "no network, no write"
  );
}

#[test]
fn lookups_count_only_under_cargos_own_names() {
  let fixture = Fixture::new();
  let environment = CommandEnvironment::new(
    [(
      VariableName::new("CARGO_ALIAS_SEARCH").expect("name"),
      "run --bin tool".to_owned(),
    )]
    .into_iter()
    .collect(),
  )
  .expect("environment");
  let aliased = fixture.scope().with_project_environment(&environment);
  let ApprovalSubject::Command(classified) = fixture.command("cargo search lurq", &aliased) else {
    panic!("a command")
  };
  assert_eq!(classified.category(), CommandCategory::Unknown);
  assert_eq!(
    fixture.held_in(
      &aliased,
      "cargo search lurq",
      (RepositoryAccess::ReadOnly, NetworkAccess::On)
    ),
    Some(ReviewHold::ReadOnlyStep)
  );
  assert_eq!(
    fixture.held_in(
      &aliased,
      "cargo locate-project",
      (RepositoryAccess::ReadOnly, NetworkAccess::On)
    ),
    None,
    "an alias of another name"
  );
}

#[test]
fn lookups_writing_a_tool_cache_go_to_the_reviewer_in_a_step_that_may_write() {
  let fixture = Fixture::new();
  let writing = (RepositoryAccess::Write, NetworkAccess::On);
  for command in [
    "cargo info serde",
    "cargo tree",
    "cargo metadata",
    "npm view react",
    "pip index versions lurq",
    "cargo search lurq --limit 3",
  ] {
    assert_eq!(fixture.held_in(&fixture.scope(), command, writing), None, "{command}");
  }
}
