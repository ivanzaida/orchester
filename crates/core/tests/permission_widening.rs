//! What widens what agents may do: profile permissions broader than those they replace, and any change to
//! a project environment variable but the few harmless ones.

use orchester_core::permission::{
  AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, FolderBounds, NetworkAccess, RepositoryAccess,
  WritableFolder, WritableFolders,
  widening::{Widening, harmless_variable},
};

fn allowed(commands: &[&str]) -> CommandPermission {
  let prefixes = commands
    .iter()
    .map(|command| CommandPrefix::new(command).expect("a command prefix"))
    .collect();
  CommandPermission::Allowed(AllowedCommands::new(prefixes).expect("an allowed list"))
}

fn permissions(repository: RepositoryAccess, commands: CommandPermission, network: NetworkAccess) -> AgentPermissions {
  AgentPermissions::new(repository, commands, network)
}

#[test]
fn widening_the_same_or_narrower_permissions_is_nothing() {
  let base = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  assert!(base.widenings_from(&base).is_empty());
  let narrower = permissions(RepositoryAccess::ReadOnly, allowed(&["cargo test"]), NetworkAccess::Off);
  assert!(narrower.widenings_from(&base).is_empty());
  let fewer = permissions(RepositoryAccess::ReadOnly, allowed(&["cargo test"]), NetworkAccess::Off);
  let more = permissions(
    RepositoryAccess::ReadOnly,
    allowed(&["cargo test", "cargo fmt"]),
    NetworkAccess::Off,
  );
  assert!(
    fewer.widenings_from(&more).is_empty(),
    "dropping an allowed command narrows"
  );
}

#[test]
fn widening_names_every_broader_setting() {
  let base = permissions(RepositoryAccess::ReadOnly, allowed(&["cargo test"]), NetworkAccess::Off);
  let mut wider = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  wider.set_auto_approve(true);
  assert_eq!(
    wider.widenings_from(&base),
    [
      Widening::Repository,
      Widening::Commands,
      Widening::Network,
      Widening::AutoApprove
    ]
  );
  let added = permissions(
    RepositoryAccess::ReadOnly,
    allowed(&["cargo test", "curl"]),
    NetworkAccess::Off,
  );
  assert_eq!(added.widenings_from(&base), [Widening::Commands]);
  let asking = permissions(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  );
  assert_eq!(
    asking.widenings_from(&base),
    [Widening::Commands],
    "asking is not an allowed list"
  );
  let empty = permissions(RepositoryAccess::ReadOnly, allowed(&[]), NetworkAccess::Off);
  assert!(
    empty.widenings_from(&asking).is_empty(),
    "an empty list runs nothing unasked"
  );
  let mut asked = base.clone();
  asked.set_interactive_tool_approvals(true);
  assert_eq!(base.widenings_from(&asked), [Widening::ToolCallsUnasked]);
  let mut auto = base;
  auto.set_auto_approve(true);
  assert!(
    auto.widenings_from(&auto).is_empty(),
    "keeping auto-approval is no change"
  );
}

#[test]
fn widening_counts_a_writable_folder_not_granted_before() {
  let repo = tempfile::tempdir().expect("a repository");
  let outside = tempfile::tempdir().expect("a folder outside it");
  let bounds = FolderBounds::new(repo.path(), None).expect("bounds");
  let folder = WritableFolder::new(&outside.path().display().to_string(), &bounds).expect("a writable folder");
  let folders = WritableFolders::new(vec![folder]).expect("folders");
  let base = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  let with_folder = base.clone().with_writable_folders(folders.clone());
  assert_eq!(with_folder.widenings_from(&base), [Widening::WritableFolder]);
  assert!(base.widenings_from(&with_folder).is_empty());
  let read_only = permissions(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  )
  .with_writable_folders(folders);
  assert!(
    read_only.widenings_from(&base).is_empty(),
    "folders apply with write access only"
  );
}

#[test]
fn widening_takes_only_a_short_list_of_harmless_variables() {
  for name in [
    "RUST_LOG",
    "rust_backtrace",
    "CI",
    "NO_COLOR",
    "FORCE_COLOR",
    "TZ",
    "LANG",
    "LC_ALL",
    "lc_time",
    "LC_CTYPE",
  ] {
    assert!(harmless_variable(name), "{name} changes nothing agents may do");
  }
  for name in [
    "PATH",
    "NODE_OPTIONS",
    "OPENAI_BASE_URL",
    "HTTPS_PROXY",
    "CODEX_HOME",
    "CLAUDE_CODE_SHELL_PREFIX",
    "CLAUDE_ENV_FILE",
    "AWS_ENDPOINT_URL",
    "ANTHROPIC_API_KEY",
    "GIT_EXTERNAL_DIFF",
    "GIT_PROXY_COMMAND",
    "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER",
    "RUSTFLAGS",
    "CC",
    "DOTNET_STARTUP_HOOKS",
    "PIP_INDEX_URL",
    "EDITOR",
    "SSL_CERT_FILE",
    "NODE_TLS_REJECT_UNAUTHORIZED",
    "SYSTEMROOT",
    "TEMP",
    "DATABASE_NAME",
    "LC_EVIL",
  ] {
    assert!(!harmless_variable(name), "{name} is a person's decision");
  }
}
