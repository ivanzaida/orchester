//! The code index folder Claude Code reads: given with `--add-dir`, never editable.

use std::path::Path;

use orchester_core::{
  connection::invocation::{ClaudeInvocation, Platform},
  permission::{NetworkAccess, RepositoryAccess},
};

use crate::{allowed, args, enforced, invocation};

#[test]
fn claude_invocation_reads_the_code_index_but_may_not_edit_it() {
  let index = [
    (
      Path::new("/repo/.orchester/index/4d2b8e0"),
      "Edit(//repo/.orchester/index/4d2b8e0/**)",
    ),
    (
      Path::new(r"C:\repo\.orchester\index\4d2b8e0"),
      "Edit(//c/repo/.orchester/index/4d2b8e0/**)",
    ),
  ];
  for (folder, rule) in index {
    let writing = enforced(
      RepositoryAccess::Write,
      allowed(&["git"]),
      NetworkAccess::On,
      Platform::Windows,
    );
    let given = ClaudeInvocation {
      code_index: Some(folder),
      ..invocation(&writing)
    };
    let given_args = args(&given);
    let added = given_args
      .windows(2)
      .any(|pair| pair[0] == "--add-dir" && pair[1] == folder.to_string_lossy());
    assert!(added, "{given_args:?}");
    assert_eq!(
      given_args
        .iter()
        .filter(|arg| arg.as_str() == "--disallowedTools")
        .count(),
      1,
      "{given_args:?}"
    );
    assert!(given_args.iter().any(|arg| arg == rule), "{given_args:?}");

    let reading = enforced(
      RepositoryAccess::ReadOnly,
      allowed(&["git"]),
      NetworkAccess::On,
      Platform::Windows,
    );
    let read_only = ClaudeInvocation {
      code_index: Some(folder),
      ..invocation(&reading)
    };
    assert!(
      !args(&read_only).iter().any(|arg| arg.starts_with("Edit(")),
      "every edit tool is denied already"
    );
  }
}

#[test]
fn claude_invocation_gets_no_edit_rule_for_a_code_index_on_a_network_share() {
  // How Claude Code writes a network path in its patterns is not documented: the index's files
  // are read-only instead.
  for folder in [
    r"\\server\share\repo\.orchester\index\4d2b8e0",
    r"\\?\UNC\server\share\repo\.orchester\index\4d2b8e0",
  ] {
    let writing = enforced(
      RepositoryAccess::Write,
      allowed(&[]),
      NetworkAccess::On,
      Platform::Windows,
    );
    let given = ClaudeInvocation {
      code_index: Some(Path::new(folder)),
      ..invocation(&writing)
    };
    let given_args = args(&given);
    assert!(!given_args.iter().any(|arg| arg.starts_with("Edit(")), "{given_args:?}");
  }
}
