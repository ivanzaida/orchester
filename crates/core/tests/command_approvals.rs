use orchester_core::{
  connection::{
    AgentProvider,
    app_server::{AppServerStream, ProtocolError, Update},
    invocation::Platform,
  },
  permission::{
    AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess, StepPermissions,
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
  assert_eq!(
    stream.parse(&message.to_string()).err(),
    Some(ProtocolError::InvalidApproval)
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
  for provider in AgentProvider::ALL {
    for platform in [Platform::Windows, Platform::MacOs] {
      let mut ask = AgentPermissions::new(
        RepositoryAccess::ReadOnly,
        CommandPermission::AskBefore,
        NetworkAccess::On,
      );
      ask.set_interactive_tool_approvals(true);
      // Claude Code would run an approved command unsandboxed, so a read-only step gets none.
      let expected = match provider {
        AgentProvider::CodexCli => Mechanism::CodexCommandApprovals,
        AgentProvider::ClaudeCodeCli => Mechanism::ClaudeNoCommandsReadOnly,
      };
      assert_eq!(
        ask
          .enforced_on(provider, platform)
          .expect("supported")
          .mechanism(PermissionSetting::Commands),
        expected
      );
      ask.set_repository(RepositoryAccess::Write);
      let writing = ask.enforced_on(provider, platform).expect("supported");
      assert_ne!(
        writing.mechanism(PermissionSetting::Commands),
        Mechanism::ClaudeNoCommandsReadOnly
      );
    }
  }
}
