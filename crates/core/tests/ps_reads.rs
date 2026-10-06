//! Read-only PowerShell run with `-NoProfile`: network endpoint listings and the file readers
//! (`Get-Content`, `Get-ChildItem`, `Get-Item`, `Test-Path`, `Resolve-Path`) with only the parameters
//! that pick what they read are known-safe reads, in every step, their paths placed as `cat`'s are.
//! A credential path, a path outside, `-Stream`, other parameters and a `-NoProfile` that is part of
//! the script still ask.
//! `cargo test -p orchester-core --test ps_reads ps_reads`.

use orchester_core::{
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandScope},
  },
  run::{ApprovalSubject, permitted},
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
    for folder in [worktree.join("src"), home.join(".ssh")] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("src/a.txt"), "a").expect("file");
    std::fs::write(home.join(".ssh/id_rsa"), "key").expect("key");
    std::fs::write(root.path().join("repo/outside.txt"), "x").expect("outside");
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&root.path().join("temp")));
    Self { _root: root, scope }
  }

  fn runs(&self, line: &str, repository: RepositoryAccess) -> bool {
    let command = ClassifiedCommand::classify(line, &self.scope);
    let entry = CommandPrefix::new("cargo test").expect("prefix");
    let list = CommandPermission::Allowed(AllowedCommands::new(vec![entry]).expect("list"));
    let permissions = AgentPermissions::new(repository, list, NetworkAccess::Off);
    permitted(&ApprovalSubject::Command(command), &permissions).is_some()
  }
}

const RUNS: &[&str] = &[
  "powershell -NoProfile -Command \"Get-NetTCPConnection -LocalPort 47811 -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess\"",
  "pwsh -NoProfile -Command 'Get-NetUDPEndpoint -LocalPort 53 | Select-Object LocalAddress,OwningProcess'",
  "powershell -NoProfile -Command \"Get-Content src/a.txt -TotalCount 5\"",
  "pwsh -NoProfile -Command 'Get-ChildItem src -Recurse -Filter *.txt'",
  "pwsh -NoProfile -Command 'Test-Path -LiteralPath src/a.txt'",
  "pwsh -nop -Command 'Get-Item src/a.txt; Resolve-Path src'",
];

const ASKS: &[&str] = &[
  "pwsh -NoProfile -Command 'Get-Content ~/.ssh/id_rsa'",
  "pwsh -NoProfile -Command 'Get-Content ../outside.txt'",
  "pwsh -NoProfile -Command 'Get-Content src/a.txt -Stream x'",
  "pwsh -NoProfile -Command 'Get-ChildItem src -Exclude x'",
  "pwsh -NoProfile -Command 'Get-Content src/a.txt -OutVariable x'",
  "powershell -Command 'Get-ChildItem src' -nop",
  "powershell -Command 'Get-Content src/a.txt -NoProfile'",
  "powershell -Command 'Get-NetTCPConnection -LocalPort 1' -NoProfile",
  "pwsh -NoProfile -Command 'Get-NetTCPConnection -LocalPort 1 | Stop-Process'",
  "pwsh -NoProfile -Command 'Get-NetTCPConnection -LocalPort $(1)'",
  "pwsh -NoProfile -Command 'Get-NetTCPConnection -CimSession x'",
];

#[test]
fn ps_reads_run_in_every_step() {
  let fixture = Fixture::new();
  for line in RUNS {
    for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
      assert!(fixture.runs(line, repository), "`{line}` runs");
    }
  }
}

#[test]
fn ps_reads_ask_for_what_reaches_further() {
  let fixture = Fixture::new();
  for line in ASKS {
    for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
      assert!(!fixture.runs(line, repository), "`{line}` must ask");
    }
  }
}
