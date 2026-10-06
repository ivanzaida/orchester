//! A native agent's command is judged in the grammar of what runs it: a PowerShell command is never
//! read as Bash (which drops a backslash, so a Windows path outside would look like a file in the
//! worktree); only the strict read-only PowerShell rule lets one run without a person, and everything
//! else counts as Unknown, which neither the step's permissions nor auto-approval answer. A zsh command
//! on macOS keeps the Bash rules.
//! `cargo test -p orchester-core --test native_command_grammar native_grammar`.

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{CommandCategory, CommandScope},
  },
  run::{
    ApprovalSubject, AutoAsk, AutoVerdict, CommandShell, NativeCommandAsk, PendingToolApproval, auto_verdict, permitted,
  },
  time::Timestamp,
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

  /// The subject of a native agent's request to run `command` through `shell` (`None`: no shell) in the
  /// worktree.
  fn subject(&self, command: &str, shell: Option<CommandShell>) -> ApprovalSubject {
    let approval = PendingToolApproval {
      id: "i1:call-1".to_owned(),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation: InvocationId::generate(),
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: "command".to_owned(),
      action: command.to_owned(),
      target: Some(self.scope.worktree().to_string_lossy().into_owned()),
      requested_at: Timestamp::from_unix_millis(1_000),
      native: Some(NativeCommandAsk {
        shell,
        time_limit_seconds: 120,
      }),
    };
    ApprovalSubject::of(&approval, &self.scope)
  }

  /// Whether the step's permissions (`cargo test` and `cat` listed, the repository writable) run it.
  fn runs(&self, command: &str, shell: Option<CommandShell>) -> bool {
    let list = ["cargo test", "cat"].map(|entry| CommandPrefix::new(entry).expect("prefix"));
    let permissions = AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(AllowedCommands::new(list.to_vec()).expect("list")),
      NetworkAccess::Off,
    );
    permitted(&self.subject(command, shell), &permissions).is_some()
  }
}

#[test]
fn native_grammar_powershell_keeps_backslashes_and_asks_for_a_path_outside() {
  let fixture = Fixture::new();
  let command = r"cat ..\..\home\.ssh\id_rsa";
  for shell in [CommandShell::Pwsh, CommandShell::WindowsPowerShell] {
    assert!(!fixture.runs(command, Some(shell)), "{shell:?}");
    assert!(
      matches!(
        auto_verdict(&fixture.subject(command, Some(shell))),
        AutoVerdict::Ask(_)
      ),
      "{shell:?}"
    );
  }
}

#[test]
fn native_grammar_powershell_runs_only_the_strict_read_only_rule() {
  let fixture = Fixture::new();
  for command in [
    "Get-Process",
    "Get-Process | Select-Object Id,ProcessName",
    "Get-Content src/a.txt",
  ] {
    assert!(fixture.runs(command, Some(CommandShell::Pwsh)), "{command}");
  }
  // Listed, and a write in the worktree, by Bash's rules; PowerShell's grammar is not read that far.
  for command in ["cargo test", "Set-Content src/a.txt b", "cargo test; Get-Process"] {
    assert!(!fixture.runs(command, Some(CommandShell::Pwsh)), "{command}");
    let ApprovalSubject::Command(classified) = fixture.subject(command, Some(CommandShell::Pwsh)) else {
      panic!("a command");
    };
    assert_eq!(classified.category(), CommandCategory::Unknown, "{command}");
    assert_eq!(
      auto_verdict(&ApprovalSubject::Command(classified)),
      AutoVerdict::Ask(AutoAsk::Unknown),
      "{command}"
    );
  }
}

#[test]
fn native_grammar_zsh_keeps_the_bash_rules() {
  let fixture = Fixture::new();
  assert!(fixture.runs("cat src/a.txt", Some(CommandShell::Zsh)));
  assert!(fixture.runs("cargo test", Some(CommandShell::Zsh)));
  assert!(!fixture.runs("cat ../outside.txt", Some(CommandShell::Zsh)));
  assert!(!fixture.runs("cat ~/.ssh/id_rsa", Some(CommandShell::Zsh)));
}

#[test]
fn native_grammar_direct_command_reads_its_arguments_as_split() {
  let fixture = Fixture::new();
  assert!(fixture.runs("cat src/a.txt", None));
  assert!(!fixture.runs("cat ../outside.txt", None));
}

/// Without a shell a Windows program gets a backslash as it is, a folder separator there.
#[cfg(windows)]
#[test]
fn native_grammar_direct_command_keeps_windows_backslashes() {
  let fixture = Fixture::new();
  assert!(!fixture.runs(r"cat ..\..\home\.ssh\id_rsa", None));
  assert!(!fixture.runs(r"cat ..\outside.txt", None));
}
