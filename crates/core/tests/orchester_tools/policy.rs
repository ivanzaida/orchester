//! The policy table, and how each CLI is told what runs and what asks.

use orchester_core::{
  connection::{
    claude_approval::{ApprovalToken, ClaudeApprovalScope, ClaudeAsk},
    invocation::{CodexMcp, CodexOrchester},
  },
  orchester_tools::{OrchesterTool, StepTools, ToolCall, ToolPolicy, auto_approvable, decide},
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
    (OrchesterTool::ReadFile, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::ListFiles, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (
      OrchesterTool::SearchFile,
      Some(ToolPolicy::Runs),
      Some(ToolPolicy::Runs),
    ),
    (OrchesterTool::EditFile, Some(ToolPolicy::Runs), None),
    (OrchesterTool::CreateFile, Some(ToolPolicy::Runs), None),
    (
      OrchesterTool::SearchExternalFile,
      Some(ToolPolicy::Asks),
      Some(ToolPolicy::Asks),
    ),
    (
      OrchesterTool::ReadExternalFile,
      Some(ToolPolicy::Asks),
      Some(ToolPolicy::Asks),
    ),
    (
      // These steps have no network (see the next test for one with it).
      OrchesterTool::RunCheck,
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
    [
      "read_file",
      "list_files",
      "search_file",
      "find_references",
      "search_external_file",
      "read_external_file",
      "get_task",
      "save_memory",
      "report_progress"
    ]
  );
  for gone in [
    "git_status",
    "git_diff",
    "git_log",
    "git_show",
    "git_add",
    "git_commit",
    "write_file",
    "read_outside_file",
    "search",
  ] {
    assert_eq!(OrchesterTool::from_name(gone), None, "{gone}");
  }
  let project = [project_tool("Run tests", "/bin/true")];
  let mut online = write;
  online.set_network(NetworkAccess::On);
  let offered = StepTools::for_step(&online, &project);
  assert_eq!(offered.policy(OrchesterTool::RunCheck), Some(ToolPolicy::Asks));
  let listed = offered.mcp_tools();
  let run = listed.iter().find(|tool| tool["name"] == "run_check").expect("listed");
  assert_eq!(run["inputSchema"]["properties"]["name"]["enum"], json!(["Run tests"]));
  assert_eq!(run["inputSchema"]["additionalProperties"], json!(false));
  assert!(
    listed
      .iter()
      .all(|tool| tool["inputSchema"]["additionalProperties"] == json!(false))
  );
}
/// A check whose access rules approve no call runs without asking; with one that asks, `run_check` asks for that one.
#[test]
fn orchester_tools_checks_ask_by_their_access_rules() {
  let mut online = permissions(RepositoryAccess::Write);
  online.set_network(NetworkAccess::On);
  let project = [project_tool("Run tests", "/bin/true")];
  let quiet = orchester_core::orchester_tools::ProjectTool::new(
    "lint",
    "",
    orchester_core::resource::CommandAction::new(
      orchester_core::resource::CommandProgram::new("/bin/true").expect("a program"),
      Vec::new(),
    ),
    orchester_core::resource::ApprovalMode::Never,
  );
  let only_quiet = StepTools::for_step(&online, std::slice::from_ref(&quiet));
  assert_eq!(only_quiet.policy(OrchesterTool::RunCheck), Some(ToolPolicy::Runs));
  let mixed = StepTools::for_step(&online, &[quiet, project[0].clone()]);
  assert_eq!(mixed.policy(OrchesterTool::RunCheck), Some(ToolPolicy::Asks));
  let call = |name: &str| ToolCall::parse("run_check", &json!({ "name": name }), &mixed).expect("offered");
  assert!(!mixed.call_asks(&call("lint")));
  assert!(mixed.call_asks(&call("Run tests")));
}
#[test]
fn orchester_tools_tell_each_cli_what_runs_and_what_asks() {
  assert_eq!(
    reading().claude_allow_rules(),
    [
      "mcp__Orchester__read_file",
      "mcp__Orchester__list_files",
      "mcp__Orchester__search_file",
      "mcp__Orchester__find_references",
      "mcp__Orchester__get_task",
      "mcp__Orchester__save_memory",
      "mcp__Orchester__report_progress"
    ]
  );
  let approved = writing().codex_approved();
  assert_eq!(
    approved,
    [
      "read_file",
      "list_files",
      "search_file",
      "edit_file",
      "create_file",
      "delete_file",
      "move_file",
      "find_references",
      "search_external_file",
      "read_external_file",
      "get_task",
      "save_memory",
      "report_progress",
    ]
  );
  let token = ApprovalToken::generate();
  let server = CodexOrchester::new(
    "/bin/orchester-runner",
    &["--orchester-tools"],
    "127.0.0.1:4000",
    token.variable(),
    &approved[..2],
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
      r#"mcp_servers.Orchester.tools={"read_file"={approval_mode="approve"},"list_files"={approval_mode="approve"}}"#,
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
      "input": {"filename": "/deps/src/lib.rs"},
      "tool_use_id": "toolu_1",
    }))
  };
  let ClaudeAsk::Person(request) = call("mcp__Orchester__read_external_file") else {
    panic!("a read outside asks a person");
  };
  assert_eq!(request.server(), "Orchester");
  assert_eq!(request.action(), "read_external_file");
  assert!(!request.is_command());
  assert_eq!(request.scope(), Some(r#"{"filename":"/deps/src/lib.rs"}"#));
  assert_eq!(request.with_target("/deps/src".to_owned()).target(), Some("/deps/src"));
  assert_eq!(call("mcp__Orchester__read_file"), ClaudeAsk::Refused);
  assert_eq!(call("mcp__Orchester__edit_file"), ClaudeAsk::Refused);
  assert_eq!(call("mcp__Orchester__git_status"), ClaudeAsk::Refused);
  assert_eq!(call("mcp__Orchester__approve"), ClaudeAsk::Refused);
}

/// A project's program runs outside every sandbox: a step without network is not offered it, and
/// auto-approval never runs it.
#[test]
fn orchester_tools_project_programs_follow_the_network_and_skip_auto_approval() {
  let mut online = permissions(RepositoryAccess::Write);
  online.set_network(NetworkAccess::On);
  assert_eq!(decide(OrchesterTool::RunCheck, &online), Some(ToolPolicy::Asks));
  assert_eq!(
    decide(OrchesterTool::RunCheck, &permissions(RepositoryAccess::Write)),
    None
  );
  let project = [project_tool("Run tests", "/bin/true")];
  let offline = StepTools::for_step(&permissions(RepositoryAccess::Write), &project);
  assert_eq!(offline.policy(OrchesterTool::RunCheck), None);
  assert!(!auto_approvable("Orchester", "run_check"));
  assert!(auto_approvable("Orchester", "read_external_file"));
  assert!(auto_approvable("pencil", "run_check"));
}
