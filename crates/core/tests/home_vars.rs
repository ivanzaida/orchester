//! `$HOME` and `$USERPROFILE` (also braced) are read as the home folder the agent's CLI was started
//! with, so a path through them is judged as the same path through `~`: a crate's source in Cargo's
//! registry is a read outside the worktree, a credential path still asks. A line that may set either
//! leaves them unread.
//! `cargo test -p orchester-core --test home_vars home_vars`.

use orchester_core::{
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
  },
  run::{ApprovalSubject, RuleScope, permitted},
};
use tempfile::TempDir;

struct Fixture {
  _root: TempDir,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let worktree = root.path().join("repo").join("w1");
    let home = root.path().join("home");
    let lurq = home.join(".cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.41.2/src");
    for folder in [worktree.clone(), lurq.clone(), home.join(".ssh")] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(lurq.join("lib.rs"), "pub fn future() {}").expect("source");
    std::fs::write(home.join(".ssh/id_rsa"), "key").expect("key");
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&root.path().join("temp")));
    Self { _root: root, scope }
  }

  fn command(&self, line: &str) -> ClassifiedCommand {
    ClassifiedCommand::classify(line, &self.scope)
  }

  fn runs(&self, line: &str) -> bool {
    runs(self.command(line))
  }
}

/// Whether `command` runs unasked in a step that edits files.
fn runs(command: ClassifiedCommand) -> bool {
  let entry = CommandPrefix::new("cargo test").expect("prefix");
  let list = CommandPermission::Allowed(AllowedCommands::new(vec![entry]).expect("list"));
  let permissions = AgentPermissions::new(RepositoryAccess::Write, list, NetworkAccess::Off);
  permitted(&ApprovalSubject::Command(command), &permissions).is_some()
}

const REGISTRY: &str = ".cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.41.2/src";

#[test]
fn home_vars_name_the_home_folder() {
  let fixture = Fixture::new();
  let lines = [
    format!(r#"grep -rn "pub fn future\b\|pub fn future<" "$USERPROFILE/{REGISTRY}""#),
    r#"ls "$USERPROFILE/.cargo/registry/src/""#.to_owned(),
    format!("ls ${{HOME}}/{REGISTRY}"),
    format!("cat $HOME/{REGISTRY}/lib.rs"),
  ];
  for line in &lines {
    let command = fixture.command(line);
    assert_eq!(command.category(), CommandCategory::ReadOnly, "{line}");
    assert!(!command.reads_secrets(), "{line} reads no credential");
    let tilde = fixture.command(
      &line
        .replace("$USERPROFILE", "~")
        .replace("${HOME}", "~")
        .replace("$HOME", "~"),
    );
    assert_eq!(fixture.runs(line), runs(tilde), "{line} as with ~");
    assert!(
      RuleScope::similar(&ApprovalSubject::Command(command)).is_ok(),
      "{line} can be approved similarly"
    );
  }
}

#[test]
fn home_vars_keep_credentials_and_lines_that_set_them_asking() {
  let fixture = Fixture::new();
  for line in ["cat $HOME/.ssh/id_rsa", "cat \"${USERPROFILE}/.ssh/id_rsa\""] {
    assert!(fixture.command(line).reads_secrets(), "{line} reads a credential");
    assert!(!fixture.runs(line), "{line} must ask");
  }
  for line in [
    "HOME=/x; ls $HOME",
    "export HOME=/x; ls $HOME",
    "USERPROFILE=x ls $USERPROFILE",
    "read HOME; ls $HOME",
    "unset HOME; ls $HOME",
  ] {
    assert_eq!(fixture.command(line).category(), CommandCategory::Unknown, "{line}");
    assert!(!fixture.runs(line), "{line} must ask");
  }
}

/// Git Bash takes `$HOME` (and `~`) from the environment's `HOME`, which may differ from `USERPROFILE`:
/// each is read as its own value, and one that is not set leaves the path unknown.
#[test]
fn home_vars_read_each_variable_as_its_own_value() {
  let root = tempfile::tempdir().expect("temporary folder");
  let profile = root.path().join("profile");
  let other = root.path().join("other");
  let worktree = profile.join("proj").join("w");
  for folder in [worktree.join("x"), other.join("proj/w/x"), other.join(".ssh")] {
    std::fs::create_dir_all(folder).expect("folder");
  }
  let scope = CommandScope::new(&worktree, &[], Some(&profile), Some(&root.path().join("temp")))
    .with_shell_homes(Some(&other), Some(&profile));
  let write = |line: &str| {
    let entry = CommandPrefix::new("cargo test").expect("prefix");
    let list = CommandPermission::Allowed(AllowedCommands::new(vec![entry]).expect("list"));
    let permissions = AgentPermissions::new(RepositoryAccess::Write, list, NetworkAccess::Off);
    let command = ClassifiedCommand::classify(line, &scope);
    (
      permitted(&ApprovalSubject::Command(command.clone()), &permissions).is_some(),
      command,
    )
  };
  let (runs, command) = write("rm -rf $HOME/proj/w/x");
  assert!(!runs, "$HOME is not the profile");
  assert_eq!(command.category(), CommandCategory::HistoryDestructive);
  assert!(
    write("rm -rf $USERPROFILE/proj/w/x").0,
    "$USERPROFILE holds the worktree"
  );
  assert!(!write("rm -rf ~/proj/w/x").0, "~ is $HOME");
  assert!(
    write("cat ~/.ssh/id_rsa").1.reads_secrets(),
    "the shell's home holds credentials too"
  );
  let unset = CommandScope::new(&worktree, &[], Some(&profile), None).with_shell_homes(None, Some(&profile));
  for line in ["ls $HOME/proj", "ls ~/proj"] {
    assert_eq!(
      ClassifiedCommand::classify(line, &unset).category(),
      CommandCategory::Unknown,
      "{line} with HOME unset"
    );
  }
}
