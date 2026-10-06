//! Script files and nested fixture repositories in the worktree (the user's decision of 2026-10-06): in a
//! step that edits files, an interpreter running a regular file of the worktree, and git in a repository
//! nested in it with only who a commit names set, run without a person, as a written test run by an
//! allowed `cargo test` would. Inline code, modules, options that load code from elsewhere, a script
//! through a link, outside or in `.git`, other git settings and a read-only step still ask.
//! `cargo test -p orchester-core --test worktree_scripts worktree_scripts`.

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
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let worktree = root.path().join("repo").join("w1");
    let verify = worktree.join(".tmp").join("verify");
    let elsewhere = root.path().join("elsewhere");
    for folder in [
      &verify.join("kontur").join(".git"),
      &worktree.join(".git").join("hooks"),
      &worktree.join(".orchester"),
      &elsewhere,
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    for script in ["start.py", "launch.py", "tree.py", "start-app.ps1", "x.js", "x.sh"] {
      std::fs::write(verify.join(script), "").expect("script");
    }
    for file in [
      worktree.join(".git").join("hooks").join("x.py"),
      worktree.join(".orchester").join("x.py"),
      elsewhere.join("x.py"),
      root.path().join("repo").join("outside.py"),
    ] {
      std::fs::write(file, "").expect("file");
    }
    link_folder(&worktree.join("out"), &elsewhere);
    let home = root.path().join("home");
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

/// A link to the folder `target` at `link`: a symbolic link on Unix, a junction on Windows, which needs
/// no privilege there and which Windows follows like a link.
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

/// The lines of Verify steps that asked a person.
const REAL: &[&str] = &[
  "python .tmp/verify/start.py",
  "python .tmp/verify/launch.py",
  "python .tmp/verify/tree.py 39684 tasks-new-task",
  "python .tmp/verify/tree.py 39684 task-title task-brief criterion-0 criterion-1",
  "powershell -NoProfile -ExecutionPolicy Bypass -File .tmp/verify/start-app.ps1",
  "git -C .tmp/verify/kontur -c user.name=verifier -c user.email=verifier@example.invalid commit -q -m 'fixture'",
];

const RUNS: &[&str] = &[
  "python3 -X importtime .tmp/verify/start.py",
  "py -3 .tmp/verify/start.py",
  "node .tmp/verify/x.js --port 3000",
  "bash .tmp/verify/x.sh",
  "sh -eu .tmp/verify/x.sh",
  "pwsh -NoProfile -NonInteractive -File .tmp/verify/start-app.ps1 -Port 1",
  "python .tmp/verify/tree.py 1 2>&1 | grep -E 'ok' | head -5",
  "git -C .tmp/verify/kontur add -A",
];

const ASKS: &[&str] = &[
  // Inline code, modules and code loaded from elsewhere.
  "python -c 'print(1)'",
  "node -e 1",
  "bash -c 'rm -rf ~'",
  "python -m http.server",
  "python -m pip install x",
  "PYTHONPATH=x python .tmp/verify/start.py",
  "NODE_OPTIONS=--require=x node .tmp/verify/x.js",
  "node -r ./x .tmp/verify/x.js",
  "bash --rcfile x .tmp/verify/x.sh",
  "sh -s < .tmp/verify/x.sh",
  "pwsh -Command 'Remove-Item x'",
  "pwsh -EncodedCommand AAAA",
  "powershell -ExecutionPolicy Unrestricted -File .tmp/verify/start-app.ps1",
  "powershell -NoProfile -File",
  // Scripts that are no regular file of the worktree, or arguments that reach out.
  "python out/x.py",
  "python ../outside.py",
  "python .git/hooks/x.py",
  "python .orchester/x.py",
  "python .tmp/verify/missing.py",
  "python .tmp/verify/*.py",
  "python .tmp/verify",
  "python .tmp/verify/start.py $(id)",
  "python .tmp/verify/start.py > ../x",
  "python .tmp/verify/start.py | tee ../x",
  "python .tmp/verify/start.py ../outside.py",
  // git settings that can start programs, and repositories that are not nested fixtures.
  "git -C .tmp/verify/kontur -c core.hooksPath=x commit -q -m m",
  "git -C .tmp/verify/kontur -c core.sshCommand=x commit -q -m m",
  "git -C .tmp/verify/kontur -c alias.x=!sh x",
  "git -C .tmp/verify/kontur -c credential.helper=x commit -q -m m",
  "git -C .tmp/verify/kontur -c protocol.allow=always commit -q -m m",
  "git -C .tmp/verify/kontur -c filter.x.clean=sh commit -q -m m",
  "git -C .tmp/verify/kontur push",
  "git -C .tmp/verify/kontur reset --hard",
  "git -C .. commit -m m",
  "git -C out commit -m m",
  "git -C . commit -m m",
  "git -C .tmp/verify commit -m m",
  "git commit -m m",
];

#[test]
fn worktree_scripts_run_in_a_step_that_edits_files() {
  let fixture = Fixture::new();
  for line in REAL.iter().chain(RUNS) {
    assert!(fixture.runs(line, RepositoryAccess::Write), "`{line}` runs");
  }
}

#[test]
fn worktree_scripts_ask_for_code_from_elsewhere() {
  let fixture = Fixture::new();
  for line in ASKS {
    assert!(!fixture.runs(line, RepositoryAccess::Write), "`{line}` must ask");
  }
}

#[test]
fn worktree_scripts_ask_in_a_read_only_step() {
  let fixture = Fixture::new();
  for line in REAL.iter().chain(RUNS).chain(ASKS) {
    assert!(!fixture.runs(line, RepositoryAccess::ReadOnly), "`{line}` must ask");
  }
}
