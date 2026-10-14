use std::num::NonZeroU32;

use orchester_core::orchester_tools::{
  AppDiscovery, AppDiscoveryError, OrchesterTool, ToolCall, ToolPolicy, ToolResult,
};
use serde_json::{Value, json};

use crate::reading;

fn discovery() -> Value {
  json!({"pid":123,"app":"orchester-app","port":4567,"url":"http://127.0.0.1:4567/mcp","token":"stand-in-credential"})
}

fn parse(value: &Value) -> Result<AppDiscovery, AppDiscoveryError> {
  AppDiscovery::parse(
    &serde_json::to_vec(value).expect("JSON"),
    NonZeroU32::new(123).expect("pid"),
    "orchester-app",
  )
}

#[test]
fn call_run_app_input_is_strict_and_both_agents_run_without_approval() {
  let step = reading();
  assert_eq!(step.policy(OrchesterTool::CallRunApp), Some(ToolPolicy::Runs));
  assert!(
    step
      .claude_allow_rules()
      .contains(&"mcp__Orchester__call_run_app".to_owned())
  );
  assert!(step.codex_approved().contains(&"call_run_app"));
  let good = json!({"pid":123,"tool":"lurq_windows","arguments":{}});
  let call = ToolCall::parse("call_run_app", &good, &step).expect("call");
  assert_eq!(call.tool(), OrchesterTool::CallRunApp);
  assert!(!step.call_asks(&call));
  for bad in [
    json!({"pid":0,"tool":"lurq_windows","arguments":{}}),
    json!({"pid":4294967296u64,"tool":"lurq_windows","arguments":{}}),
    json!({"pid":"123","tool":"lurq_windows","arguments":{}}),
    json!({"pid":123,"tool":"","arguments":{}}),
    json!({"pid":123,"tool":"foo bar","arguments":{}}),
    json!({"pid":123,"tool":"lurq_windows","arguments":null}),
    json!({"pid":123,"tool":"lurq_windows","arguments":[]}),
    json!({"pid":123,"tool":"lurq_windows"}),
    json!({"pid":123,"tool":"lurq_windows","arguments":{},"token":"not accepted"}),
  ] {
    assert!(ToolCall::parse("call_run_app", &bad, &step).is_err());
  }
  let listed = step.mcp_tools();
  let tool = listed
    .iter()
    .find(|tool| tool["name"] == "call_run_app")
    .expect("listed");
  assert_eq!(tool["annotations"]["readOnlyHint"], false);
  assert_eq!(tool["inputSchema"]["additionalProperties"], false);
  for claude in [true, false] {
    let prompt = step.prompt_paragraph(claude);
    assert!(prompt.contains(&OrchesterTool::CallRunApp.called(claude)));
    assert!(prompt.contains("instead of running tools/lurq-mcp-client.py"));
  }
}

#[test]
fn call_run_app_discovery_refuses_another_pid_app_and_non_loopback_endpoints() {
  for (field, value, expected) in [
    ("pid", json!(124), AppDiscoveryError::Identity),
    ("pid", json!("123"), AppDiscoveryError::Identity),
    ("app", json!("orchester"), AppDiscoveryError::Identity),
    ("url", json!("http://example.com/mcp"), AppDiscoveryError::Endpoint),
    ("url", json!("http://127.0.0.1:4567/other"), AppDiscoveryError::Endpoint),
    ("url", json!("http://127.0.0.1:4568/mcp"), AppDiscoveryError::Endpoint),
    ("port", json!(0), AppDiscoveryError::Invalid),
    ("port", json!(65536), AppDiscoveryError::Invalid),
    ("token", json!(""), AppDiscoveryError::Invalid),
    ("token", json!("  "), AppDiscoveryError::Invalid),
  ] {
    let mut value_in_file = discovery();
    value_in_file[field] = value;
    assert_eq!(parse(&value_in_file).expect_err("refused"), expected);
  }
  let bad = b"{invalid stand-in-credential";
  let error = AppDiscovery::parse(bad, NonZeroU32::new(123).expect("pid"), "orchester-app").expect_err("invalid");
  assert!(!error.to_string().contains("stand-in-credential"));
}

#[test]
fn call_run_app_token_is_redacted_in_text_structured_content_keys_and_debug() {
  let discovery = parse(&discovery()).expect("discovery");
  assert!(!format!("{discovery:?}").contains("stand-in-credential"));
  let answer = ToolResult::from_app(json!({
    "content":[{"type":"text","text":"echo stand-in-credential"}],
    "structuredContent":{"stand-in-credential":{"nested":["stand-in-credential",123]}},
    "isError":true
  }))
  .redacted(std::slice::from_ref(discovery.token()));
  assert!(answer.is_error());
  for exposed in [
    answer.text().to_owned(),
    answer.mcp().to_string(),
    format!("{answer:?}"),
  ] {
    assert!(!exposed.contains("stand-in-credential"));
  }
  assert_eq!(answer.mcp()["structuredContent"]["<redacted>"]["nested"][1], 123);
}

#[test]
fn call_run_app_structured_result_survives_the_relay() {
  let result = json!({"content":[{"type":"text","text":"first"},{"type":"text","text":"second"}],
    "structuredContent":{"pid":123,"ready":true},"isError":false});
  let answer = ToolResult::from_app(result.clone());
  assert_eq!(answer.mcp(), result);
  assert!(!answer.is_error());
  assert!(answer.text().contains("first\nsecond"));
  assert!(ToolResult::from_app(json!({"content":[],"structuredContent":{"large":"x".repeat(100_000)}})).is_error());
}

#[test]
fn call_run_app_redaction_preserves_the_structured_answer_limit() {
  let value = std::sync::Arc::new(orchester_core::secret::SecretValue::new("x".to_owned()).expect("secret"));
  let secret = orchester_core::command::SecretVariable::new("short", value);
  let result = ToolResult::from_app(json!({"content":[],"structuredContent":{"value":"x".repeat(20_000)}}));
  assert!(!result.is_error());
  let result = result.redacted(&[secret]);
  assert!(result.is_error());
  assert!(result.mcp().to_string().len() < orchester_core::orchester_tools::MAX_ANSWER_BYTES);
}

#[test]
fn call_run_app_start_qa_app_input_is_strict_and_asks() {
  use orchester_core::{
    orchester_tools::{StartedQaApp, qa_name},
    resource::StepKey,
    run::RunNumber,
  };
  let step = reading();
  let call = ToolCall::parse("start_qa_app", &json!({"scenario":"task-states"}), &step).expect("call");
  assert_eq!(call.tool(), OrchesterTool::StartQaApp);
  assert!(step.call_asks(&call), "it runs the worktree's code outside the sandbox");
  let ToolCall::StartQaApp(start) = call else {
    panic!("a QA app start");
  };
  let name = qa_name(
    RunNumber::new(NonZeroU32::new(3).expect("number")),
    &StepKey::new("verify").expect("key"),
  );
  assert_eq!(name, "run-3-verify");
  assert_eq!(
    start.script_args(&name),
    ["-Name", "run-3-verify", "-Scenario", "task-states"]
  );
  let fake = json!({"scenario":"live","fake_agents":true,"no_runner":null});
  let Ok(ToolCall::StartQaApp(fake)) = ToolCall::parse("start_qa_app", &fake, &step) else {
    panic!("a QA app start with stand-in agents");
  };
  assert_eq!(
    fake.script_args("run-3-verify").last().map(String::as_str),
    Some("-FakeAgents")
  );
  for bad in [
    json!({}),
    json!({"scenario":""}),
    json!({"scenario":"Tasks"}),
    json!({"scenario":"../tasks"}),
    json!({"scenario":"tasks","fake_agents":true,"no_runner":true}),
    json!({"scenario":"tasks","name":"other"}),
  ] {
    assert!(ToolCall::parse("start_qa_app", &bad, &step).is_err(), "{bad}");
  }
  // Without a pid, call_run_app calls the step's QA app.
  let call = ToolCall::parse("call_run_app", &json!({"tool":"app_state","arguments":{}}), &step).expect("call");
  let ToolCall::App(call) = call else {
    panic!("an app call");
  };
  assert_eq!(call.pid(), None);
  let printed = "Compiling...\nscenario tasks\npid 4242\nmcp C:/repo/.tmp/qa/run-3-verify/mcp\n\
                 discovery C:/repo/.tmp/qa/run-3-verify/mcp/4242.json\n";
  let started = StartedQaApp::read(printed).expect("an app");
  assert_eq!(started.pid.get(), 4242);
  assert!(started.discovery.ends_with("4242.json"));
  assert_eq!(StartedQaApp::read("pid nothing\n"), None);
}
