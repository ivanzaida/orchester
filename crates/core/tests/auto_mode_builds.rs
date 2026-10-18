//! Auto mode's hard rules for Cargo builds in a step that only reads (the person's decision of 2026-10-18): a build,
//! check, test or clippy whose only write is its target directory, git-ignored in the worktree or in a writable
//! folder, goes to the reviewer; anything else that writes still holds, as do the other hard rules.
//! `cargo test -p orchester-core --test auto_mode_builds builds`.

use std::path::{Path, PathBuf};

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

/// A Cargo workspace's worktree in a home folder, with a writable folder beside it.
struct Fixture {
  root: TempDir,
  worktree: PathBuf,
  shared: PathBuf,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("kontur");
    let shared = home.join("shared");
    for folder in [&worktree, &shared, &root.path().join("temp")] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/*\"]\n").expect("manifest");
    Self { root, worktree, shared }
  }

  fn scope(&self) -> CommandScope {
    CommandScope::new(
      &self.worktree,
      std::slice::from_ref(&self.shared),
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
      agent: "Reviewer".to_owned(),
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
}

fn read_only(network: NetworkAccess) -> AgentPermissions {
  AgentPermissions::new(RepositoryAccess::ReadOnly, CommandPermission::AskBefore, network)
}

fn rules<'a>(permissions: &'a AgentPermissions, ignored: &'a [PathBuf]) -> HoldRules<'a> {
  HoldRules {
    permissions: Some(permissions),
    push: PushFacts {
      branch: Some("orc/kon-1"),
      head: Some("orc/kon-1"),
      origin_kept: true,
    },
    escalation: false,
    build_output: ignored,
  }
}

/// The target directories of `subject`'s builds, as the runner asks git about them.
fn targets(subject: &ApprovalSubject) -> Vec<PathBuf> {
  let ApprovalSubject::Command(command) = subject else {
    return Vec::new();
  };
  command
    .parts()
    .iter()
    .filter_map(|part| part.build_output())
    .map(|output| output.folder().to_path_buf())
    .collect()
}

/// The hold of `command` in a read-only step whose repository ignores every build output folder it names.
fn held(fixture: &Fixture, scope: &CommandScope, command: &str) -> Option<ReviewHold> {
  let subject = fixture.command(command, scope);
  let ignored = targets(&subject);
  review_hold(&subject, &rules(&read_only(NetworkAccess::On), &ignored))
}

#[test]
fn builds_that_write_only_ignored_output_go_to_the_reviewer() {
  let fixture = Fixture::new();
  let scope = fixture.scope();
  for command in [
    "cargo test -p orchester-core --test task_requirements _",
    "cargo test -p orchester-workspace --test orchestrator_waits",
    "cargo check -p orchester-core",
    "cargo build --release",
    "cargo clippy --all-targets -- -D warnings",
    "cargo test -p orchester-core -- --nocapture --test-threads=4 waits_ --skip slow -q",
    "cargo test -- --exact --test-threads 1 --format pretty --color never --show-output",
    "cargo bench -- --list --include-ignored",
    "cargo clippy --all-targets -- -D warnings -Wclippy::pedantic --allow dead_code",
    "cargo doc --no-deps",
    "cargo +nightly test",
    "cargo test --target-dir target/alt",
    "cargo test 2>&1",
    "cargo test 2>/dev/null",
    "cargo check && git status",
  ] {
    assert_eq!(held(&fixture, &scope, command), None, "{command}");
  }
  let subject = fixture.command("cargo test", &scope);
  let [folder] = targets(&subject).try_into().expect("one target directory");
  assert!(folder.ends_with("target"), "{}", folder.display());
  let offline = read_only(NetworkAccess::Off);
  assert_eq!(review_hold(&subject, &rules(&offline, &[folder])), None);
}

#[test]
fn builds_hold_while_git_does_not_ignore_their_output() {
  let fixture = Fixture::new();
  let scope = fixture.scope();
  let permissions = read_only(NetworkAccess::On);
  for command in ["cargo test", "cargo check", "cargo test --target-dir out"] {
    let subject = fixture.command(command, &scope);
    assert_eq!(
      review_hold(&subject, &rules(&permissions, &[])),
      Some(ReviewHold::ReadOnlyStep),
      "{command}"
    );
  }
}

#[test]
fn builds_that_change_sources_or_point_elsewhere_still_hold() {
  let fixture = Fixture::new();
  let scope = fixture.scope();
  for (command, hold) in [
    ("cargo fmt", ReviewHold::ReadOnlyStep),
    ("cargo fmt --all", ReviewHold::ReadOnlyStep),
    ("cargo fix --allow-dirty", ReviewHold::Unreadable),
    ("cargo clippy --fix", ReviewHold::ReadOnlyStep),
    ("cargo add serde", ReviewHold::ReadOnlyStep),
    ("cargo install ripgrep", ReviewHold::ReadOnlyStep),
    ("cargo publish", ReviewHold::Unreadable),
    ("cargo run -p orchester-app", ReviewHold::ReadOnlyStep),
    ("cargo doc --open", ReviewHold::ReadOnlyStep),
    ("cargo clean", ReviewHold::ReadOnlyStep),
    ("cargo test --manifest-path ../other/Cargo.toml", ReviewHold::Unreadable),
    ("cargo test --manifest-path Cargo.toml", ReviewHold::ReadOnlyStep),
    ("cargo test --target-dir ../../outside", ReviewHold::Unreadable),
    ("cargo test --target-dir .git/build", ReviewHold::ReadOnlyStep),
    ("cargo test --target-dir .", ReviewHold::ReadOnlyStep),
    ("cargo test --target-dir a --target-dir b", ReviewHold::ReadOnlyStep),
    ("cargo test > results.txt", ReviewHold::ReadOnlyStep),
    ("cargo test && touch notes.md", ReviewHold::ReadOnlyStep),
    ("CARGO_TARGET_DIR=/elsewhere cargo test", ReviewHold::Unreadable),
    ("cargo test --config build.target-dir=\"x\"", ReviewHold::Unreadable),
    ("cargo test -Zunstable-options --artifact-dir x", ReviewHold::Unreadable),
  ] {
    assert_eq!(held(&fixture, &scope, command), Some(hold), "{command}");
  }
  // Run below the worktree's root, Cargo may take another folder for its workspace.
  assert_eq!(
    held(&fixture, &scope, "cd crates && cargo test"),
    Some(ReviewHold::ReadOnlyStep)
  );
}

#[test]
fn builds_follow_the_target_directory_the_environment_names() {
  let fixture = Fixture::new();
  let outside = fixture.root.path().join("cache").join("target");
  let elsewhere = fixture.scope().with_cargo_target_dir(&outside);
  assert_eq!(
    held(&fixture, &elsewhere, "cargo test"),
    Some(ReviewHold::ReadOnlyStep),
    "a target directory outside the worktree and its writable folders"
  );
  let shared = fixture.scope().with_cargo_target_dir(&fixture.shared.join("target"));
  let subject = fixture.command("cargo test", &shared);
  assert_eq!(
    review_hold(&subject, &rules(&read_only(NetworkAccess::On), &[])),
    None,
    "a writable folder outside the worktree is the step's to write"
  );
  let inside = fixture.scope().with_cargo_target_dir(Path::new(".orchester-target"));
  assert_eq!(held(&fixture, &inside, "cargo check"), None);
  assert_eq!(
    review_hold(
      &fixture.command("cargo check", &inside),
      &rules(&read_only(NetworkAccess::On), &[])
    ),
    Some(ReviewHold::ReadOnlyStep),
    "in the worktree only where git ignores it"
  );
}

#[test]
fn builds_hold_where_a_cargo_configuration_or_another_workspace_may_move_the_output() {
  let fixture = Fixture::new();
  let scope = fixture.scope();
  let config = fixture.worktree.join(".cargo");
  std::fs::create_dir_all(&config).expect("config folder");
  std::fs::write(config.join("config.toml"), "[build]\ntarget-dir = \"../elsewhere\"\n").expect("config");
  assert_eq!(held(&fixture, &scope, "cargo test"), Some(ReviewHold::ReadOnlyStep));
  assert_eq!(
    held(&fixture, &scope, "cargo test --target-dir target"),
    None,
    "--target-dir wins over the configuration"
  );
  std::fs::remove_file(config.join("config.toml")).expect("config removed");
  std::fs::write(fixture.worktree.join("Cargo.toml"), "[package]\nname = \"kontur\"\n").expect("package");
  std::fs::write(fixture.root.path().join("home").join("Cargo.toml"), "[workspace]\n").expect("outer");
  assert_eq!(
    held(&fixture, &scope, "cargo test"),
    Some(ReviewHold::ReadOnlyStep),
    "a workspace above the worktree may take the package in"
  );
}

#[test]
fn builds_keep_the_other_hard_rules_and_writing_steps_unchanged() {
  let fixture = Fixture::new();
  let scope = fixture.scope();
  assert_eq!(
    held(&fixture, &scope, "cargo test && curl https://example.dev"),
    Some(ReviewHold::ReadOnlyStep)
  );
  assert_eq!(
    held(&fixture, &scope, "cargo test; cat ~/.ssh/id_rsa"),
    Some(ReviewHold::Secrets)
  );
  let writing = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  for command in ["cargo test", "cargo fmt", "cargo test --target-dir out"] {
    let subject = fixture.command(command, &scope);
    assert_eq!(review_hold(&subject, &rules(&writing, &[])), None, "{command}");
  }
}

#[test]
fn builds_hold_when_what_follows_their_double_dash_may_write() {
  let fixture = Fixture::new();
  let scope = fixture.scope();
  for command in [
    "cargo test -- --logfile src/main.rs",
    "cargo test -- --logfile=notes.txt",
    "cargo bench -- --save-baseline main",
    "cargo test -- --report-time",
    "cargo clippy -- --emit=metadata=out.rmeta",
    "cargo clippy -- -o out.rmeta",
    "cargo clippy -- warnings",
    "cargo clippy -- -D",
    "cargo build -- extra",
    "cargo check -- --emit=metadata",
  ] {
    assert_eq!(
      held(&fixture, &scope, command),
      Some(ReviewHold::ReadOnlyStep),
      "{command}"
    );
  }
}

#[test]
fn builds_point_their_outputs_only_inside_the_worktree() {
  let fixture = Fixture::new();
  let scope = fixture.scope();
  let outside = fixture.root.path().join("elsewhere.rmeta");
  for command in [
    format!("cargo clippy -- --emit=metadata={}", outside.display()),
    format!("cargo clippy -- --emit metadata={}", outside.display()),
    format!("cargo clippy -- -o {}", outside.display()),
  ] {
    let ApprovalSubject::Command(classified) = fixture.command(&command, &scope) else {
      panic!("a command")
    };
    assert_eq!(classified.category(), CommandCategory::Unknown, "{command}");
    assert_eq!(
      held(&fixture, &scope, &command),
      Some(ReviewHold::Unreadable),
      "{command}"
    );
  }
}

#[test]
fn builds_count_only_under_cargos_own_names() {
  let fixture = Fixture::new();
  let scope = fixture.scope();
  for command in ["cargo t", "cargo b --release", "cargo c -p kontur"] {
    assert_eq!(
      held(&fixture, &scope, command),
      Some(ReviewHold::ReadOnlyStep),
      "{command}"
    );
  }
  assert_eq!(held(&fixture, &scope, "cargo clippy"), None);
  // `clippy` is no built-in subcommand: an alias of a configuration may stand in its place.
  let config = fixture.worktree.join(".cargo");
  std::fs::create_dir_all(&config).expect("config folder");
  std::fs::write(config.join("config.toml"), "[alias]\nclippy = \"run --bin tool\"\n").expect("alias");
  assert_eq!(held(&fixture, &scope, "cargo clippy"), Some(ReviewHold::ReadOnlyStep));
  assert_eq!(
    held(&fixture, &scope, "cargo test"),
    None,
    "built-in subcommands cannot be aliased"
  );
  std::fs::write(config.join("config.toml"), "include = \"more.toml\"\n").expect("include");
  assert_eq!(
    held(&fixture, &scope, "cargo test"),
    Some(ReviewHold::ReadOnlyStep),
    "an include may move the target"
  );
  std::fs::write(
    config.join("config.toml"),
    r#"[build]
"target\u002Ddir" = "../x"
"#,
  )
  .expect("escaped");
  assert_eq!(
    held(&fixture, &scope, "cargo test"),
    Some(ReviewHold::ReadOnlyStep),
    "an escaped key"
  );
}

#[test]
fn builds_take_the_target_directory_from_the_project_environment_as_cargo_does() {
  let fixture = Fixture::new();
  let environment = |pairs: &[(&str, &str)]| {
    CommandEnvironment::new(
      pairs
        .iter()
        .map(|(name, value)| (VariableName::new(name).expect("name"), (*value).to_owned()))
        .collect(),
    )
    .expect("environment")
  };
  let outside = fixture.root.path().join("cache").display().to_string();
  // `CARGO_TARGET_DIR` wins over `CARGO_BUILD_TARGET_DIR`.
  let both = fixture.scope().with_project_environment(&environment(&[
    ("CARGO_TARGET_DIR", "inside-target"),
    ("CARGO_BUILD_TARGET_DIR", outside.as_str()),
  ]));
  assert_eq!(held(&fixture, &both, "cargo test"), None);
  let build_only = fixture
    .scope()
    .with_project_environment(&environment(&[("CARGO_BUILD_TARGET_DIR", outside.as_str())]));
  assert_eq!(
    held(&fixture, &build_only, "cargo test"),
    Some(ReviewHold::ReadOnlyStep)
  );
  // The project's Cargo home is read for its configuration too.
  let home = fixture.root.path().join("cargo-home");
  std::fs::create_dir_all(&home).expect("home");
  std::fs::write(home.join("config.toml"), "[build]\ntarget-dir = \"/elsewhere\"\n").expect("config");
  let with_home = fixture
    .scope()
    .with_project_environment(&environment(&[("CARGO_HOME", &home.display().to_string())]));
  assert_eq!(held(&fixture, &with_home, "cargo test"), Some(ReviewHold::ReadOnlyStep));
  assert_eq!(held(&fixture, &fixture.scope(), "cargo test"), None);
}
