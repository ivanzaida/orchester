//! The app's MCP server while nobody is signed in to Orchester Cloud (the person's decision, 2026-10-28): the local
//! tools answer, `sign_in_state` says so, a project an account holds answers `sign_in_required`, signing in itself is
//! never a tool, and the Orchestrator's chat follows the same rule.
//!
//! `cargo test -p orchester-app --test mcp_server signed_out_`

use lurq::app::App;
use orchester_app::{i18n, mcp::chat::ChatServers};
use orchester_core::id::{ChatId, ChatMessageId};
use orchester_workspace::orchestrator::tools::ChatScope;
use serde_json::{Value, json};

use super::{
  Endpoint, Served,
  http::post,
  support::{Data, signed_in},
  tool_value,
};

const SIGN_IN_REQUIRED: &str = "sign_in_required";

#[test]
fn signed_out_local_tools_answer_and_no_tool_signs_in() {
  let mut served = Served::start(true);
  let session = served.session();
  let names = served.endpoint().tool_names(&session);
  assert!(
    !names
      .iter()
      .any(|name| name.contains("sign_in") && name != "sign_in_state"),
    "no tool signs in: {names:?}"
  );
  for tool in ["app_state", "list_projects", "lurq_windows"] {
    let (value, error) = tool_value(&served.call(&session, tool, &json!({})));
    assert!(!error, "{tool}: {value}");
  }
  let (state, error) = tool_value(&served.call(&session, "sign_in_state", &json!({})));
  assert!(!error);
  assert_eq!(state, json!({ "signed_in": false, "reason": Value::Null }));
}

#[test]
fn signed_out_a_project_an_account_holds_answers_sign_in_required() {
  let mut served = Served::start(true);
  let services = served.launch.services().expect("services").clone();
  signed_in::sign_in(&services, served.data.runtime.handle(), &served.data.cloud);
  let project = served.data.add_kontur(&services.workspace);
  let id = project.project.id().to_string();
  let session = served.session();
  let (listed, error) = tool_value(&served.call(&session, "list_projects", &json!({})));
  assert!(!error && listed.to_string().contains(&id), "its own: {listed}");

  served
    .data
    .runtime
    .block_on(services.cloud.sign_out())
    .expect("signed out");
  let (listed, _) = tool_value(&served.call(&session, "list_projects", &json!({})));
  assert!(!listed.to_string().contains(&id), "it waits, out of the list: {listed}");
  for named in ["Kontur", "kontur", id.as_str()] {
    for tool in ["get_project", "list_tasks"] {
      let (refusal, error) = tool_value(&served.call(&session, tool, &json!({ "project": named })));
      assert!(error, "{tool} of {named}: {refusal}");
      assert_eq!(refusal["code"], SIGN_IN_REQUIRED, "{tool} of {named}: {refusal}");
    }
  }
  let (refusal, error) = tool_value(&served.call(&session, "get_project", &json!({ "project": "Nothing" })));
  assert!(error);
  assert_eq!(refusal["code"], "project_not_found", "what no account holds: {refusal}");

  let (state, _) = tool_value(&served.call(&session, "sign_in_state", &json!({})));
  assert_eq!(state, json!({ "signed_in": false, "reason": "signed_out" }));
}

#[test]
fn signed_out_a_project_of_this_computers_answers_with_nobody_signed_in() {
  let mut served = Served::start(true);
  let services = served.launch.services().expect("services").clone();
  served.data.add_kontur(&services.workspace);
  let session = served.session();
  let (project, error) = tool_value(&served.call(&session, "get_project", &json!({ "project": "Kontur" })));
  assert!(!error, "{project}");
}

#[test]
fn signed_out_a_revoked_sign_in_says_why() {
  let mut served = Served::start(true);
  let services = served.launch.services().expect("services").clone();
  signed_in::sign_in(&services, served.data.runtime.handle(), &served.data.cloud);
  let session = served.session();
  let (state, _) = tool_value(&served.call(&session, "sign_in_state", &json!({})));
  assert_eq!(state, json!({ "signed_in": true }));

  served.data.cloud.end_token(Some("revoked"), 1_792_147_957_042);
  served
    .data
    .runtime
    .block_on(services.cloud.check_sign_in())
    .expect_err("the cloud ended the token");
  let (state, _) = tool_value(&served.call(&session, "sign_in_state", &json!({})));
  assert_eq!(state, json!({ "signed_in": false, "reason": "revoked" }));
  let (answer, error) = tool_value(&served.call(&session, "app_state", &json!({})));
  assert!(!error, "the tools go on: {answer}");
}

#[test]
fn signed_out_the_orchestrators_chat_follows_the_same_rule() {
  let data = Data::new();
  let launch = data.launch();
  let services = launch.services().expect("the session starts").clone();
  let open = data.add_kontur(&services.workspace);
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let scope = ChatScope {
    project: open.project.id(),
    name: "Kontur".to_owned(),
    root: open.repo_root,
    chat: ChatId::generate(),
    on_its_own: false,
  };
  // Its servers stop when it goes.
  let servers = ChatServers::default();
  let tools = servers
    .serve_turn(&services, app.i18n().clone(), (scope, ChatMessageId::generate()))
    .expect("the turn's tools are served");
  let port = tools
    .url
    .rsplit_once(':')
    .and_then(|(_, rest)| rest.trim_end_matches("/mcp").parse().ok())
    .expect("a loopback address with a port");
  let endpoint = Endpoint {
    port,
    bearer: format!("Bearer {}", tools.token.expose()),
  };
  let session = endpoint.session_on("2025-06-18");
  let body = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
    "params": { "name": "list_tasks", "arguments": { "project": "Kontur" } } });
  let (answer, error) = tool_value(&post(
    port,
    &[("Authorization", &endpoint.bearer), ("Mcp-Session-Id", &session)],
    &body,
  ));
  assert!(!error, "its own project answers signed out: {answer}");
}
