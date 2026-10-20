//! What the project's environment changes about Cargo builds in a step that only reads (the person's decisions of
//! 2026-10-18): an alias in it may stand in for a subcommand, a relative `CARGO_HOME` leaves Cargo's configuration
//! unknown, its names are read as Cargo reads them, and auto mode's reviewer is told the names, never the values, of
//! the variables that change what a build runs. A toolchain named with a folder is no build either.
//! `cargo test -p orchester-core --test auto_mode_build_environment build_environment`.

use std::path::PathBuf;

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  permission::{
    AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess,
    category::{CommandScope, build_variables},
  },
  resource::{CommandEnvironment, VariableName},
  run::{
    ApprovalSubject, PendingToolApproval,
    review::{
      HoldRules, ReviewHold, ReviewInput, ReviewedRequest, ReviewedStep, ReviewedTask, review_hold, review_prompt,
    },
  },
  time::Timestamp,
};
use tempfile::TempDir;

/// A Cargo workspace's worktree in a home folder.
struct Fixture {
  root: TempDir,
  worktree: PathBuf,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let worktree = root.path().join("home").join("kontur");
    std::fs::create_dir_all(&worktree).expect("worktree");
    std::fs::write(worktree.join("Cargo.toml"), "[workspace]\n").expect("manifest");
    Self { root, worktree }
  }

  fn scope(&self, environment: &[(&str, &str)]) -> CommandScope {
    CommandScope::new(&self.worktree, &[], Some(&self.root.path().join("home")), None)
      .with_project_environment(&variables(environment))
  }

  /// The hold of `command` in a read-only step whose repository ignores every build output folder it names.
  fn held(&self, scope: &CommandScope, command: &str) -> Option<ReviewHold> {
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
    let subject = ApprovalSubject::of(&approval, scope);
    let ApprovalSubject::Command(classified) = &subject else {
      panic!("a command")
    };
    let ignored: Vec<PathBuf> = classified
      .parts()
      .iter()
      .filter_map(|part| part.build_output())
      .map(|output| output.folder().to_path_buf())
      .collect();
    let permissions = AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AskBefore,
      NetworkAccess::On,
    );
    review_hold(
      &subject,
      &HoldRules {
        permissions: Some(&permissions),
        build_output: &ignored,
        ..HoldRules::default()
      },
    )
  }
}

fn variables(pairs: &[(&str, &str)]) -> CommandEnvironment {
  CommandEnvironment::new(
    pairs
      .iter()
      .map(|(name, value)| (VariableName::new(name).expect("name"), (*value).to_owned()))
      .collect(),
  )
  .expect("environment")
}

#[test]
fn build_environment_aliases_stand_in_for_subcommands() {
  let fixture = Fixture::new();
  let plain = fixture.scope(&[]);
  assert_eq!(fixture.held(&plain, "cargo clippy"), None);
  assert_eq!(fixture.held(&plain, "cargo test"), None);
  let other = fixture.scope(&[("CARGO_ALIAS_LINT", "run --bin lint")]);
  assert_eq!(
    fixture.held(&other, "cargo clippy"),
    Some(ReviewHold::ReadOnlyStep),
    "any alias may be clippy's"
  );
  assert_eq!(fixture.held(&other, "cargo test"), None);
  for (alias, command) in [
    ("CARGO_ALIAS_CLIPPY", "cargo clippy"),
    ("CARGO_ALIAS_TEST", "cargo test"),
  ] {
    let aliased = fixture.scope(&[(alias, "run --bin tool")]);
    assert_eq!(
      fixture.held(&aliased, command),
      Some(ReviewHold::ReadOnlyStep),
      "{alias}"
    );
  }
}

#[test]
fn build_environment_with_a_relative_cargo_home_leaves_the_configuration_unknown() {
  let fixture = Fixture::new();
  let relative = fixture.scope(&[("CARGO_HOME", "cargo-home")]);
  assert_eq!(fixture.held(&relative, "cargo test"), Some(ReviewHold::ReadOnlyStep));
  assert_eq!(
    fixture.held(&relative, "cargo clippy --target-dir target"),
    Some(ReviewHold::ReadOnlyStep)
  );
  assert_eq!(
    fixture.held(&relative, "cargo test --target-dir target"),
    None,
    "--target-dir wins over any configuration"
  );
}

#[test]
fn build_environment_toolchains_named_with_a_folder_are_no_build() {
  let fixture = Fixture::new();
  let scope = fixture.scope(&[]);
  assert_eq!(fixture.held(&scope, "cargo +nightly test"), None);
  for command in [
    "cargo +../tools/evil test",
    r"cargo +C:\toolchains\x test",
    "cargo +x:y check",
  ] {
    assert_eq!(
      fixture.held(&scope, command),
      Some(ReviewHold::ReadOnlyStep),
      "{command}"
    );
  }
}

#[test]
fn build_environment_names_are_read_as_cargo_reads_them() {
  let fixture = Fixture::new();
  let outside = fixture.root.path().join("cache").display().to_string();
  let lower = fixture.scope(&[("cargo_target_dir", outside.as_str())]);
  // Windows compares names without regard to case; macOS and Linux by their exact spelling.
  let expected = cfg!(windows).then_some(ReviewHold::ReadOnlyStep);
  assert_eq!(fixture.held(&lower, "cargo test"), expected);
  let names = build_variables(&variables(&[("rustc_wrapper", "x"), ("HOME", "y")]));
  assert_eq!(names.is_empty(), !cfg!(windows));
}

#[test]
fn build_environment_names_what_changes_a_build_to_the_reviewer_without_values() {
  let environment = variables(&[
    ("RUSTC_WRAPPER", "C:/secret/wrapper.exe"),
    ("RUSTFLAGS", "-C link-arg=/secret"),
    ("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER", "runner.exe"),
    ("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "w.exe"),
    ("RUSTUP_TOOLCHAIN", "nightly"),
    ("CARGO_TARGET_DIR", "target"),
    ("FAKE_REVIEW_LOG", "log.txt"),
  ]);
  let mut names = build_variables(&environment);
  names.sort();
  assert_eq!(
    names,
    [
      "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
      "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER",
      "RUSTC_WRAPPER",
      "RUSTFLAGS",
      "RUSTUP_TOOLCHAIN"
    ]
  );
  let input = ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: "cargo test".to_owned(),
      arguments: None,
      note: None,
      working_folder: None,
    },
    ReviewedStep {
      name: "Review".to_owned(),
      repository: RepositoryAccess::ReadOnly,
      network: NetworkAccess::On,
      writable_folders: Vec::new(),
    },
    ReviewedTask {
      title: "Check".to_owned(),
      brief: String::new(),
      criteria: Vec::new(),
    },
    &[],
  )
  .expect("within a review's size")
  .with_build_variables(names);
  let prompt = review_prompt(&input);
  assert!(
    prompt.contains("\"project_variables_that_change_what_a_build_runs\":[\"CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER\"")
  );
  for value in ["C:/secret/wrapper.exe", "-C link-arg=/secret", "runner.exe", "w.exe"] {
    assert!(!prompt.contains(value), "{value}");
  }
}
