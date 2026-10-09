//! What widens what agents may do: profile permissions broader than those they replace, and project
//! environment variables that change what programs load, where they connect or where tools keep their
//! configuration.

use orchester_core::permission::{
  AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, FolderBounds, NetworkAccess, RepositoryAccess,
  WritableFolder, WritableFolders,
  widening::{Widening, changes_loading},
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
  let mut auto = base.clone();
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
fn widening_variables_that_change_loading_endpoints_proxies_or_homes() {
  for name in [
    "PATH",
    "path",
    "NODE_OPTIONS",
    "OPENAI_BASE_URL",
    "ANTHROPIC_BASE_URL",
    "HTTPS_PROXY",
    "http_proxy",
    "ALL_PROXY",
    "NO_PROXY",
    "CODEX_HOME",
    "CLAUDE_CONFIG_DIR",
    "GIT_SSH_COMMAND",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_COUNT",
    "BASH_ENV",
    "ENV",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "RUBYOPT",
    "PERL5OPT",
    "ComSpec",
    "PATHEXT",
  ] {
    assert!(changes_loading(name), "{name} is a person's decision");
  }
  for name in ["RUST_LOG", "CI", "DATABASE_NAME", "FEATURE_FLAGS", "ENVIRONMENT"] {
    assert!(!changes_loading(name), "{name} changes no loading");
  }
}
