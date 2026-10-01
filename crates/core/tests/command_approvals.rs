use orchester_core::{
  connection::{
    AgentProvider,
    app_server::{AppServerStream, ProtocolError, Update},
    invocation::Platform,
  },
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, OutsideCommands,
    RepositoryAccess, StepPermissions,
    capability::{Mechanism, PermissionSetting},
  },
};
use serde_json::{Value, json};

#[test]
fn command_approvals_reply_once_and_reject_sticky_grants() {
  let mut stream = AppServerStream::default();
  let mut message = json!({"id":"request-1","method":"item/commandExecution/requestApproval","params":{
    "threadId":"thread-1","turnId":"turn-1","itemId":"item-1",
    "command":"echo harmless","cwd":"/worktree","environmentId":"local",
    "availableDecisions":["accept","decline","acceptForSession"]
  }});
  let updates = stream.parse(&message.to_string()).expect("request");
  let [Update::Approval(request)] = updates.as_slice() else {
    panic!("approval expected")
  };
  assert!(request.is_command());
  assert_eq!(request.action, "echo harmless");
  assert_eq!(request.target.as_deref(), Some("/worktree"));
  for (approve, decision) in [(true, "accept"), (false, "decline")] {
    assert_eq!(
      serde_json::from_str::<Value>(&request.response(approve)).expect("response"),
      json!({"id":"request-1","result":{"decision":decision}})
    );
  }
  assert!(!format!("{request:?}").contains("echo harmless"));
  message["params"]["environmentId"] = json!("remote");
  assert_eq!(
    stream.parse(&message.to_string()).err(),
    Some(ProtocolError::InvalidApproval)
  );
  message["params"]["environmentId"] = json!("local");
  message["params"]["additionalPermissions"] = json!({"network":{"enabled":true}});
  let refused = stream.parse(&message.to_string()).err();
  assert_eq!(refused, Some(ProtocolError::AdditionalPermissions));
  // The step's activity says why it stopped.
  assert!(
    refused
      .expect("refused")
      .to_string()
      .contains("with_additional_permissions")
  );
  message["params"]["additionalPermissions"] = Value::Null;
  message["params"]["availableDecisions"] = json!(["acceptForSession"]);
  assert_eq!(
    stream.parse(&message.to_string()).err(),
    Some(ProtocolError::InvalidApproval)
  );
}

#[test]
fn command_approvals_native_cancel_and_legacy_decline_remain_action_scoped() {
  // Native Codex 0.159.2 rule-based requests advertise accept/cancel, not decline.
  let request = json!({"id":7,"method":"item/commandExecution/requestApproval","params":{
    "threadId":"thread-1","turnId":"turn-1","itemId":"item-1","startedAtMs":1,
    "kind":"command","environmentId":"local","command":"echo harmless","cwd":"/worktree",
    "availableDecisions":["accept","cancel"]
  }});
  for (decisions, denial) in [
    (json!(["accept", "cancel"]), "cancel"),
    (json!(["accept", "decline", "cancel", "acceptForSession"]), "decline"),
    (Value::Null, "decline"),
  ] {
    let mut message = request.clone();
    message["params"]["availableDecisions"] = decisions;
    let updates = AppServerStream::default().parse(&message.to_string()).expect("request");
    let [Update::Approval(approval)] = updates.as_slice() else {
      panic!("command approval expected")
    };
    assert!(approval.is_command());
    for (approve, decision) in [(true, "accept"), (false, denial)] {
      assert_eq!(
        serde_json::from_str::<Value>(&approval.response(approve)).expect("response"),
        json!({"id":7,"result":{"decision":decision}})
      );
    }
  }
  for decisions in [
    json!(["acceptForSession", "cancel"]),
    json!(["accept"]),
    json!(["cancel"]),
    json!("accept"),
  ] {
    let mut message = request.clone();
    message["params"]["availableDecisions"] = decisions;
    assert_eq!(
      AppServerStream::default().parse(&message.to_string()).err(),
      Some(ProtocolError::InvalidApproval)
    );
  }
}

#[test]
fn command_approvals_legacy_permissions_and_mcp_policy_round_trip() {
  let mut profile = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  let legacy = serde_json::to_string(&profile).expect("serialize");
  assert!(!legacy.contains("interactive_tool_approvals"));
  profile.set_interactive_tool_approvals(true);
  let encoded = serde_json::to_string(&profile).expect("serialize");
  assert_eq!(
    serde_json::from_str::<AgentPermissions>(&encoded).expect("deserialize"),
    profile
  );
  assert!(
    profile
      .narrowed(RepositoryAccess::ReadOnly, &StepPermissions::default())
      .interactive_tool_approvals()
  );
}

#[test]
fn permissions_asking_before_commands_route_to_each_clis_approval_on_both_platforms() {
  let expected = [
    (AgentProvider::CodexCli, Mechanism::CodexCommandApprovals),
    (AgentProvider::ClaudeCodeCli, Mechanism::ClaudePermissionPrompt),
  ];
  for (provider, expected) in expected {
    for platform in [Platform::Windows, Platform::MacOs] {
      let mut ask = AgentPermissions::new(
        RepositoryAccess::ReadOnly,
        CommandPermission::AskBefore,
        NetworkAccess::On,
      );
      ask.set_interactive_tool_approvals(true);
      // Agents ask: a read-only Claude Code step asks about every command too, and its launch
      // permissions keep asking before commands.
      let enforced = ask.enforced_on(provider, platform).expect("supported");
      assert_eq!(enforced.mechanism(PermissionSetting::Commands), expected);
      assert_eq!(*enforced.permissions().commands(), CommandPermission::AskBefore);
    }
  }
}

/// A command that needs more than the sandbox (`require_escalated`) reaches a person as an ordinary
/// command approval, its justification shown as the reason; approved, only that call runs.
#[test]
fn command_approvals_ask_about_an_escalation_once() {
  let message = json!({"id":200,"method":"item/commandExecution/requestApproval","params":{
    "threadId":"thread-1","turnId":"turn-1","itemId":"item-1","startedAtMs":1,"kind":"command",
    "environmentId":"local","command":"pnpm install","cwd":"/worktree",
    "reason":"Do you want to reach the package registry?",
    "proposedExecpolicyAmendment":["pnpm","install"],
    "availableDecisions":["accept","acceptForSession",{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["pnpm","install"]}},"decline"]
  }});
  let updates = AppServerStream::default().parse(&message.to_string()).expect("request");
  let [Update::Approval(request)] = updates.as_slice() else {
    panic!("an escalation asks a person")
  };
  assert!(request.is_command());
  assert_eq!(request.action, "pnpm install");
  assert_eq!(
    request.scope.as_deref(),
    Some("Do you want to reach the package registry?")
  );
  for (approve, decision) in [(true, "accept"), (false, "decline")] {
    assert_eq!(
      serde_json::from_str::<Value>(&request.response(approve)).expect("response"),
      json!({"id":200,"result":{"decision":decision}}),
      "never a session grant or a rule amendment"
    );
  }
}

/// Edits keep their rules: a file change outside the writable roots is declined at once, as is a
/// host of Codex's managed network proxy, whose grant may outlast the one command.
#[test]
fn command_approvals_decline_file_changes_and_network_hosts_without_a_person() {
  let patch = json!({"id":300,"method":"item/fileChange/requestApproval","params":{
    "threadId":"thread-1","turnId":"turn-1","itemId":"patch-1","startedAtMs":1,"grantRoot":"/elsewhere"
  }});
  let host = json!({"id":"n1","method":"item/commandExecution/requestApproval","params":{
    "threadId":"thread-1","turnId":"turn-1","itemId":"item-2","startedAtMs":1,"command":"curl example.com",
    "cwd":"/worktree","networkApprovalContext":{"host":"example.com","protocol":"https"},
    "availableDecisions":["accept","decline"]
  }});
  for (message, id) in [(patch, json!(300)), (host, json!("n1"))] {
    let updates = AppServerStream::default().parse(&message.to_string()).expect("request");
    let [Update::Declined(response)] = updates.as_slice() else {
      panic!("declined at once")
    };
    assert_eq!(
      serde_json::from_str::<Value>(response).expect("response"),
      json!({"id":id,"result":{"decision":"decline"}})
    );
  }
}

/// How a person is asked about a command the permissions do not run by themselves, as each launch
/// records it: Codex through its sandbox approval, Claude Code through its permission prompt, whatever
/// the repository access and the commands.
#[test]
fn permissions_record_how_other_commands_ask_a_person() {
  let listed =
    CommandPermission::Allowed(AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("list"));
  let none = CommandPermission::Allowed(AllowedCommands::default());
  for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
    let cases = [
      (
        AgentProvider::CodexCli,
        CommandPermission::AnyInWorktree,
        OutsideCommands::SandboxApproval,
      ),
      (
        AgentProvider::CodexCli,
        CommandPermission::AskBefore,
        OutsideCommands::SandboxApproval,
      ),
      (
        AgentProvider::ClaudeCodeCli,
        listed.clone(),
        OutsideCommands::PermissionPrompt,
      ),
      (
        AgentProvider::ClaudeCodeCli,
        none.clone(),
        OutsideCommands::PermissionPrompt,
      ),
      (
        AgentProvider::ClaudeCodeCli,
        CommandPermission::AskBefore,
        OutsideCommands::PermissionPrompt,
      ),
    ];
    for (provider, commands, outside) in cases {
      let enforced = AgentPermissions::new(repository, commands.clone(), NetworkAccess::On)
        .enforced_on(provider, Platform::Windows)
        .expect("enforceable");
      assert_eq!(
        enforced.outside_commands(),
        outside,
        "{provider:?} {repository:?} {commands:?}"
      );
      assert_eq!(enforced.launch().outside_commands(), Some(outside));
      assert_eq!(enforced.launch().permissions(), enforced.permissions());
    }
  }
}
