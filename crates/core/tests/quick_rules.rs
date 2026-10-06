//! The rules of 2026-10-06 for what Verify steps asked about: benign environment prefixes, executables
//! built in the worktree, process listings, `for` loops over literals, and inline code in a step that
//! edits files. Each is no wider than what ran before; the refusals hold the forms that would be.
//! `cargo test -p orchester-core --test quick_rules quick_rules`.

use std::path::Path;

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
  worktree: String,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let worktree = root.path().join("repo").join("w1");
    let verify = worktree.join(".tmp").join("verify");
    let elsewhere = root.path().join("elsewhere");
    for folder in [
      verify.join("orc50b"),
      verify.join("seed/target/debug"),
      verify.join("seeder-target/debug"),
      verify.join("repo"),
      worktree.join(".git"),
      elsewhere.clone(),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    for file in [
      verify.join("orc50b/drive.py"),
      verify.join("seed/target/debug/seed.exe"),
      verify.join("seeder-target/debug/orc50-seeder.exe"),
      verify.join("m.sh"),
      worktree.join(".git/x.exe"),
      elsewhere.join("x.exe"),
      root.path().join("repo/x.exe"),
    ] {
      std::fs::write(file, "").expect("file");
    }
    link_folder(&worktree.join("out"), &elsewhere);
    let home = root.path().join("home");
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&root.path().join("temp")));
    Self {
      worktree: worktree.to_string_lossy().replace('\\', "/"),
      _root: root,
      scope,
    }
  }

  fn runs(&self, line: &str, repository: RepositoryAccess) -> bool {
    let line = line.replace("{WT}", &self.worktree);
    let command = ClassifiedCommand::classify(&line, &self.scope);
    let entry = CommandPrefix::new("cargo test").expect("prefix");
    let list = CommandPermission::Allowed(AllowedCommands::new(vec![entry]).expect("list"));
    let permissions = AgentPermissions::new(repository, list, NetworkAccess::Off);
    permitted(&ApprovalSubject::Command(command), &permissions).is_some()
  }
}

/// A link to the folder `target` at `link`: a symbolic link on Unix, a junction on Windows.
fn link_folder(link: &Path, target: &Path) {
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("link");
  #[cfg(windows)]
  {
    let made = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(link)
      .arg(target)
      .output()
      .expect("mklink starts");
    assert!(made.status.success(), "junction: {made:?}");
  }
}

/// Lines of Verify steps that ran code of the worktree, and run in a step that edits files only.
const EDITING: &[&str] = &[
  "PYTHONIOENCODING=utf-8 python .tmp/verify/orc50b/drive.py find \"First edit\"",
  "NO_COLOR=1 RUST_BACKTRACE=1 python .tmp/verify/orc50b/drive.py",
  ".tmp/verify/seed/target/debug/seed.exe seed .tmp/verify/repo 3",
  ".tmp/verify/seeder-target/debug/orc50-seeder.exe {WT}/.tmp/verify/repo",
  "for i in 1 2 3 4 5 6 7 8 9 10 11 12; do sh .tmp/verify/m.sh lurq_read_tree '{\"id\":1}' | grep -o 'task-[a-z]*' | head -1; done",
  "sh .tmp/verify/m.sh lurq_read_tree '{}' | python -c \"import sys,json; print(json.load(sys.stdin))\"",
  "python -c \"print(1)\"",
  "PYTHONIOENCODING=utf-8 python3 -c 'import json'",
  "node -e \"console.log(1)\"",
  "node -p 1",
];

/// Process listings, which read only: they run in every step.
const LISTINGS: &[&str] = &[
  "tasklist //FI \"PID eq 99612\"",
  "tasklist //FI \"IMAGENAME eq orchester.exe\" //FO CSV",
  "tasklist /V /NH",
  "ps aux",
  "ps -ef",
  "ps -p 123 -o pid,comm",
  "powershell -NoProfile -Command \"Get-Process -Id 27196,33436 -ErrorAction SilentlyContinue | Select-Object Id,ProcessName\"",
  "powershell -NoProfile -Command \"Get-CimInstance Win32_Process -Filter \\\"Name='orchester.exe'\\\" | Select-Object ProcessId,CommandLine | Format-Table -AutoSize\"",
  "pwsh -NoProfile -Command 'Get-WmiObject Win32_Process | Where-Object { $_.Name -eq \"orchester.exe\" } | Format-List'",
  "pwsh -NoProfile -Command \"Get-Process orchester | Sort-Object CPU -Descending | Out-String -Width 200\"",
  "pwsh -NoProfile -Command \"Get-Process | Where-Object ProcessName -like 'orch*'\"",
];

const ASKS: &[&str] = &[
  // Environment prefixes that change what runs or reads.
  "PATH=./bin python .tmp/verify/orc50b/drive.py",
  "PYTHONPATH=x python -c 'print(1)'",
  "PYTHONPATH=x python .tmp/verify/orc50b/drive.py",
  "NODE_OPTIONS=--require=x node -e 1",
  "LD_PRELOAD=x tasklist",
  "PYTHONIOENCODING='$(id)' python .tmp/verify/orc50b/drive.py",
  "\"PYTHONIOENCODING=utf-8\" python .tmp/verify/orc50b/drive.py",
  "TERM=/x python .tmp/verify/orc50b/drive.py",
  "PYTHONIOENCODING=utf-8",
  // Executables that are no regular file of the worktree, or may be changed first.
  "out/x.exe",
  "../x.exe",
  ".git/x.exe",
  "C:/Windows/System32/whoami.exe",
  ".tmp/verify/missing.exe",
  "echo x > .tmp/verify/seed/target/debug/seed.exe && .tmp/verify/seed/target/debug/seed.exe",
  "cp out/x.exe .tmp/x.exe; .tmp/x.exe",
  // Listings that do more, and killing.
  "tasklist /S remote",
  "tasklist /U x /P y",
  "tasklist /M",
  "taskkill /PID 1",
  "taskkill //F //IM orchester.exe",
  "ps e",
  "ps eww",
  "ps -E",
  "powershell -Command \"Get-Process\"",
  "powershell -NoProfile -Command \"Get-Process; Stop-Process -Id 1\"",
  "powershell -NoProfile -Command \"Get-Process | Stop-Process\"",
  "powershell -NoProfile -Command \"Get-Process | ForEach-Object { $_.Kill() }\"",
  "powershell -NoProfile -Command \"Get-Process | Where-Object { $_.Kill() }\"",
  "powershell -NoProfile -Command \"Get-Process -Id $(1)\"",
  "powershell -NoProfile -Command \"Get-CimInstance Win32_Service\"",
  "powershell -NoProfile -Command \"Get-CimInstance Win32_Process | Invoke-CimMethod -MethodName Terminate\"",
  "powershell -NoProfile -Command \"iex 'Get-Process'\"",
  "powershell -NoProfile -Command \"Get-Process | Out-File x\"",
  "powershell -NoProfile -Command \"Get-Process & calc\"",
  "powershell -NoProfile -Command \"Get-Process -Module\"",
  "powershell -NoProfile -Command \"Get-Process | Select-Object @{n='x';e={calc}}\"",
  "powershell -NoProfile -EncodedCommand AAAA",
  // Loops that are not over literals, or not the whole line.
  "for i in $(ls); do cat $i; done",
  "for i in *; do rm $i; done",
  "for i in 1 2; do rm -rf ~; done",
  "for ((i=0;i<3;i++)); do ls; done",
  "while true; do ls; done",
  "ls; for i in 1; do ls; done",
  "for path in a b; do ls; done",
  "for i in 1 2; do ls; donex",
  "for i in a
do rm -rf /c/Users/x
done
for j in b; do echo; done",
  "for i in a
do ls
done
for j in b; do echo; done",
  "for i in a; do ls; done; for j in b; do echo; done",
  "for i in a; do for j in b; do ls; done; done",
  "for i in a;; do ls; done",
  "for i in a; do ls;; done",
  "for i in a do ls; done",
  // Inline code that loads from elsewhere, or after a part that changes files.
  "python -m http.server",
  "node --require x -e 1",
  "C:/evil/python -c 'print(1)'",
  "echo x > a.py; python -c 'import a'",
  "python -c 'print(1)' | tee ../x",
  "bash -c 'rm -rf ~'",
];

#[test]
fn quick_rules_run_code_of_the_worktree_in_a_step_that_edits_files() {
  let fixture = Fixture::new();
  for line in EDITING {
    assert!(fixture.runs(line, RepositoryAccess::Write), "`{line}` runs");
    assert!(
      !fixture.runs(line, RepositoryAccess::ReadOnly),
      "`{line}` asks in a read-only step"
    );
  }
}

#[test]
fn quick_rules_run_process_listings_in_every_step() {
  let fixture = Fixture::new();
  for line in LISTINGS {
    for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
      assert!(fixture.runs(line, repository), "`{line}` runs");
    }
  }
}

/// The benign prefixes of rule 1, each in front of a shell, an interpreter or a wrapper given in a way no
/// rule for a bare program covers.
const PREFIXED: &[&str] = &[
  "{VAR}=1 /any/dir/bash -c ls",
  "{VAR}=1 ../x/bash -c ls",
  "{VAR}=1 C:/evil/sh.exe -c ls",
  "{VAR}=1 /any/dir/pwsh -NoProfile -Command 'Get-Process'",
  "{VAR}=1 ../x/python .tmp/verify/orc50b/drive.py",
  "{VAR}=1 C:/evil/python -c 1",
  "{VAR}=1 out/x.exe",
  "{VAR}=1 env ls",
  "{VAR}=1 nohup ls",
  "{VAR}=1 timeout 5 ls",
  "{VAR}=1 PATH=x ls",
];

const BENIGN: [&str; 11] = [
  "PYTHONIOENCODING",
  "PYTHONUTF8",
  "PYTHONUNBUFFERED",
  "NO_COLOR",
  "FORCE_COLOR",
  "TERM",
  "LANG",
  "LC_ALL",
  "RUST_LOG",
  "RUST_BACKTRACE",
  "CARGO_TERM_COLOR",
];

#[test]
fn quick_rules_check_the_program_after_the_prefixes() {
  let fixture = Fixture::new();
  for var in BENIGN {
    for line in PREFIXED {
      let line = line.replace("{VAR}", var);
      for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
        assert!(!fixture.runs(&line, repository), "`{line}` must ask");
      }
    }
    let line = format!("{var}=1 ls");
    assert!(fixture.runs(&line, RepositoryAccess::ReadOnly), "`{line}` runs");
  }
}

#[test]
fn quick_rules_read_a_loop_with_its_body_on_lines_of_its_own() {
  let fixture = Fixture::new();
  for line in [
    "for i in 1 2
do ls
done",
    "for i in 1 2; do
  ls
done",
  ] {
    assert!(fixture.runs(line, RepositoryAccess::ReadOnly), "`{line}` runs");
  }
}

#[test]
fn quick_rules_ask_for_what_reaches_further() {
  let fixture = Fixture::new();
  for line in ASKS {
    for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
      assert!(!fixture.runs(line, repository), "`{line}` must ask");
    }
  }
}
