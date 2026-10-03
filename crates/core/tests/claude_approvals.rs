//! Claude Code's permission prompt: what reaches a person, what the prompt answers, the relay's MCP
//! side, and how an invocation that asks a person starts.

use std::path::Path;

use orchester_core::{
  command::SecretVariable,
  connection::{
    AgentProvider,
    claude_approval::{
      ApprovalToken, ClaudeApprovalConfig, ClaudeApprovalScope, ClaudeAsk, PROMPT_TOOL, not_answered, prompt_result,
      refusal,
      relay::{RelayStep, ask_result, relay_step},
    },
    invocation::{ClaudeInvocation, ClaudeMcp, ClaudeSession, InvocationError, Platform},
    mcp::{InvocationExemptions, McpServerName},
  },
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess, claude::PermissionFolders,
  },
  resource::CommandEnvironment,
  run::{ARGUMENTS_CEILING, cut_arguments, recorded_arguments},
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

/// The scope of a step that asks about commands (`commands`) and MCP actions (`tools`). Only *Any
/// command in the worktree*, which Claude Code cannot enforce, leaves commands out of it.
fn scope(commands: bool, tools: bool) -> ClaudeApprovalScope {
  let commands = if commands {
    CommandPermission::AskBefore
  } else {
    CommandPermission::AnyInWorktree
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
  // The person sees the arguments too, whole.
  assert_eq!(tool.scope(), Some(r#"{"filePath":"notes.md","text":"x"}"#));
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
  // Agents ask: even an empty list asks about every command.
  let empty = permissions(CommandPermission::Allowed(AllowedCommands::default()), false);
  let ClaudeAsk::Person(_) = ClaudeApprovalScope::of(&empty, &[]).expect("asks").classify(&bash) else {
    panic!("a command goes to a person");
  };
}

fn secret(name: &str, value: &str) -> SecretVariable {
  SecretVariable::new(
    name,
    std::sync::Arc::new(orchester_core::secret::SecretValue::new(value.to_owned()).expect("secret")),
  )
}

/// The arguments a person is shown for an MCP call with `input`, `secrets` removed.
fn shown_with(secrets: Vec<SecretVariable>, input: &Value) -> String {
  let ClaudeAsk::Person(request) = scope(false, true)
    .with_secrets(secrets)
    .classify(&call("mcp__docs__write", input))
  else {
    panic!("a tool of an invocation server goes to a person");
  };
  request.scope().expect("arguments").to_owned()
}

#[test]
fn claude_approvals_record_long_mcp_arguments_whole() {
  let lines: Vec<String> = (0..100)
    .map(|line| format!("line {line:02}: {}", "x".repeat(50)))
    .collect();
  let input = json!({"filePath": "notes.md", "lines": lines, "depth": 3});
  let raw = input.to_string();
  assert!(raw.len() > 5 * 1024, "{} bytes", raw.len());
  let shown = shown_with(Vec::new(), &input);
  assert_eq!(shown, raw, "5 KB of arguments arrive whole");
  assert_eq!(cut_arguments(&shown), None);
}

#[test]
fn claude_approvals_show_mcp_arguments_without_secrets_that_json_escapes() {
  let quoted = r#"to"k\en"#;
  let escaped = shown_with(
    vec![secret("QUOTED", quoted)],
    &json!({"header": format!("Bearer {quoted}")}),
  );
  assert_eq!(escaped, r#"{"header":"Bearer <redacted>"}"#);
}

#[test]
fn claude_approvals_remove_a_secret_held_as_a_number() {
  let shown = shown_with(
    vec![secret("PIN", "4815162342")],
    &json!({"pin": 4_815_162_342_u64, "retries": 3}),
  );
  assert_eq!(shown, r#"{"pin":"<redacted>","retries":3}"#);
}

#[test]
fn claude_approvals_remove_a_secret_deep_inside_long_arguments() {
  let token = "sk-live-0123456789abcdef";
  let text = format!(
    "{}Authorization: Bearer {token}{}",
    "a".repeat(8_000),
    "b".repeat(8_000)
  );
  let shown = shown_with(vec![secret("TOKEN", token)], &json!({"request": text}));
  assert!(!shown.contains(token), "the secret is not shown");
  assert!(shown.contains("Authorization: Bearer <redacted>"));
  assert!(
    shown.ends_with(&format!("{}\"}}", "b".repeat(8_000))),
    "nothing after it is lost"
  );
}

#[test]
fn claude_approvals_cut_only_pathological_arguments_visibly_and_never_inside_a_secret() {
  let long = "s".repeat(40);
  // The secret starts before the ceiling and ends after it: it is removed before the cut.
  let text = format!("{}{long}{}", "x".repeat(ARGUMENTS_CEILING - 30), "y".repeat(1_000));
  let shown = shown_with(vec![secret("LONG", &long)], &json!({"text": text}));
  let kept = cut_arguments(&shown).expect("arguments past the ceiling are cut");
  assert!(shown.ends_with("\n… cut at 256 KB"), "the cut is marked");
  assert!(kept.len() <= ARGUMENTS_CEILING && kept.len() > ARGUMENTS_CEILING - 4);
  assert!(kept.contains("<redacted>"));
  assert!(!shown.contains("ssss"), "no part of the secret remains");
  // A cut never splits a character.
  let wide = recorded_arguments("é".repeat(ARGUMENTS_CEILING));
  assert_eq!(cut_arguments(&wide).expect("cut").len(), ARGUMENTS_CEILING);
  let odd = recorded_arguments(format!("x{}", "é".repeat(ARGUMENTS_CEILING)));
  assert_eq!(cut_arguments(&odd).expect("cut").len(), ARGUMENTS_CEILING - 1);
  assert_eq!(recorded_arguments("{}".to_owned()), "{}");
}

#[test]
fn claude_approvals_answer_one_call_without_grants_or_its_input() {
  let allow: Value = serde_json::from_str(&prompt_result(None)).expect("JSON");
  assert_eq!(allow, json!({"behavior": "allow", "updatedInput": {}}));
  let told = "The person reviewing this run denied pencil batch_design. Continue without it.";
  let deny: Value = serde_json::from_str(&prompt_result(Some(told))).expect("JSON");
  // Denied with what the agent is told, and without interrupting it: the turn goes on.
  assert_eq!(deny, json!({"behavior": "deny", "message": told}));
  assert!(deny.get("updatedPermissions").is_none() && deny.get("interrupt").is_none());
  let unanswered: Value = serde_json::from_str(&not_answered()).expect("JSON");
  assert_eq!(unanswered["behavior"], "deny");
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
  let answer: Value = serde_json::from_str(&ask_result(&json!(2), &prompt_result(None))).expect("JSON");
  assert_eq!(answer["id"], 2);
  assert_eq!(answer["result"]["content"][0]["text"], prompt_result(None));
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
      mcp: Some(ClaudeMcp {
        config: Path::new("/tmp/servers.json"),
        servers: &servers,
        exempt: &InvocationExemptions::default(),
      }),
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
    let command = invocation.command("", Vec::new(), &CommandEnvironment::default());
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
    &InvocationExemptions::default(),
    folders,
  )
}

/// Asking before commands sends every command to a person, in a read-only step too: there the edit
/// tools stay removed and the step runs in `default`, as `dontAsk` would deny before asking.
#[test]
fn claude_approvals_ask_before_every_command_also_in_read_only_steps() {
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
  assert_eq!(
    values(&args, "--settings"),
    [r#"{"permissions":{"ask":["Bash"]}}"#],
    "{args:?}"
  );
  assert_eq!(values(&args, "--permission-mode"), ["default"], "{args:?}");
  assert!(values(&args, "--tools")[0].contains("Bash"), "{args:?}");
  assert!(
    values(&args, "--disallowedTools").starts_with(&["Edit", "Write", "NotebookEdit"]),
    "{args:?}"
  );
  let scope = ClaudeApprovalScope::of(&write, &[]).expect("every command asks");
  let ClaudeAsk::Person(_) = scope.classify(&call("Bash", &json!({"command": "ls"}))) else {
    panic!("a command goes to a person");
  };
}

fn listed(repository: RepositoryAccess, entries: &[&str]) -> AgentPermissions {
  let entries = entries
    .iter()
    .map(|entry| orchester_core::permission::CommandPrefix::new(entry).expect("prefix"))
    .collect();
  AgentPermissions::new(
    repository,
    CommandPermission::Allowed(AllowedCommands::new(entries).expect("list")),
    NetworkAccess::Off,
  )
}

/// After `flag`, up to the next option.
fn values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
  args
    .iter()
    .skip_while(|arg| *arg != flag)
    .skip(1)
    .take_while(|arg| !arg.starts_with("--"))
    .map(String::as_str)
    .collect()
}

/// "List + ask": the listed commands run, any other asks a person, in a writing and a read-only step.
#[test]
fn claude_approvals_allowed_commands_ask_for_the_others() {
  let unlisted = call("Bash", &json!({"command": "pnpm build"}));
  for entries in [&["git"][..], &["git", "rm -f build/x"]] {
    let write = listed(RepositoryAccess::Write, entries);
    let args = permission_args_of(&write);
    // dontAsk would deny before asking, and acceptEdits would run `rm`, `sed` and the other file commands in any form.
    assert_eq!(values(&args, "--permission-mode"), ["default"], "{args:?}");
    assert_eq!(
      values(&args, "--disallowedTools"),
      ["WebFetch", "WebSearch"],
      "{args:?}"
    );
    let mut allowed: Vec<String> = entries.iter().map(|entry| format!("Bash({entry}:*)")).collect();
    allowed.push("Edit(//c/repo/.orchester/worktrees/run/**)".to_owned());
    assert_eq!(values(&args, "--allowedTools"), allowed, "{args:?}");
    let scope = ClaudeApprovalScope::of(&write, &[]).expect("unlisted commands ask");
    assert!(matches!(scope.classify(&unlisted), ClaudeAsk::Person(request) if request.action() == "pnpm build"));
    let outside = call("Write", &json!({"file_path": "/etc/hosts"}));
    assert_eq!(scope.classify(&outside), ClaudeAsk::Refused, "edits keep their rules");
  }
  let read_only = listed(RepositoryAccess::ReadOnly, &["git"]);
  let args = permission_args_of(&read_only);
  assert_eq!(values(&args, "--permission-mode"), ["default"], "{args:?}");
  assert_eq!(values(&args, "--allowedTools"), ["Bash(git:*)"], "{args:?}");
  assert!(
    values(&args, "--disallowedTools").starts_with(&["Edit", "Write", "NotebookEdit"]),
    "{args:?}"
  );
  let scope = ClaudeApprovalScope::of(&read_only, &[]).expect("a read-only step asks too");
  assert!(matches!(scope.classify(&unlisted), ClaudeAsk::Person(_)));
  // Without any listed command every command asks, as when asking before commands.
  let none = listed(RepositoryAccess::ReadOnly, &[]);
  let args = permission_args_of(&none);
  assert_eq!(values(&args, "--permission-mode"), ["default"], "{args:?}");
  assert_eq!(
    values(&args, "--settings"),
    [r#"{"permissions":{"ask":["Bash"]}}"#],
    "{args:?}"
  );
  assert!(!values(&args, "--disallowedTools").contains(&"Bash"), "{args:?}");
  let scope = ClaudeApprovalScope::of(&none, &[]).expect("every command asks");
  assert!(matches!(scope.classify(&unlisted), ClaudeAsk::Person(_)));
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
