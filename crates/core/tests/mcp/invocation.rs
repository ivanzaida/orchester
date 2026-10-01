use std::{path::Path, sync::Arc};

use orchester_core::{
  command::Command,
  connection::{
    AgentProvider,
    invocation::{
      ClaudeInvocation, ClaudeMcpConfig, ClaudeSession, CodexAgentFiles, CodexInvocation, CodexMcp, LaunchValue,
      McpLaunch, Platform,
    },
    mcp::{McpServerName, McpServerProblem, McpUrl},
  },
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{CommandEnvironment, VariableName},
  secret::SecretValue,
};
use uuid::Uuid;

const TOKEN: &str = "s3cr3t-docs-token";
const BEARER: &str = "s3cr3t-bearer";

fn secret(value: &str) -> Arc<SecretValue> {
  Arc::new(SecretValue::new(value.to_owned()).expect("not blank"))
}

fn name(value: &str) -> McpServerName {
  McpServerName::new(value).expect("valid name")
}

fn variable(value: &str) -> VariableName {
  VariableName::new(value).expect("valid name")
}

/// A server the CLI starts, with a plain variable and one from a secret.
fn files(token: &Arc<SecretValue>) -> McpLaunch {
  McpLaunch::stdio(
    name("files"),
    (
      "/usr/bin/node".to_owned(),
      vec!["server.js".to_owned(), "--root".to_owned(), "a \"b\"".to_owned()],
    ),
    None,
    vec![
      (variable("LOG_LEVEL"), LaunchValue::Text("debug".to_owned())),
      (variable("DOCS_TOKEN"), LaunchValue::Secret(token.clone())),
    ],
  )
}

fn remote(bearer: Option<Arc<SecretValue>>) -> McpLaunch {
  McpLaunch::http(
    name("project-knowledge"),
    McpUrl::new("https://mcp.example.dev/v1").expect("valid URL"),
    bearer,
  )
}

fn codex_command(mcp: &CodexMcp) -> Command {
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    repository: Path::new("/repo"),
    access: RepositoryAccess::ReadOnly,
    network: NetworkAccess::Off,
    writable_roots: &[],
    worktree: Path::new("/repo/worktree"),
    platform: Platform::MacOs,
    mcp: Some(mcp),
    agent_files: CodexAgentFiles::Loaded,
  }
  .command(Vec::new(), &CommandEnvironment::default())
  .expect("a Unicode worktree")
}

/// Nothing a person or a log could read holds the secret values: arguments, the command's `Debug`
/// output and its plain variables.
fn assert_hidden(command: &Command, values: &[&str]) {
  let shown = format!("{command:?}");
  for value in values {
    assert!(
      !command.args().iter().any(|arg| arg.contains(value)),
      "{value} in {:?}",
      command.args()
    );
    assert!(!shown.contains(value), "{value} in {shown}");
    assert!(!command.env().iter().any(|(_, plain)| plain.contains(value)));
  }
}

#[test]
fn mcp_codex_gets_servers_as_configuration_overrides_without_secret_values() {
  let (token, bearer) = (secret(TOKEN), secret(BEARER));
  let (mcp, left_out) = CodexMcp::new(&[files(&token), remote(Some(bearer))]);
  assert!(left_out.is_empty(), "{left_out:?}");
  let expected: Vec<String> = [
    "-c",
    r#"mcp_servers.files.command="/usr/bin/node""#,
    "-c",
    r#"mcp_servers.files.args=["server.js","--root","a \"b\""]"#,
    "-c",
    r#"mcp_servers.files.env.LOG_LEVEL="debug""#,
    "-c",
    r#"mcp_servers.files.env_vars=["DOCS_TOKEN"]"#,
    "-c",
    r#"mcp_servers.project-knowledge.url="https://mcp.example.dev/v1""#,
    "-c",
    r#"mcp_servers.project-knowledge.bearer_token_env_var="ORCHESTER_MCP_TOKEN_PROJECT_KNOWLEDGE""#,
    "-c",
    r#"shell_environment_policy.exclude=["DOCS_TOKEN","ORCHESTER_MCP_TOKEN_PROJECT_KNOWLEDGE"]"#,
  ]
  .map(str::to_owned)
  .to_vec();
  assert_eq!(mcp.args(), expected.as_slice());
  let command = codex_command(&mcp);
  let args = command.args();
  assert_eq!(
    &args[args.len() - expected.len()..],
    expected.as_slice(),
    "the servers come last"
  );
  assert_hidden(&command, &[TOKEN, BEARER]);
  let secrets: Vec<(&str, &str)> = command
    .secret_env()
    .iter()
    .map(|variable| (variable.name(), variable.expose()))
    .collect();
  assert_eq!(
    secrets,
    vec![("DOCS_TOKEN", TOKEN), ("ORCHESTER_MCP_TOKEN_PROJECT_KNOWLEDGE", BEARER)]
  );
}

#[test]
fn mcp_codex_passes_a_working_directory_and_leaves_out_what_cannot_reach_a_server() {
  let (token, other) = (secret(TOKEN), secret("another-value"));
  let in_folder = McpLaunch::stdio(
    name("docs"),
    ("uvx".to_owned(), Vec::new()),
    Some("/repo/worktree/tools".to_owned()),
    vec![(variable("DOCS_TOKEN"), LaunchValue::Secret(token.clone()))],
  );
  let clash = McpLaunch::stdio(
    name("clash"),
    ("uvx".to_owned(), Vec::new()),
    None,
    vec![(variable("DOCS_TOKEN"), LaunchValue::Secret(other))],
  );
  let same_secret = McpLaunch::stdio(
    name("same"),
    ("uvx".to_owned(), Vec::new()),
    None,
    vec![(variable("docs_token"), LaunchValue::Secret(token))],
  );
  let reserved = McpLaunch::stdio(
    name("reserved"),
    ("uvx".to_owned(), Vec::new()),
    None,
    vec![(variable("PATH"), LaunchValue::Secret(secret("path-value")))],
  );
  let (mcp, left_out) = CodexMcp::new(&[in_folder, clash, same_secret, reserved]);
  assert!(
    mcp
      .args()
      .contains(&r#"mcp_servers.docs.cwd="/repo/worktree/tools""#.to_owned())
  );
  assert!(!mcp.args().iter().any(|arg| arg.contains("mcp_servers.clash.")));
  assert!(!mcp.args().iter().any(|arg| arg.contains("mcp_servers.reserved.")));
  assert_eq!(
    left_out,
    vec![
      (
        name("clash"),
        McpServerProblem::SharedVariable {
          variable: "DOCS_TOKEN".to_owned()
        }
      ),
      (
        name("same"),
        McpServerProblem::SharedVariable {
          variable: "docs_token".to_owned()
        }
      ),
      (
        name("reserved"),
        McpServerProblem::ReservedVariable {
          variable: "PATH".to_owned()
        }
      ),
    ]
  );
  assert_eq!(mcp.secrets().len(), 1);
}

#[test]
fn mcp_codex_without_servers_adds_nothing() {
  let (mcp, left_out) = CodexMcp::new(&[]);
  assert!(mcp.args().is_empty() && mcp.secrets().is_empty() && left_out.is_empty());
  let command = codex_command(&mcp);
  assert!(!command.args().iter().any(|arg| arg.contains("mcp_servers")));
  assert!(command.secret_env().is_empty());
}

#[test]
fn mcp_claude_gets_a_private_config_file_and_allows_only_the_servers_tools() {
  let (token, bearer) = (secret(TOKEN), secret(BEARER));
  let in_folder = McpLaunch::stdio(
    name("docs"),
    ("uvx".to_owned(), Vec::new()),
    Some("/repo/worktree/tools".to_owned()),
    Vec::new(),
  );
  let (config, left_out) = ClaudeMcpConfig::new(&[files(&token), remote(Some(bearer)), in_folder]);
  let config = config.expect("two servers are left");
  assert_eq!(
    left_out,
    vec![(name("docs"), McpServerProblem::WorkingDirectoryUnsupported)]
  );
  assert_eq!(config.servers(), &[name("files"), name("project-knowledge")]);
  let file: serde_json::Value = serde_json::from_str(config.contents()).expect("JSON");
  assert_eq!(
    file,
    serde_json::json!({"mcpServers": {
      "files": {"type": "stdio", "command": "/usr/bin/node", "args": ["server.js", "--root", "a \"b\""],
        "env": {"DOCS_TOKEN": TOKEN, "LOG_LEVEL": "debug"}},
      "project-knowledge": {"type": "http", "url": "https://mcp.example.dev/v1",
        "headers": {"Authorization": format!("Bearer {BEARER}")}},
    }})
  );
  let shown = format!("{config:?}");
  assert!(!shown.contains(TOKEN) && !shown.contains(BEARER), "{shown}");

  let permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
  .enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows)
  .expect("enforceable");
  let config_path = Path::new("/tmp/orchester-mcp/invocation.json");
  let command = ClaudeInvocation {
    executable: Path::new("/bin/claude"),
    model: "haiku",
    effort: None,
    permissions: &permissions,
    writable_roots: &[],
    worktree: Path::new("/repo/worktree"),
    session: ClaudeSession::New(Uuid::nil()),
    mcp: Some((config_path, config.servers())),
    skills: None,
    code_index: None,
    // Every Claude Code step asks about commands through the permission prompt.
    approvals: Some(Path::new("/tmp/orchester-mcp/approvals.json")),
  }
  .command("Do it.".to_owned(), Vec::new(), &CommandEnvironment::default())
  .expect("a Unicode worktree");
  let args = command.args();
  assert!(args.iter().any(|arg| arg == "--strict-mcp-config"));
  let config = args
    .iter()
    .position(|arg| arg == "--mcp-config")
    .expect("the config file");
  assert_eq!(Path::new(&args[config + 1]), config_path);
  let allowed = args
    .iter()
    .position(|arg| arg == "--allowedTools")
    .expect("allowed tools");
  assert_eq!(
    &args[allowed + 1..allowed + 3],
    ["mcp__files", "mcp__project-knowledge"]
  );
  assert_eq!(args[allowed + 3], "--session-id");
  assert_hidden(&command, &[TOKEN, BEARER]);
  assert!(
    command.secret_env().is_empty(),
    "Claude Code reads secrets from the file only"
  );
}

#[test]
fn mcp_claude_without_servers_gets_no_config_file() {
  let in_folder = McpLaunch::stdio(
    name("docs"),
    ("uvx".to_owned(), Vec::new()),
    Some("/w/tools".to_owned()),
    Vec::new(),
  );
  let (config, left_out) = ClaudeMcpConfig::new(&[in_folder]);
  assert!(config.is_none());
  assert_eq!(left_out.len(), 1);
  assert!(ClaudeMcpConfig::new(&[]).0.is_none());
}
