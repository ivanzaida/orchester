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
