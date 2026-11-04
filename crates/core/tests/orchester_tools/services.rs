//! `start_service` and `stop_service`: offered with the project's published services to a step that reaches the
//! network, a start asks and auto-approval never answers it, a stop never asks, and both take only a published
//! service's name.

use orchester_core::{
  orchester_tools::{OrchesterTool, StepTools, ToolCall, ToolCallError, ToolPolicy, auto_approvable},
  permission::NetworkAccess,
  resource::RepositoryAccess,
  service::ServiceName,
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
  assert_eq!(none.policy(OrchesterTool::StartService), None, "network off");
  assert_eq!(none.policy(OrchesterTool::StopService), None, "network off");
  let empty = step(RepositoryAccess::Write, &[]);
  assert_eq!(empty.policy(OrchesterTool::StartService), None, "no service");
  for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
    let offered = step(repository, &["web", "api"]);
    assert_eq!(offered.policy(OrchesterTool::StartService), Some(ToolPolicy::Asks));
    assert_eq!(offered.policy(OrchesterTool::StopService), Some(ToolPolicy::Runs));
  }
  assert!(!auto_approvable("Orchester", "start_service"));
  assert!(auto_approvable("Orchester", "stop_service"));
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
fn service_tools_list_the_services_and_point_the_agent_at_them() {
  let offered = step(RepositoryAccess::Write, &["web", "api"]);
  let listed = offered.mcp_tools();
  let start = listed
    .iter()
    .find(|tool| tool["name"] == "start_service")
    .expect("listed");
  assert_eq!(
    start["inputSchema"]["properties"]["name"]["enum"],
    json!(["web", "api"])
  );
  let stop = listed
    .iter()
    .find(|tool| tool["name"] == "stop_service")
    .expect("listed");
  assert_eq!(stop["annotations"]["destructiveHint"], json!(true));
  let order: Vec<&str> = offered.offered().iter().map(|(tool, _)| tool.as_str()).collect();
  let at = |name: &str| order.iter().position(|tool| *tool == name).expect("offered");
  assert_eq!(at("stop_service"), at("start_service") + 1);
  assert!(at("start_service") < at("get_task"));
  let paragraph = offered.prompt_paragraph(true);
  assert!(
    paragraph.contains("mcp__Orchester__start_service(name) (web, api)"),
    "{paragraph}"
  );
  assert!(paragraph.contains("mcp__Orchester__stop_service(name)"), "{paragraph}");
}
