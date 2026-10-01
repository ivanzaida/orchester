//! Claude Code's permission prompt: what reaches a person, what the prompt answers, the relay's MCP
//! side, and how an invocation that asks a person starts.

use std::path::Path;

use orchester_core::{
  command::SecretVariable,
  connection::{
    AgentProvider,
    claude_approval::{
      ApprovalToken, ClaudeApprovalConfig, ClaudeApprovalScope, ClaudeAsk, PROMPT_TOOL, prompt_result, refusal,
      relay::{RelayStep, ask_result, relay_step},
    },
    invocation::{ClaudeInvocation, ClaudeSession, InvocationError, Platform},
    mcp::McpServerName,
  },
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess, claude::PermissionFolders,
  },
  resource::CommandEnvironment,
};
use serde_json::{Value, json};

fn server(name: &str) -> McpServerName {
  McpServerName::new(name).expect("valid server name")
}

fn permissions(commands: CommandPermission, tools: bool) -> AgentPermissions {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::On);
  permissions.set_interactive_tool_approvals(tools);
  permissions
}

fn scope(commands: bool, tools: bool) -> ClaudeApprovalScope {
  let commands = if commands {
    CommandPermission::AskBefore
  } else {
    CommandPermission::Allowed(AllowedCommands::default())
  };
  ClaudeApprovalScope::of(&permissions(commands, tools), &[server("docs"), server("docs__v2")])
    .expect("the permissions ask a person")
}

fn call(tool: &str, input: &Value) -> Value {
  json!({"tool_name": tool, "input": input, "tool_use_id": "toolu_1"})
}

#[test]
fn claude_approvals_route_only_what_the_permissions_leave_to_a_person() {
  let bash = call("Bash", &json!({"command": "pnpm publish", "description": "Publish"}));
  let ClaudeAsk::Person(command) = scope(true, false).classify(&bash) else {
    panic!("a command goes to a person");
  };
  assert!(command.is_command());
  assert_eq!(
    (command.call(), command.server(), command.action()),
    ("toolu_1", "command", "pnpm publish")
  );
  assert_eq!(command.scope(), Some("Publish"));
  assert_eq!(scope(false, true).classify(&bash), ClaudeAsk::Refused);

  let write = call("mcp__docs__v2__write", &json!({"filePath": "notes.md", "text": "x"}));
  let ClaudeAsk::Person(tool) = scope(false, true).classify(&write) else {
    panic!("a tool of an invocation server goes to a person");
  };
  assert!(!tool.is_command());
  assert_eq!((tool.server(), tool.action()), ("docs__v2", "write"));
  assert_eq!(tool.target(), Some("notes.md"));
  // The person sees the arguments too, cut when they are long.
  assert_eq!(tool.scope(), Some(r#"{"filePath":"notes.md","text":"x"}"#));
  let long = call("mcp__docs__write", &json!({"text": "y".repeat(2_000)}));
  let ClaudeAsk::Person(long) = scope(false, true).classify(&long) else {
    panic!("a tool of an invocation server goes to a person");
  };
  let shown = long.scope().expect("arguments");
  assert_eq!(shown.chars().count(), 601);
  assert!(shown.ends_with('…'));
  assert_eq!(scope(true, false).classify(&write), ClaudeAsk::Refused);
  assert_eq!(
    scope(true, true).classify(&call("mcp__other__write", &json!({}))),
    ClaudeAsk::Refused
  );
  for tool in ["Write", "Edit", "Read", "WebFetch", "mcp__Orchester__approve"] {
    assert_eq!(
      scope(true, true).classify(&call(tool, &json!({"file_path": "/etc/hosts"}))),
      ClaudeAsk::Refused,
      "{tool}"
    );
  }
  let anonymous = json!({"tool_name": "Bash", "input": {"command": "ls"}});
  assert_eq!(scope(true, true).classify(&anonymous), ClaudeAsk::Refused);
  let asks_nothing = permissions(CommandPermission::Allowed(AllowedCommands::default()), false);
  assert_eq!(ClaudeApprovalScope::of(&asks_nothing, &[]), None);
}

#[test]
fn claude_approvals_show_mcp_arguments_without_secrets_that_json_escapes_or_the_cut_splits() {
  let secret = |name: &str, value: &str| {
    SecretVariable::new(
      name,
      std::sync::Arc::new(orchester_core::secret::SecretValue::new(value.to_owned()).expect("secret")),
    )
  };
  let quoted = r#"to"k\en"#;
  let long = "s".repeat(40);
  let scope = scope(false, true).with_secrets(vec![secret("QUOTED", quoted), secret("LONG", &long)]);
  let shown = |input: &Value| {
    let ClaudeAsk::Person(request) = scope.classify(&call("mcp__docs__write", input)) else {
      panic!("a tool of an invocation server goes to a person");
    };
    request.scope().expect("arguments").to_owned()
  };
  let escaped = shown(&json!({"header": format!("Bearer {quoted}")}));
  assert_eq!(escaped, r#"{"header":"Bearer <redacted>"}"#);
  // The secret starts before the cut and ends after it: none of it is shown.
  let text = format!("{}{long}", "x".repeat(580));
  let cut = shown(&json!({"text": text}));
  assert!(cut.ends_with('…'), "{cut}");
  assert!(!cut.contains("ssss"), "no part of the secret remains: {cut}");
  assert!(cut.contains("<redacted>") || !cut.contains('s'), "{cut}");
}

#[test]
fn claude_approvals_answer_one_call_without_grants_or_its_input() {
  let allow: Value = serde_json::from_str(&prompt_result(true)).expect("JSON");
  assert_eq!(allow, json!({"behavior": "allow", "updatedInput": {}}));
  let deny: Value = serde_json::from_str(&prompt_result(false)).expect("JSON");
  assert_eq!(deny["behavior"], "deny");
  assert!(deny.get("updatedPermissions").is_none() && deny.get("interrupt").is_none());
  let refused: Value = serde_json::from_str(&refusal()).expect("JSON");
  assert_eq!(refused["behavior"], "deny");
}

#[test]
fn claude_approvals_relay_answers_the_handshake_and_hands_over_calls() {
  let RelayStep::Reply(init) =
    relay_step(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#)
  else {
    panic!("initialize is answered");
  };
  let init: Value = serde_json::from_str(&init).expect("JSON");
  assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
  assert_eq!(init["id"], 0);
  assert_eq!(
    relay_step(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
    RelayStep::Ignore
  );
  let RelayStep::Reply(list) = relay_step(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#) else {
    panic!("the tool list is answered");
  };
  let list: Value = serde_json::from_str(&list).expect("JSON");
  assert_eq!(list["result"]["tools"][0]["name"], "approve");
  let arguments = json!({"tool_name": "Bash", "input": {"command": "ls"}, "tool_use_id": "toolu_1"});
  let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"approve","arguments":arguments}});
  assert_eq!(
    relay_step(&request.to_string()),
    RelayStep::Ask {
      id: json!(2),
      arguments
    }
  );
  let RelayStep::Reply(unknown) = relay_step(r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#) else {
    panic!("an unknown request is answered");
  };
  assert!(unknown.contains("-32601"));
  let answer: Value = serde_json::from_str(&ask_result(&json!(2), &prompt_result(true))).expect("JSON");
  assert_eq!(answer["id"], 2);
  assert_eq!(answer["result"]["content"][0]["text"], prompt_result(true));
}

#[test]
fn claude_approvals_config_starts_the_relay_and_keeps_the_token_out_of_debug() {
  let token = ApprovalToken::generate();
  assert_eq!(token.expose().len(), 64);
  assert!(!ApprovalToken::generate().matches(token.expose()));
  assert!(token.matches(token.expose()));
  let config = ClaudeApprovalConfig::new("/bin/orchester-runner", &["--relay"], "127.0.0.1:4000", &token);
  let file: Value = serde_json::from_str(config.contents()).expect("JSON");
  let relay = &file["mcpServers"]["Orchester"];
  assert_eq!(relay["type"], "stdio");
  assert_eq!(relay["command"], "/bin/orchester-runner");
  assert_eq!(relay["args"], json!(["--relay"]));
  assert_eq!(relay["env"]["ORCHESTER_APPROVAL_ADDRESS"], "127.0.0.1:4000");
  assert_eq!(relay["env"]["ORCHESTER_APPROVAL_TOKEN"], token.expose());
  assert!(!format!("{config:?}{token:?}").contains(token.expose()));
  assert_eq!(PROMPT_TOOL, "mcp__Orchester__approve");
}

fn args(invocation: &ClaudeInvocation<'_>) -> Vec<String> {
  invocation
    .args()
    .into_iter()
    .map(|arg| arg.into_string().expect("unicode"))
    .collect()
}

#[test]
fn claude_approvals_invocation_gets_the_prompt_and_leaves_open_what_a_person_decides() {
  for platform in [Platform::Windows, Platform::MacOs] {
    let enforced = permissions(CommandPermission::AskBefore, true)
      .enforced_on(AgentProvider::ClaudeCodeCli, platform)
      .expect("enforceable");
    let servers = [server("docs")];
    let mut invocation = ClaudeInvocation {
      executable: Path::new("/bin/claude"),
      model: "claude-opus-5-5",
      effort: None,
      permissions: &enforced,
      writable_roots: &[],
      worktree: Path::new("/repo/.orchester/worktrees/run"),
      session: ClaudeSession::Resume("s"),
      mcp: Some((Path::new("/tmp/servers.json"), &servers)),
      skills: None,
      code_index: None,
      approvals: Some(Path::new("/tmp/approvals.json")),
    };
    let args = args(&invocation).join(" ");
    assert!(
      args.contains(
        "--mcp-config /tmp/servers.json /tmp/approvals.json --permission-prompt-tool mcp__Orchester__approve \
         --tools Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash,WebFetch,WebSearch --permission-mode acceptEdits --settings {\"permissions\":{\"ask\":[\"Bash\"]}} "
      ),
      "{args}"
    );
    assert!(!args.contains("--allowedTools"), "{args}");
    assert!(args.ends_with("--resume s"), "{args}");
    invocation.approvals = None;
    let command = invocation.command(String::new(), Vec::new(), &CommandEnvironment::default());
    assert_eq!(command.err(), Some(InvocationError::NoPermissionPrompt));
  }
}

fn permission_args_of(permissions: &AgentPermissions) -> Vec<String> {
  let folders = PermissionFolders {
    editable: &[Path::new(r"C:\repo\.orchester\worktrees\run")],
    protected: &[],
  };
  orchester_core::permission::claude::permission_args(
    &permissions
      .enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows)
      .expect("enforceable"),
    &[],
    folders,
  )
}

#[test]
fn claude_approvals_ask_before_every_command_and_never_in_read_only_steps() {
  let mut write = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  );
  let args = permission_args_of(&write);
  let settings = args.iter().position(|arg| arg == "--settings").expect("settings");
  assert_eq!(args[settings + 1], r#"{"permissions":{"ask":["Bash"]}}"#);
  assert!(args.contains(&"acceptEdits".to_owned()));
  assert!(!args.iter().any(|arg| arg.starts_with("Bash(")), "{args:?}");
  write.set_repository(RepositoryAccess::ReadOnly);
  let args = permission_args_of(&write);
  assert!(!args.contains(&"--settings".to_owned()), "{args:?}");
  let tools = args.iter().position(|arg| arg == "--tools").expect("tools");
  assert!(!args[tools + 1].contains("Bash"), "{args:?}");
  let denied = args.iter().position(|arg| arg == "--disallowedTools").expect("denied");
  assert!(args[denied + 1..].contains(&"Bash".to_owned()), "{args:?}");
  assert_eq!(ClaudeApprovalScope::of(&write, &[]), None, "nothing is left to ask");
}

#[test]
fn claude_approvals_allowed_commands_deny_what_accept_edits_would_run() {
  let list = |entries: &[&str]| {
    CommandPermission::Allowed(
      AllowedCommands::new(
        entries
          .iter()
          .map(|entry| orchester_core::permission::CommandPrefix::new(entry).expect("prefix"))
          .collect(),
      )
      .expect("list"),
    )
  };
  let args = permission_args_of(&AgentPermissions::new(
    RepositoryAccess::Write,
    list(&["git"]),
    NetworkAccess::Off,
  ));
  assert!(args.contains(&"acceptEdits".to_owned()), "{args:?}");
  for command in ["mkdir", "touch", "rm", "rmdir", "mv", "cp", "sed"] {
    assert!(args.contains(&format!("Bash({command}:*)")), "{command}: {args:?}");
  }
  assert!(!args.iter().any(|arg| arg.starts_with("Edit(")), "{args:?}");
  // A deny rule for rm would deny the listed rm too: no acceptEdits, edits by rule instead.
  let args = permission_args_of(&AgentPermissions::new(
    RepositoryAccess::Write,
    list(&["git", "rm -f build/x"]),
    NetworkAccess::Off,
  ));
  assert!(args.contains(&"dontAsk".to_owned()), "{args:?}");
  assert!(
    !args
      .iter()
      .any(|arg| arg.starts_with("Bash(rm") && arg.ends_with(":*)") && arg != "Bash(rm -f build/x:*)")
  );
  let allowed = args.iter().position(|arg| arg == "--allowedTools").expect("allowed");
  assert_eq!(
    args[allowed + 1..],
    [
      "Bash(git:*)",
      "Bash(rm -f build/x:*)",
      "Edit(//c/repo/.orchester/worktrees/run/**)"
    ]
  );
  let read_only = permission_args_of(&AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    list(&["git"]),
    NetworkAccess::Off,
  ));
  assert!(
    !read_only.contains(&"Bash(mkdir:*)".to_owned()),
    "dontAsk runs nothing by mode"
  );
}

#[test]
fn claude_approvals_edit_by_rule_in_default_mode_when_mcp_actions_ask() {
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(
      AllowedCommands::new(vec![
        orchester_core::permission::CommandPrefix::new("sed -n").expect("prefix"),
      ])
      .expect("list"),
    ),
    NetworkAccess::Off,
  );
  permissions.set_interactive_tool_approvals(true);
  let args = permission_args_of(&permissions);
  // dontAsk would deny before the permission prompt is asked.
  assert!(args.contains(&"default".to_owned()), "{args:?}");
  assert!(!args.contains(&"Bash(sed:*)".to_owned()), "{args:?}");
  assert!(
    args.contains(&"Edit(//c/repo/.orchester/worktrees/run/**)".to_owned()),
    "{args:?}"
  );
}

#[test]
fn claude_approvals_keep_the_code_index_read_only_beside_the_ask_rule() {
  let enforced = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  )
  .enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows)
  .expect("enforceable");
  let invocation = ClaudeInvocation {
    executable: Path::new("/bin/claude"),
    model: "claude-opus-5-5",
    effort: None,
    permissions: &enforced,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    session: ClaudeSession::New(uuid::Uuid::nil()),
    mcp: None,
    skills: None,
    code_index: Some(Path::new("/repo/.orchester/index/abc")),
    approvals: Some(Path::new("/tmp/approvals.json")),
  };
  let args = args(&invocation);
  let at = |flag: &str| args.iter().position(|arg| arg == flag).expect(flag);
  assert_eq!(args[at("--settings") + 1], r#"{"permissions":{"ask":["Bash"]}}"#);
  let denied = &args[at("--disallowedTools") + 1..];
  assert!(
    denied.contains(&"Edit(//repo/.orchester/index/abc/**)".to_owned()),
    "{args:?}"
  );
  assert!(
    args
      .windows(2)
      .any(|pair| pair == ["--add-dir", "/repo/.orchester/index/abc"]),
    "{args:?}"
  );
  assert_eq!(args[at("--permission-prompt-tool") + 1], PROMPT_TOOL);
  assert_eq!(args.iter().filter(|arg| *arg == "--disallowedTools").count(), 1);
}
