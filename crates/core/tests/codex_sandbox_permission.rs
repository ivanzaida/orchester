//! *Codex sandbox* (ORC-789): on by default and left out of the stored form while on; turning it off widens; with it
//! off Codex enforces neither repository access, network, writable folders nor where any command runs, while asking
//! before commands still asks; another CLI ignores it, and fitting a profile to one turns it on again; a step keeps
//! the profile's; and Codex starts without the sandbox's launch settings, its shell kept. Run with
//! `cargo test -p orchester-core --test codex_sandbox_permission codex_sandbox`.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{
    AgentProvider,
    invocation::{CodexAgentFiles, CodexInvocation, Platform},
  },
  permission::{
    AgentPermissions, CodexSandbox, CommandPermission, FolderBounds, NetworkAccess, RepositoryAccess, StepPermissions,
    WritableFolder, WritableFolders,
    capability::{Mechanism, PermissionSetting},
    widening::Widening,
  },
};

/// Write access with network off, `commands` and one extra writable folder.
fn permissions(commands: CommandPermission, sandbox: CodexSandbox) -> AgentPermissions {
  let repository = tempfile::tempdir().expect("a repository folder");
  let bounds = FolderBounds::new(repository.path(), None).expect("bounds");
  let folder = WritableFolder::new(".orchester/target", &bounds).expect("a folder");
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off)
    .with_writable_folders(WritableFolders::new(vec![folder]).expect("unique"));
  permissions.set_codex_sandbox(sandbox);
  permissions
}

fn off() -> AgentPermissions {
  permissions(CommandPermission::AnyInWorktree, CodexSandbox::Off)
}

fn on() -> AgentPermissions {
  permissions(CommandPermission::AnyInWorktree, CodexSandbox::On)
}

#[test]
fn codex_sandbox_is_on_by_default_and_stored_only_while_off() {
  let fresh = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  );
  assert_eq!(fresh.codex_sandbox(), CodexSandbox::On);
  let stored = serde_json::to_value(on()).expect("JSON");
  assert!(stored.get("codex_sandbox").is_none(), "as before it existed: {stored}");
  let stored = serde_json::to_value(off()).expect("JSON");
  assert_eq!(stored["codex_sandbox"], "off");
  let read: AgentPermissions = serde_json::from_value(stored).expect("read back");
  assert_eq!(read, off());
}

#[test]
fn codex_sandbox_turned_off_widens_and_turned_on_narrows() {
  assert_eq!(off().widenings_from(&on()), [Widening::CodexSandboxOff]);
  assert!(on().widenings_from(&off()).is_empty());
  assert!(off().widenings_from(&off()).is_empty(), "keeping it off is no change");
}

#[test]
fn codex_sandbox_off_enforces_nothing_the_sandbox_did() {
  for platform in [Platform::Windows, Platform::MacOs] {
    let enforced = off()
      .enforced_on(AgentProvider::CodexCli, platform)
      .expect("a person's choice runs");
    for setting in [
      PermissionSetting::Repository,
      PermissionSetting::Commands,
      PermissionSetting::Network,
    ] {
      assert_eq!(enforced.mechanism(setting), Mechanism::CodexSandboxOff, "{setting:?}");
    }
    assert_eq!(enforced.writable_folders(), Some(Mechanism::CodexSandboxOff));
    assert!(!enforced.enforces_every_setting());
    assert!(!Mechanism::CodexSandboxOff.enforces());
    let sandboxed = on().enforced_on(AgentProvider::CodexCli, platform).expect("enforced");
    assert!(sandboxed.enforces_every_setting(), "{platform:?}");
    assert_eq!(
      sandboxed.mechanism(PermissionSetting::Repository),
      Mechanism::CodexWorkspaceWriteSandbox
    );
  }
}

#[test]
fn codex_sandbox_off_still_asks_before_commands() {
  let asking = permissions(CommandPermission::AskBefore, CodexSandbox::Off)
    .enforced_on(AgentProvider::CodexCli, Platform::current())
    .expect("enforced");
  assert_eq!(
    asking.mechanism(PermissionSetting::Commands),
    Mechanism::CodexCommandApprovals
  );
  assert_eq!(
    asking.mechanism(PermissionSetting::Repository),
    Mechanism::CodexSandboxOff
  );
}

#[test]
fn codex_sandbox_means_nothing_to_claude_code_and_fitting_turns_it_on() {
  let listed = permissions(CommandPermission::Allowed(Default::default()), CodexSandbox::Off);
  let claude = listed
    .enforced_on(AgentProvider::ClaudeCodeCli, Platform::current())
    .expect("enforced");
  assert_eq!(
    claude.mechanism(PermissionSetting::Repository),
    Mechanism::ClaudeAcceptEdits
  );
  assert!(claude.enforces_every_setting());
  let (fitted, resets) = off().fitted_to(AgentProvider::ClaudeCodeCli, Platform::current());
  assert_eq!(fitted.codex_sandbox(), CodexSandbox::On);
  assert!(!resets.is_empty(), "any command is reset too");
  let (kept, _) = off().fitted_to(AgentProvider::CodexCli, Platform::current());
  assert_eq!(kept.codex_sandbox(), CodexSandbox::Off);
}

#[test]
fn codex_sandbox_of_the_profile_holds_for_its_steps() {
  let narrowed = off().narrowed(RepositoryAccess::ReadOnly, &StepPermissions::default());
  assert_eq!(narrowed.codex_sandbox(), CodexSandbox::Off);
}

fn codex_args(sandbox: CodexSandbox, platform: Platform) -> Vec<String> {
  let roots = [PathBuf::from("/repo/.orchester/target")];
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    repository: Path::new("/repo"),
    access: RepositoryAccess::Write,
    network: NetworkAccess::On,
    writable_roots: &roots,
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform,
    mcp: None,
    agent_files: CodexAgentFiles::Loaded,
    sandbox,
  }
  .args()
  .expect("unicode paths")
}

#[test]
fn codex_sandbox_off_leaves_out_the_sandbox_settings_and_keeps_the_shell() {
  for platform in [Platform::Windows, Platform::MacOs] {
    let off = codex_args(CodexSandbox::Off, platform).join(" ");
    assert!(
      !off.contains("windows.sandbox") && !off.contains("sandbox_workspace_write"),
      "{off}"
    );
    assert!(
      off.contains("features.hooks=false") && off.contains("trust_level=\"untrusted\""),
      "{off}"
    );
    assert!(!off.contains("features.shell_tool"), "commands run: {off}");
    let on = codex_args(CodexSandbox::On, platform).join(" ");
    assert!(on.contains("sandbox_workspace_write.network_access=true"), "{on}");
    assert!(on.contains("sandbox_workspace_write.writable_roots="), "{on}");
    assert_eq!(
      platform == Platform::Windows,
      on.contains("windows.sandbox=elevated"),
      "{on}"
    );
  }
}
