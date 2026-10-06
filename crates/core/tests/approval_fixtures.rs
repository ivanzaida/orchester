//! The 59 commands Claude Code asked about in four real runs (runs 22–25, 2026-10-05, about 1.5 h), as
//! Orchester would answer them now: how many run without a person (`run::permitted`), how many ask
//! with Approve similar available, and how many ask without it. The runs' worktrees, Git Bash's
//! `/h/…` and `H:/…` forms of them and the home folder are mapped onto a temporary folder; the profile
//! lists the checks a builder runs. In a read-only step a line that writes files through the shell asks; in
//! a step that edits files, one that writes only in the worktree runs, as the edit tools would.
//! `cargo test -p orchester-core --test approval_fixtures approval_fixtures`.

use std::path::{Path, PathBuf};

use orchester_core::{
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
  },
  run::{ApprovalSubject, RuleScope, permitted},
};
use serde::Deserialize;

const FIXTURE: &str = include_str!("fixtures/approvals/runs-22-25.json");
const REPOSITORY: &str = "projects/orchester/.orchester/worktrees/";
const REGISTRY: &str = "index.crates.io-1949cf8c6b5b557f";

#[derive(Deserialize)]
struct Request {
  action: String,
  target: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Table {
  /// Run without a person.
  runs: usize,
  /// Ask, with Approve similar available.
  similar: usize,
  /// Ask as Unknown: not read to the end.
  unknown: usize,
  /// Ask without Approve similar for another reason (several different parts ask, a credential).
  other: usize,
}

struct Machine {
  _root: tempfile::TempDir,
  home: PathBuf,
  worktrees: PathBuf,
}

impl Machine {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktrees = root.path().join(REPOSITORY);
    let lurq = home.join(".cargo/registry/src").join(REGISTRY).join("lurq-0.40.1");
    for folder in [&worktrees, &lurq.join("src/app"), &root.path().join("temp")] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    Self {
      _root: root,
      home,
      worktrees,
    }
  }

  /// The request's worktree here, and its command with the machine's paths mapped onto it.
  fn place(&self, request: &Request) -> (PathBuf, String) {
    let id = request
      .target
      .rsplit(['\\', '/'])
      .next()
      .expect("the target names the worktree");
    let worktree = self.worktrees.join(id);
    std::fs::create_dir_all(worktree.join(".tmp/orc59")).expect("worktree");
    let worktrees = slashed(&self.worktrees);
    let command = request
      .action
      .replace("C:/Users/someone", &slashed(&self.home))
      .replace(&format!("/h/{REPOSITORY}"), &format!("{}/", bash_form(&worktrees)))
      .replace(&format!("H:/{REPOSITORY}"), &format!("{worktrees}/"));
    (worktree, command)
  }
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

/// A path with forward slashes as Git Bash names it on Windows (`C:/x` is `/c/x`); elsewhere as it is.
fn bash_form(path: &str) -> String {
  match path.as_bytes() {
    [drive, b':', ..] if cfg!(windows) => format!("/{}{}", char::from(drive.to_ascii_lowercase()), &path[2..]),
    _ => path.to_owned(),
  }
}

fn builder() -> CommandPermission {
  let entries = ["cargo test", "cargo check", "cargo clippy", "cargo fmt"]
    .iter()
    .map(|entry| CommandPrefix::new(entry).expect("prefix"))
    .collect();
  CommandPermission::Allowed(AllowedCommands::new(entries).expect("allowed commands"))
}

/// How the fixture's requests are answered in a builder's step with `repository` access.
fn answered(repository: RepositoryAccess) -> Table {
  let requests: Vec<Request> = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
  assert_eq!(requests.len(), 59);
  let machine = Machine::new();
  let permissions = AgentPermissions::new(repository, builder(), NetworkAccess::On);
  let mut table = Table::default();
  for request in &requests {
    let (worktree, line) = machine.place(request);
    let scope = CommandScope::new(&worktree, &[], Some(&machine.home), None);
    let command = ClassifiedCommand::classify(&line, &scope);
    let unknown = command.category() == CommandCategory::Unknown;
    let subject = ApprovalSubject::Command(command);
    let slot = if permitted(&subject, &permissions).is_some() {
      &mut table.runs
    } else if RuleScope::similar(&subject).is_ok() {
      &mut table.similar
    } else if unknown {
      &mut table.unknown
    } else {
      &mut table.other
    };
    *slot += 1;
  }
  table
}

#[test]
fn approval_fixtures_of_runs_22_to_25() {
  // On Windows a word that starts with `\` lies at the root of the drive: one `grep` pattern of the
  // fixture (`\.assemble(`) is then a path outside the worktree, which asks.
  let runs = if cfg!(windows) { 5 } else { 6 };
  assert_eq!(
    answered(RepositoryAccess::ReadOnly),
    Table {
      runs,
      similar: 46 - runs,
      unknown: 2,
      other: 11,
    }
  );
  // A step that edits files runs the shell edits in the worktree and inline code (`python -c`) too.
  let runs = if cfg!(windows) { 33 } else { 34 };
  assert_eq!(
    answered(RepositoryAccess::Write),
    Table {
      runs,
      similar: 57 - runs,
      unknown: 2,
      other: 0,
    }
  );
}
