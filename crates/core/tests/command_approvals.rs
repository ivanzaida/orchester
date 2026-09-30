use orchester_core::{
  connection::{
    AgentProvider,
    app_server::{AppServerStream, ProtocolError, Update},
    invocation::Platform,
  },
  permission::{
    AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess, StepPermissions,
    capability::{Mechanism, PermissionOption, PermissionProblem, PermissionSetting, Unavailable},
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
fn permissions_asking_before_commands_requires_codex_on_both_platforms() {
  for provider in AgentProvider::ALL {
    for platform in [Platform::Windows, Platform::MacOs] {
      let ask = AgentPermissions::new(
        RepositoryAccess::ReadOnly,
        CommandPermission::AskBefore,
        NetworkAccess::On,
      );
      if provider == AgentProvider::CodexCli {
        assert_eq!(
          ask
            .enforced_on(provider, platform)
            .expect("supported")
            .mechanism(PermissionSetting::Commands),
          Mechanism::CodexCommandApprovals
        );
        continue;
      }
      assert_eq!(
        ask.enforced_on(provider, platform),
        Err(PermissionProblem::NotEnforceable {
          option: PermissionOption::AskBeforeCommands,
          reason: Unavailable::NeedsPermissionRequests,
        })
      );
    }
  }
}
