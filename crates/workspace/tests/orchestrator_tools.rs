//! The Orchestrator chat's tools (`docs/design/orchestrator-toolset.md`): one list of names, of which the chat lists
//! those the app has, so a name whose tool another change still builds is not listed and breaks nothing; what only a
//! person decides is not among them; and the tools that wait for the person say so.

use orchester_core::orchestrator::change::Publishing;
use orchester_workspace::{
  app_mcp,
  orchestrator::tools::{TOOLS, listed},
};

/// The names of every tool of the app, of both its lists.
fn app_tools() -> Vec<String> {
  app_mcp::listed()
    .into_iter()
    .chain(app_mcp::setup::listed())
    .map(|tool| tool.name.to_string())
    .collect()
}

#[test]
fn orchestrator_tools_list_the_named_tools_the_app_has() {
  let app = app_tools();
  let expected: Vec<&str> = TOOLS
    .iter()
    .copied()
    .filter(|name| app.iter().any(|tool| tool == name))
    .collect();
  let mut shown: Vec<String> = listed().into_iter().map(|tool| tool.name.to_string()).collect();
  let mut wanted: Vec<String> = expected.iter().map(|name| (*name).to_owned()).collect();
  shown.sort();
  wanted.sort();
  assert_eq!(shown, wanted, "the chat lists exactly the named tools the app has");
  let mut unique = TOOLS.to_vec();
  unique.sort_unstable();
  unique.dedup();
  assert_eq!(unique.len(), TOOLS.len(), "each name once");
  for name in [
    "list_tasks",
    "run_task",
    "publish_pipeline",
    "merge_run",
    "save_tool_draft",
  ] {
    assert!(shown.iter().any(|tool| tool == name), "{name} is listed");
  }
}

#[test]
fn orchestrator_tools_leave_what_a_person_decides_out() {
  for name in [
    "decide_approval",
    "answer_question",
    "decide_plan",
    "continue_run",
    "send_message",
    "set_run_auto_mode",
    "set_auto_mode",
    "remove_worktree",
    "return_to_draft",
    "add_project",
    "set_project_environment",
    "set_repository_agent_files",
    "set_code_index",
    "update_project_limits",
    "add_cli_connection",
    "set_connection_account",
    "add_mcp_server",
    "update_mcp_server",
    "remove_mcp_server",
    "set_mcp_server_enabled",
  ] {
    assert!(!TOOLS.contains(&name), "{name} is not the chat's");
  }
}

#[test]
fn orchestrator_tools_say_what_waits_for_the_person() {
  let tools = listed();
  let description = |name: &str| {
    tools
      .iter()
      .find(|tool| tool.name == name)
      .and_then(|tool| tool.description.as_deref())
      .unwrap_or_else(|| panic!("{name} is listed with a description"))
      .to_owned()
  };
  assert!(description("run_task").contains("auto mode"));
  for name in ["cancel_run", "merge_run"] {
    assert!(
      description(name).contains("proposes the change to the person"),
      "{name}"
    );
  }
  for kind in Publishing::ALL {
    assert!(TOOLS.contains(&kind.tool()), "{} is named", kind.tool());
    assert_eq!(Publishing::of_tool(kind.tool()), Some(kind));
    if tools.iter().any(|tool| tool.name == kind.tool()) {
      let told = description(kind.tool());
      assert!(
        told.contains("Let the Orchestrator publish drafts")
          && told.contains("Let automation change agent permissions"),
        "{}: {told}",
        kind.tool()
      );
    }
  }
}
