//! The `ui_*` tools and the QA app as the built-in service `desktop-qa`: a step that may start a QA app is offered
//! `desktop-qa` beside the project's services and the `ui_*` tools, which run unasked; their input is strict; and the
//! `mcp` driver maps each call onto one of the app's lurq tools and reads the app's answer into the common shape.

use orchester_core::{
  orchester_tools::{
    OrchesterTool, QA_SERVICE, StepTools, ToolCall, ToolCallError, ToolPolicy, UiCall,
    ui::{
      UiAction, UiCondition, UiElement, UiRef,
      mcp::{self, UiAnswer},
    },
  },
  permission::NetworkAccess,
  resource::RepositoryAccess,
  service::ServiceName,
};
use serde_json::{Value, json};

use super::{permissions, qa_step};

fn web() -> ServiceName {
  ServiceName::new("web").expect("a name")
}

fn online(repository: RepositoryAccess) -> orchester_core::permission::AgentPermissions {
  let mut online = permissions(repository);
  online.set_network(NetworkAccess::On);
  online
}

/// A writing step on the network of a project that publishes `web`: offered `desktop-qa` and the `ui_*` tools.
fn ui_step() -> StepTools {
  qa_step().with_services(&online(RepositoryAccess::Write), &[web()])
}

fn ui(name: &str, arguments: &Value) -> Result<UiCall, ToolCallError> {
  match ToolCall::parse(name, arguments, &ui_step())? {
    ToolCall::Ui(call) => Ok(call),
    other => panic!("{name} is a ui call, not {other:?}"),
  }
}

#[test]
fn ui_tools_and_desktop_qa_come_with_start_qa_app_and_run_unasked() {
  let step = ui_step();
  for tool in OrchesterTool::UI {
    assert_eq!(step.policy(tool), Some(ToolPolicy::Runs), "{}", tool.as_str());
  }
  assert_eq!(step.policy(OrchesterTool::StartService), Some(ToolPolicy::Asks));
  assert!(step.offers_qa_service());
  // With no project service at all, the built-in one still brings the service tools.
  let alone = qa_step().with_services(&online(RepositoryAccess::Write), &[]);
  assert!(alone.offers_qa_service());
  assert!(ToolCall::parse("start_service", &json!({"name": QA_SERVICE}), &alone).is_ok());
  let listed = alone.mcp_tools();
  let open = listed
    .iter()
    .find(|tool| tool["name"] == "ui_open")
    .expect("ui_open is listed");
  assert_eq!(
    open["inputSchema"]["properties"]["service"]["enum"],
    json!([QA_SERVICE])
  );
  assert_eq!(open["annotations"]["readOnlyHint"], true);
  let act = listed.iter().find(|tool| tool["name"] == "ui_act").expect("ui_act");
  assert_eq!(act["annotations"]["readOnlyHint"], false);
  let prompt = alone.prompt_paragraph(true);
  assert!(
    prompt.contains("mcp__Orchester__ui_open(\"desktop-qa\")") && prompt.contains("mcp__Orchester__ui_snapshot"),
    "{prompt}"
  );

  // A read-only step, or one off the network, may not start a QA app: no desktop-qa and no ui_* tools.
  let read_only = StepTools::for_step(&online(RepositoryAccess::ReadOnly), &[])
    .with_services(&online(RepositoryAccess::ReadOnly), &[web()]);
  assert!(!read_only.offers_qa_service());
  assert!(OrchesterTool::UI.iter().all(|tool| read_only.policy(*tool).is_none()));
  assert_eq!(
    ToolCall::parse("start_service", &json!({"name": QA_SERVICE}), &read_only),
    Err(ToolCallError::UnknownService(QA_SERVICE.to_owned()))
  );
  assert!(
    StepTools::for_step(&permissions(RepositoryAccess::Write), &[])
      .with_services(&permissions(RepositoryAccess::Write), &[web()])
      .offered()
      .iter()
      .all(|(tool, _)| !tool.is_ui_tool())
  );
}

#[test]
fn ui_tools_input_is_strict() {
  assert_eq!(
    ui("ui_open", &json!({"service": "desktop-qa"})),
    Ok(UiCall::Open(ServiceName::new("desktop-qa").expect("a name")))
  );
  assert_eq!(
    ui("ui_open", &json!({"service": "nope"})),
    Err(ToolCallError::UnknownService("nope".to_owned()))
  );
  assert_eq!(ui("ui_snapshot", &Value::Null), Ok(UiCall::Snapshot { query: None }));
  let reference = |value: &str| UiRef::new(value).expect("a ref");
  assert_eq!(
    ui("ui_act", &json!({"ref": "ref_3", "action": "type", "text": "hello"})),
    Ok(UiCall::Act {
      target: Some(reference("ref_3")),
      action: UiAction::Type("hello".to_owned()),
    })
  );
  assert_eq!(
    ui("ui_act", &json!({"action": "key", "text": "Enter"})),
    Ok(UiCall::Act {
      target: None,
      action: UiAction::Key("Enter".to_owned()),
    })
  );
  assert_eq!(
    ui("ui_wait", &json!({"condition": "element", "query": "Saved"})),
    Ok(UiCall::Wait {
      condition: UiCondition::Element("Saved".to_owned()),
      timeout_ms: 10_000,
    })
  );
  assert_eq!(
    ui("ui_logs", &json!({})),
    Ok(UiCall::Logs {
      lines: 200,
      filter: None
    })
  );
  for (name, bad) in [
    ("ui_open", json!({})),
    ("ui_snapshot", json!({"query": ""})),
    ("ui_snapshot", json!({"other": 1})),
    ("ui_act", json!({"action": "click"})),
    ("ui_act", json!({"ref": "ref_1", "action": "drag"})),
    ("ui_act", json!({"ref": "ref_1", "action": "type"})),
    ("ui_act", json!({"ref": "ref_1", "action": "click", "text": "x"})),
    ("ui_act", json!({"ref": "ref 1", "action": "click"})),
    ("ui_screenshot", json!({"ref": "a/b"})),
    ("ui_logs", json!({"lines": 1001})),
    ("ui_wait", json!({"condition": "idle", "query": "x"})),
    ("ui_wait", json!({"condition": "element"})),
    ("ui_wait", json!({"condition": "idle", "timeout_ms": 60_001})),
    ("ui_wait", json!({"condition": "soon"})),
  ] {
    assert!(
      matches!(
        ToolCall::parse(name, &bad, &ui_step()),
        Err(ToolCallError::Invalid { .. })
      ),
      "{name} {bad}"
    );
  }
}

#[test]
fn ui_mcp_driver_maps_each_call_onto_a_lurq_tool() {
  let reference = |value: &str| Some(UiRef::new(value).expect("a ref"));
  let request = |call: UiCall| {
    let request = mcp::request(&call);
    (request.tool, request.arguments)
  };
  assert_eq!(
    request(UiCall::Snapshot { query: None }),
    ("lurq_inspect", json!({"max_nodes": 300, "max_depth": 40}))
  );
  assert_eq!(
    request(UiCall::Act {
      target: reference("ref_2"),
      action: UiAction::Click
    }),
    ("lurq_act", json!({"ref": "ref_2", "action": "invoke"}))
  );
  assert_eq!(
    request(UiCall::Act {
      target: reference("ref_4"),
      action: UiAction::Select("High".to_owned())
    }),
    ("lurq_set_value", json!({"ref": "ref_4", "value": "High"}))
  );
  assert_eq!(
    request(UiCall::Act {
      target: reference("ref_4"),
      action: UiAction::Scroll
    }),
    ("lurq_interact", json!({"action": "scroll_to", "ref": "ref_4"}))
  );
  assert_eq!(
    request(UiCall::Screenshot {
      target: reference("ref_2")
    }),
    ("lurq_screenshot", json!({"ref": "ref_2"}))
  );
  assert_eq!(
    request(UiCall::Wait {
      condition: UiCondition::Idle,
      timeout_ms: 500
    }),
    ("lurq_wait", json!({"timeout_ms": 500}))
  );
}

#[test]
fn ui_mcp_driver_reads_the_apps_answers_into_the_common_shape() {
  let snapshot = UiCall::Snapshot { query: None };
  // lurq answers JSON as pretty text with the same structured content; the text alone reads the same.
  let tree = json!({"window": "main", "truncated": true, "tree": {"ref": "ref_1", "role": "window", "name": "QA",
  "state": {}, "actions": [], "bounds": [0, 0, 10, 10], "children": [
    {"ref": "ref_2", "role": "button", "name": "Save", "id": "save"},
    {"ref": "ref_3", "role": "generic", "name": null},
    {"role": "text"},
  ]}});
  let expected = json!({"tree": {"ref": "ref_1", "role": "window", "name": "QA", "children": [
    {"ref": "ref_2", "role": "button", "name": "Save"},
    {"ref": "ref_3", "role": "generic"},
  ]}, "truncated": true});
  for result in [
    json!({"content": [{"type": "text", "text": tree.to_string()}], "structuredContent": tree}),
    json!({"content": [{"type": "text", "text": tree.to_string()}]}),
  ] {
    assert_eq!(mcp::answer(&snapshot, &result), Ok(UiAnswer::Done(expected.clone())));
  }
  let element = UiElement {
    reference: "ref_1".to_owned(),
    role: "button".to_owned(),
    name: None,
    children: Vec::new(),
  };
  assert_eq!(
    serde_json::to_value(element).expect("JSON"),
    json!({"ref": "ref_1", "role": "button"})
  );

  let wait = UiCall::Wait {
    condition: UiCondition::Element("Saved".to_owned()),
    timeout_ms: 1000,
  };
  let none = json!({"structuredContent": {"matches": [], "truncated": false}, "content": []});
  assert_eq!(mcp::answer(&wait, &none), Ok(UiAnswer::NotYet));
  let shot = UiCall::Screenshot { target: None };
  let saved = json!({"content": [{"type": "text", "text": "PNG saved: C:/r/.orchester/runs/x/screenshots/app-1.png; open it with read_file to see it"}]});
  assert_eq!(
    mcp::answer(&shot, &saved),
    Ok(UiAnswer::Done(
      json!({"screenshot": "C:/r/.orchester/runs/x/screenshots/app-1.png"})
    ))
  );
  let logs = UiCall::Logs { lines: 5, filter: None };
  let empty = json!({"content": [{"type": "text", "text": "(no matching log lines)"}]});
  assert_eq!(mcp::answer(&logs, &empty), Ok(UiAnswer::Done(json!({"lines": []}))));
  let refused = json!({"content": [{"type": "text", "text": "unknown or stale ref \"ref_2\""}], "isError": true});
  assert_eq!(
    mcp::answer(&snapshot, &refused),
    Err("unknown or stale ref \"ref_2\"".to_owned())
  );
}
