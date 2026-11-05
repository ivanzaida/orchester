//! The service tools: offered with the project's published services to a step that reaches the network, a start asks
//! and auto-approval never answers it, a stop, `service_status` and `service_logs` never ask, and all take only a
//! published service's name; `service_logs` takes a capped tail. `start_stack` comes with them when the project
//! publishes a stack, asks as a start does and takes only a published stack's name. The prompt tells the agent to
//! start servers only through `start_service` and `start_stack` and to look at and stop them with these tools, not by
//! shell.

use orchester_core::{
  orchester_tools::{OrchesterTool, StepTools, ToolCall, ToolCallError, ToolPolicy, auto_approvable},
  permission::NetworkAccess,
  resource::RepositoryAccess,
  service::{DEFAULT_TAIL, MAX_TAIL, ServiceName, StackName},
};
use serde_json::json;

use crate::permissions;

fn names(values: &[&str]) -> Vec<ServiceName> {
  values
    .iter()
    .map(|name| ServiceName::new(name).expect("a name"))
    .collect()
}

/// A step with `repository` access and network on, offered the project's `services`.
fn step(repository: RepositoryAccess, services: &[&str]) -> StepTools {
  let mut online = permissions(repository);
  online.set_network(NetworkAccess::On);
  StepTools::for_step(&online, &[]).with_services(&online, &names(services))
}

#[test]
fn service_tools_come_with_published_services_and_the_network() {
  let offline = permissions(RepositoryAccess::Write);
  let none = StepTools::for_step(&offline, &[]).with_services(&offline, &names(&["web"]));
  for tool in [
    OrchesterTool::StartService,
    OrchesterTool::StopService,
    OrchesterTool::ServiceStatus,
    OrchesterTool::ServiceLogs,
  ] {
    assert_eq!(none.policy(tool), None, "network off");
    assert_eq!(step(RepositoryAccess::Write, &[]).policy(tool), None, "no service");
  }
  for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
    let offered = step(repository, &["web", "api"]);
    assert_eq!(offered.policy(OrchesterTool::StartService), Some(ToolPolicy::Asks));
    assert_eq!(offered.policy(OrchesterTool::StopService), Some(ToolPolicy::Runs));
    assert_eq!(offered.policy(OrchesterTool::ServiceStatus), Some(ToolPolicy::Runs));
    assert_eq!(offered.policy(OrchesterTool::ServiceLogs), Some(ToolPolicy::Runs));
  }
  assert!(!auto_approvable("Orchester", "start_service"));
  assert!(auto_approvable("Orchester", "stop_service"));
  assert!(auto_approvable("Orchester", "service_status"));
  assert!(auto_approvable("Orchester", "service_logs"));
}

#[test]
fn service_tools_take_only_a_published_services_name() {
  let offered = step(RepositoryAccess::Write, &["web"]);
  assert_eq!(
    ToolCall::parse("start_service", &json!({"name": "web"}), &offered),
    Ok(ToolCall::StartService(ServiceName::new("web").expect("a name")))
  );
  assert_eq!(
    ToolCall::parse("stop_service", &json!({"name": "web"}), &offered),
    Ok(ToolCall::StopService(ServiceName::new("web").expect("a name")))
  );
  assert_eq!(
    ToolCall::parse("start_service", &json!({"name": "api"}), &offered),
    Err(ToolCallError::UnknownService("api".to_owned()))
  );
  for wrong in [
    json!({"name": "Web Server"}),
    json!({"name": "web", "port": 80}),
    json!({}),
  ] {
    assert!(
      matches!(
        ToolCall::parse("start_service", &wrong, &offered),
        Err(ToolCallError::Invalid { .. })
      ),
      "{wrong}"
    );
  }
}

#[test]
fn service_status_and_logs_take_a_published_service_and_a_capped_tail() {
  let offered = step(RepositoryAccess::Write, &["web"]);
  let web = || ServiceName::new("web").expect("a name");
  assert_eq!(
    ToolCall::parse("service_status", &json!({"name": "web"}), &offered),
    Ok(ToolCall::ServiceStatus(web()))
  );
  assert_eq!(
    ToolCall::parse("service_logs", &json!({"name": "web"}), &offered),
    Ok(ToolCall::ServiceLogs {
      name: web(),
      tail: DEFAULT_TAIL,
      since: 0
    })
  );
  assert_eq!(
    ToolCall::parse(
      "service_logs",
      &json!({"name": "web", "tail": MAX_TAIL, "since": 42}),
      &offered
    ),
    Ok(ToolCall::ServiceLogs {
      name: web(),
      tail: MAX_TAIL,
      since: 42
    })
  );
  for tool in ["service_status", "service_logs"] {
    assert_eq!(
      ToolCall::parse(tool, &json!({"name": "api"}), &offered),
      Err(ToolCallError::UnknownService("api".to_owned()))
    );
  }
  for wrong in [
    json!({"name": "web", "tail": MAX_TAIL + 1}),
    json!({"name": "web", "tail": 0}),
    json!({"name": "web", "since": -1}),
    json!({"name": "web", "grep": "error"}),
  ] {
    assert!(
      matches!(
        ToolCall::parse("service_logs", &wrong, &offered),
        Err(ToolCallError::Invalid { .. })
      ),
      "{wrong}"
    );
  }
}

#[test]
fn service_tools_list_the_services_and_point_the_agent_at_them() {
  let offered = step(RepositoryAccess::Write, &["web", "api"]);
  let listed = offered.mcp_tools();
  let tool = |name: &str| {
    listed
      .iter()
      .find(|tool| tool["name"] == name)
      .unwrap_or_else(|| panic!("{name} is listed"))
  };
  for name in ["start_service", "stop_service", "service_status", "service_logs"] {
    assert_eq!(
      tool(name)["inputSchema"]["properties"]["name"]["enum"],
      json!(["web", "api"]),
      "{name}"
    );
  }
  assert_eq!(tool("stop_service")["annotations"]["destructiveHint"], json!(true));
  for name in ["service_status", "service_logs"] {
    assert_eq!(tool(name)["annotations"]["readOnlyHint"], json!(true), "{name}");
    assert_eq!(tool(name)["annotations"]["destructiveHint"], json!(false), "{name}");
  }
  assert_eq!(
    tool("service_logs")["inputSchema"]["properties"]["tail"]["maximum"],
    json!(MAX_TAIL)
  );
  let order: Vec<&str> = offered.offered().iter().map(|(tool, _)| tool.as_str()).collect();
  let at = |name: &str| order.iter().position(|tool| *tool == name).expect("offered");
  assert_eq!(at("stop_service"), at("start_service") + 1);
  assert!(at("service_logs") < at("get_task"));
}

#[test]
fn service_prompt_rule_says_to_use_the_service_tools_instead_of_the_shell() {
  let offered = step(RepositoryAccess::Write, &["web", "api"]);
  for (claude, prefix) in [(true, "mcp__Orchester__"), (false, "")] {
    let paragraph = offered.prompt_paragraph(claude);
    for expected in [
      format!("Start the project's servers only with {prefix}start_service(name) (web, api), never by shell"),
      format!("with {prefix}service_status(name)"),
      format!("with {prefix}service_logs(name, tail, since)"),
      format!("stop it with {prefix}stop_service(name), not with shell commands"),
    ] {
      assert!(paragraph.contains(&expected), "{expected}\n{paragraph}");
    }
  }
  let none = step(RepositoryAccess::Write, &[]).prompt_paragraph(true);
  assert!(!none.contains("start_service"), "{none}");
}

fn stacks(values: &[&str]) -> Vec<StackName> {
  values
    .iter()
    .map(|name| StackName::new(name).expect("a name"))
    .collect()
}

#[test]
fn service_stack_tool_comes_with_published_stacks_and_the_service_tools() {
  let offered = step(RepositoryAccess::Write, &["web", "api"]).with_stacks(&stacks(&["full"]));
  assert_eq!(offered.policy(OrchesterTool::StartStack), Some(ToolPolicy::Asks));
  assert!(!auto_approvable("Orchester", "start_stack"));
  let order: Vec<&str> = offered.offered().iter().map(|(tool, _)| tool.as_str()).collect();
  let at = |name: &str| order.iter().position(|tool| *tool == name).expect("offered");
  assert_eq!(at("start_stack") + 1, at("start_service"));
  assert_eq!(
    step(RepositoryAccess::Write, &["web"])
      .with_stacks(&[])
      .policy(OrchesterTool::StartStack),
    None,
    "no stack"
  );
  assert_eq!(
    step(RepositoryAccess::Write, &[])
      .with_stacks(&stacks(&["full"]))
      .policy(OrchesterTool::StartStack),
    None,
    "no service tools"
  );
  let offline = permissions(RepositoryAccess::Write);
  let none = StepTools::for_step(&offline, &[])
    .with_services(&offline, &names(&["web"]))
    .with_stacks(&stacks(&["full"]));
  assert_eq!(none.policy(OrchesterTool::StartStack), None, "network off");
}

#[test]
fn service_stack_tool_takes_only_a_published_stacks_name_and_is_in_the_prompt() {
  let offered = step(RepositoryAccess::Write, &["web"]).with_stacks(&stacks(&["full", "cloud"]));
  assert_eq!(
    ToolCall::parse("start_stack", &json!({"name": "full"}), &offered),
    Ok(ToolCall::StartStack(StackName::new("full").expect("a name")))
  );
  assert_eq!(
    ToolCall::parse("start_stack", &json!({"name": "team"}), &offered),
    Err(ToolCallError::UnknownStack("team".to_owned()))
  );
  assert!(matches!(
    ToolCall::parse("start_stack", &json!({"name": "full", "only": "web"}), &offered),
    Err(ToolCallError::Invalid { .. })
  ));
  let listed = offered.mcp_tools();
  let stack = listed
    .iter()
    .find(|tool| tool["name"] == "start_stack")
    .expect("start_stack is listed");
  assert_eq!(
    stack["inputSchema"]["properties"]["name"]["enum"],
    json!(["full", "cloud"])
  );
  assert_eq!(stack["annotations"]["readOnlyHint"], json!(false));
  let paragraph = offered.prompt_paragraph(false);
  assert!(
    paragraph.contains("Start a whole stack (full, cloud) with start_stack(name)."),
    "{paragraph}"
  );
}
