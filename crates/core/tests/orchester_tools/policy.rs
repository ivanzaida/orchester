//! The policy table, and how each CLI is told what runs and what asks.

use orchester_core::{
  connection::{
    claude_approval::{ApprovalToken, ClaudeApprovalScope, ClaudeAsk},
    invocation::{CodexMcp, CodexOrchester},
  },
  orchester_tools::{OrchesterTool, StepTools, ToolPolicy, auto_approvable, decide},
  permission::NetworkAccess,
  resource::RepositoryAccess,
};
use serde_json::json;

use crate::{permissions, project_tool, reading, writing};

/// | class | writing step | read-only step |: read-only tools run, worktree writes run only where the step
/// may write, and the tools that reach outside ask in both.
#[test]
fn orchester_tools_policy_follows_the_table() {
  let write = permissions(RepositoryAccess::Write);
  let read = permissions(RepositoryAccess::ReadOnly);
  for (tool, writing, reading) in [
    (OrchesterTool::GitStatus, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::GitDiff, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::GitLog, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::GitShow, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::GitAdd, Some(ToolPolicy::Runs), None),
    (OrchesterTool::GitCommit, Some(ToolPolicy::Runs), None),
    (
      OrchesterTool::ReadOutsideFile,
      Some(ToolPolicy::Asks),
      Some(ToolPolicy::Asks),
    ),
    (
      // These steps have no network (see the next test for one with it).
      OrchesterTool::RunProjectTool,
      None,
      None,
    ),
  ] {
    assert_eq!(decide(tool, &write), writing, "{tool:?}");
    assert_eq!(decide(tool, &read), reading, "{tool:?}");
  }
  let mut asking = read;
  asking.set_interactive_tool_approvals(true);
  assert_eq!(StepTools::for_step(&asking, &[]), reading());
  let names = |tools: &StepTools| -> Vec<&str> { tools.offered().iter().map(|(tool, _)| tool.as_str()).collect() };
  assert_eq!(
    names(&reading()),
    ["git_status", "git_diff", "git_log", "git_show", "read_outside_file"]
  );
  let project = [project_tool("Run tests", "/bin/true")];
  let mut online = write;
  online.set_network(NetworkAccess::On);
  let offered = StepTools::for_step(&online, &project);
  assert_eq!(offered.policy(OrchesterTool::RunProjectTool), Some(ToolPolicy::Asks));
  let listed = offered.mcp_tools();
  let run = listed
    .iter()
    .find(|tool| tool["name"] == "run_project_tool")
    .expect("listed");
  assert_eq!(run["inputSchema"]["properties"]["name"]["enum"], json!(["Run tests"]));
  assert_eq!(run["inputSchema"]["additionalProperties"], json!(false));
  assert!(
    listed
      .iter()
      .all(|tool| tool["inputSchema"]["additionalProperties"] == json!(false))
  );
}
#[test]
fn orchester_tools_tell_each_cli_what_runs_and_what_asks() {
  assert_eq!(
    reading().claude_allow_rules(),
    [
      "mcp__Orchester__git_status",
      "mcp__Orchester__git_diff",
      "mcp__Orchester__git_log",
      "mcp__Orchester__git_show"
    ]
  );
  assert_eq!(
    writing().codex_tools(),
    json!({"tools": {
      "git_status": {"approval_mode": "approve"},
      "git_diff": {"approval_mode": "approve"},
      "git_log": {"approval_mode": "approve"},
      "git_show": {"approval_mode": "approve"},
      "git_add": {"approval_mode": "approve"},
      "git_commit": {"approval_mode": "approve"},
      "read_outside_file": {"approval_mode": "approve"},
    }})
  );
  let token = ApprovalToken::generate();
  let server = CodexOrchester::new(
    "/bin/orchester-runner",
    &["--orchester-tools"],
    "127.0.0.1:4000",
    token.variable(),
  );
  let mcp = CodexMcp::new(&[]).0.with_orchester(server);
  assert_eq!(
    mcp.args(),
    [
      "-c",
      r#"mcp_servers.Orchester.command="/bin/orchester-runner""#,
      "-c",
      r#"mcp_servers.Orchester.args=["--orchester-tools"]"#,
      "-c",
      r#"mcp_servers.Orchester.env.ORCHESTER_APPROVAL_ADDRESS="127.0.0.1:4000""#,
      "-c",
      r#"mcp_servers.Orchester.env_vars=["ORCHESTER_APPROVAL_TOKEN"]"#,
      "-c",
      "mcp_servers.Orchester.tool_timeout_sec=86460",
      "-c",
      r#"shell_environment_policy.exclude=["ORCHESTER_APPROVAL_TOKEN"]"#,
    ]
  );
  assert_eq!(mcp.secrets().len(), 1);
  assert_eq!(mcp.secrets()[0].expose(), token.expose());
  assert!(!format!("{mcp:?}").contains(token.expose()));
  assert!(!mcp.args().concat().contains(token.expose()));
}
/// Claude Code asks through its permission prompt about the tools without an allow rule: those reach a
/// person whether or not the step asks before MCP actions; a call of one that runs by rule never should.
#[test]
fn orchester_tools_that_ask_reach_a_person_through_claudes_prompt() {
  let scope = ClaudeApprovalScope::of(&permissions(RepositoryAccess::ReadOnly), &[])
    .expect("a step with allowed commands asks")
    .with_orchester_tools(reading());
  let call = |tool: &str| {
    scope.classify(&json!({
      "tool_name": tool,
      "input": {"path": "/deps/src/lib.rs"},
      "tool_use_id": "toolu_1",
    }))
  };
  let ClaudeAsk::Person(request) = call("mcp__Orchester__read_outside_file") else {
    panic!("a read outside asks a person");
  };
  assert_eq!(request.server(), "Orchester");
  assert_eq!(request.action(), "read_outside_file");
  assert!(!request.is_command());
  assert_eq!(request.scope(), Some(r#"{"path":"/deps/src/lib.rs"}"#));
  assert_eq!(call("mcp__Orchester__git_status"), ClaudeAsk::Refused);
  assert_eq!(call("mcp__Orchester__git_commit"), ClaudeAsk::Refused);
  assert_eq!(call("mcp__Orchester__approve"), ClaudeAsk::Refused);
}

/// A project's program runs outside every sandbox: a step without network is not offered it, and
/// auto-approval never runs it.
#[test]
fn orchester_tools_project_programs_follow_the_network_and_skip_auto_approval() {
  let mut online = permissions(RepositoryAccess::Write);
  online.set_network(NetworkAccess::On);
  assert_eq!(decide(OrchesterTool::RunProjectTool, &online), Some(ToolPolicy::Asks));
  assert_eq!(
    decide(OrchesterTool::RunProjectTool, &permissions(RepositoryAccess::Write)),
    None
  );
  let project = [project_tool("Run tests", "/bin/true")];
  let offline = StepTools::for_step(&permissions(RepositoryAccess::Write), &project);
  assert_eq!(offline.policy(OrchesterTool::RunProjectTool), None);
  assert!(!auto_approvable("Orchester", "run_project_tool"));
  assert!(auto_approvable("Orchester", "read_outside_file"));
  assert!(auto_approvable("pencil", "run_project_tool"));
}
