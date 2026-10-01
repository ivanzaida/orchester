//! Tools that run without asking: which names a server can list, the Claude Code allow rule each one
//! gets when the step asks before MCP actions, the approval mode Codex App Server gets for it, and the
//! activity that says such a call was not asked about.

use std::path::Path;

use orchester_core::{
  connection::{
    AgentProvider,
    app_server::{AppServerStream, Update},
    app_server_mcp::thread_servers,
    invocation::{ClaudeInvocation, ClaudeMcp, ClaudeMcpConfig, ClaudeSession, CodexMcp, McpLaunch, Platform},
    mcp::{ExemptTools, ExemptToolsError, InvocationExemptions, McpServerName, McpToolName, McpToolNameError, McpUrl},
    stream::{AgentEvent, ClaudeStream, OutputStream},
  },
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
};
use serde_json::json;
use uuid::Uuid;

fn name(value: &str) -> McpServerName {
  McpServerName::new(value).expect("valid name")
}

fn tools(text: &str) -> ExemptTools {
  ExemptTools::parse(text).expect("valid tools")
}

fn server(server: &str, exempt: &str) -> McpLaunch {
  McpLaunch::http(
    name(server),
    McpUrl::new("http://localhost:7/mcp").expect("valid URL"),
    None,
  )
  .with_exempt_tools(tools(exempt))
}

/// `pencil` lists two read-only tools; `docs` lists none.
fn launches() -> Vec<McpLaunch> {
  vec![server("pencil", "get_screenshot\nbatch_get"), server("docs", "")]
}

#[test]
fn mcp_exempt_tool_names_are_names_both_clis_keep() {
  assert_eq!(
    McpToolName::new(" get_screenshot ").expect("valid").as_str(),
    "get_screenshot"
  );
  assert_eq!(McpToolName::new("Search-2").expect("valid").as_str(), "Search-2");
  assert_eq!(McpToolName::new(" "), Err(McpToolNameError::Blank));
  for invalid in ["get screenshot", "docs.search", "mcp/tool", "dökumente"] {
    assert_eq!(
      McpToolName::new(invalid),
      Err(McpToolNameError::Invalid(invalid.to_owned())),
      "{invalid}"
    );
  }
  assert_eq!(McpToolName::new(&"a".repeat(65)), Err(McpToolNameError::TooLong));
  let listed = tools("\nget_editor_state\n\n  batch_get\n");
  let names: Vec<&str> = listed.tools().iter().map(McpToolName::as_str).collect();
  assert_eq!(names, ["get_editor_state", "batch_get"], "one per line, in order");
  assert_eq!(
    ExemptTools::parse("batch_get\nbatch_get"),
    Err(ExemptToolsError::Duplicate("batch_get".to_owned()))
  );
  let many: Vec<String> = (0..=ExemptTools::MAX).map(|n| format!("tool_{n}")).collect();
  assert_eq!(ExemptTools::parse(&many.join("\n")), Err(ExemptToolsError::TooMany));
  assert!(serde_json::from_str::<ExemptTools>(r#"["get_screenshot","bad name"]"#).is_err());
}

fn claude_args(tool_approvals: bool, config: &ClaudeMcpConfig) -> Vec<String> {
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::On,
  );
  permissions.set_interactive_tool_approvals(tool_approvals);
  let enforced = permissions
    .enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows)
    .expect("enforceable");
  ClaudeInvocation {
    executable: Path::new("/bin/claude"),
    model: "haiku",
    effort: None,
    permissions: &enforced,
    writable_roots: &[],
    worktree: Path::new("/repo/worktree"),
    session: ClaudeSession::New(Uuid::nil()),
    mcp: Some(ClaudeMcp {
      config: Path::new("/tmp/servers.json"),
      servers: config.servers(),
      exempt: config.exemptions(),
    }),
    skills: None,
    code_index: None,
    approvals: Some(Path::new("/tmp/approvals.json")),
  }
  .args()
  .into_iter()
  .map(|arg| arg.into_string().expect("unicode"))
  .collect()
}

/// The values after `--allowedTools`, up to the next option.
fn allowed(args: &[String]) -> Vec<&str> {
  let Some(start) = args.iter().position(|arg| arg == "--allowedTools") else {
    return Vec::new();
  };
  args[start + 1..]
    .iter()
    .take_while(|arg| !arg.starts_with("--"))
    .map(String::as_str)
    .collect()
}

/// Asking before MCP actions leaves out the server-wide rules, so every call asks, except the listed
/// tools, which get one exact rule each. Without asking, the listed tools change nothing.
#[test]
fn mcp_exempt_tools_get_one_claude_allow_rule_each_when_the_step_asks() {
  let (config, left_out) = ClaudeMcpConfig::new(&launches());
  assert!(left_out.is_empty());
  let config = config.expect("servers");
  assert_eq!(
    allowed(&claude_args(true, &config)),
    ["mcp__pencil__get_screenshot", "mcp__pencil__batch_get"]
  );
  assert_eq!(allowed(&claude_args(false, &config)), ["mcp__pencil", "mcp__docs"]);
}

/// Codex gets no argument for them: the thread's configuration approves each listed tool, and only in a
/// step that asks (the runner passes no exemptions otherwise).
#[test]
fn mcp_exempt_tools_are_approved_in_the_codex_thread_configuration() {
  let (codex, _) = CodexMcp::new(&launches());
  assert!(
    !codex.args().iter().any(|arg| arg.contains("approval_mode")),
    "{:?}",
    codex.args()
  );
  assert!(codex.exemptions().exempts("pencil", "batch_get"));
  assert!(!codex.exemptions().exempts("pencil", "batch_design"));
  assert!(!codex.exemptions().exempts("docs", "batch_get"));
  assert_eq!(
    serde_json::Value::Object(thread_servers(&["user-notes".to_owned()], codex.exemptions())),
    json!({
      "user-notes": {"enabled": false},
      "pencil": {"tools": {
        "get_screenshot": {"approval_mode": "approve"},
        "batch_get": {"approval_mode": "approve"},
      }},
    })
  );
  assert_eq!(
    serde_json::Value::Object(thread_servers(&[], &InvocationExemptions::default())),
    json!({})
  );
}

fn exemptions() -> InvocationExemptions {
  InvocationExemptions::new([(name("pencil"), tools("get_screenshot")), (name("docs"), tools(""))])
}

#[test]
fn mcp_exempt_claude_call_is_reported_as_not_asked() {
  let mut stream = ClaudeStream::with_exemptions(exemptions());
  let line = |tool: &str, id: &str| {
    json!({"type":"assistant","message":{"id":id,"content":[
      {"type":"tool_use","id":id,"name":tool,"input":{}}
    ]}})
    .to_string()
  };
  let events = stream.parse(OutputStream::Stdout, &line("mcp__pencil__get_screenshot", "toolu_1"));
  assert_eq!(
    events[1..],
    [
      AgentEvent::CommandStarted {
        item: "toolu_1".to_owned(),
        command: "mcp__pencil__get_screenshot".to_owned(),
      },
      AgentEvent::McpToolNotAsked {
        item: "toolu_1".to_owned(),
        server: "pencil".to_owned(),
        tool: "get_screenshot".to_owned(),
      },
    ]
  );
  let other = stream.parse(OutputStream::Stdout, &line("mcp__pencil__batch_design", "toolu_2"));
  assert!(
    !other
      .iter()
      .any(|event| matches!(event, AgentEvent::McpToolNotAsked { .. })),
    "{other:?}"
  );
}

#[test]
fn mcp_exempt_codex_call_is_reported_as_not_asked() {
  let mut stream = AppServerStream::with_exemptions(exemptions());
  let started = |item: &str, tool: &str| {
    json!({"method":"item/started","params":{"threadId":"t","turnId":"u","item":{
      "id":item,"type":"mcpToolCall","server":"pencil","tool":tool,"arguments":{}
    }}})
    .to_string()
  };
  let updates = stream.parse(&started("i1", "get_screenshot")).expect("valid");
  let [Update::Activity(event)] = updates.as_slice() else {
    panic!("one activity event");
  };
  assert_eq!(
    *event,
    AgentEvent::McpToolNotAsked {
      item: "i1".to_owned(),
      server: "pencil".to_owned(),
      tool: "get_screenshot".to_owned(),
    }
  );
  assert!(stream.parse(&started("i2", "batch_design")).expect("valid").is_empty());
  let mut plain = AppServerStream::default();
  assert!(plain.parse(&started("i3", "get_screenshot")).expect("valid").is_empty());
}
