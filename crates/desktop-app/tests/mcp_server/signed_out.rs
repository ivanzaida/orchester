//! The app's MCP server while this computer is signed out of Orchester Cloud (the sign-in gate, 2026-10-16): every
//! tool refuses with `sign_in_required`, the app's, the domain's and lurq's alike; `sign_in_state` answers; a QA copy
//! or a development build still lets lurq's looking tools see the gate, and the installed app does not; after a sign-in
//! everything answers again. Signing in itself is never a tool.
//!
//! `cargo test -p orchester-app --test mcp_server signed_out_`

use lurq::app::App;
use orchester_app::{
  i18n,
  mcp::{
    chat::ChatServers,
    input_gate::signed_out::{CODE, LOOKS_ON_GATE, looks_on_gate},
  },
};
use orchester_core::id::{ChatId, ChatMessageId, ProjectId};
use orchester_workspace::{build::Build, orchestrator::tools::ChatScope};
use serde_json::{Value, json};

use super::{
  Endpoint, Served,
  http::post,
  support::{Data, signed_in},
  tool_value,
};

/// lurq's tools that read what the window shows, which a QA copy lets see the gate.
const LOOKS: &[&str] = &[
  "lurq_find",
  "lurq_find_by_class",
  "lurq_find_by_id",
  "lurq_inspect",
  "lurq_read_tree",
  "lurq_screenshot",
  "lurq_wait",
];

/// Whether `tool`, called with no arguments, was refused as signed out.
fn refused(served: &mut Served, session: &str, tool: &str) -> bool {
  let (value, error) = tool_value(&served.call(session, tool, &json!({})));
  error && value["code"] == CODE
}

#[test]
fn signed_out_every_tool_is_refused_but_the_state_and_in_qa_the_looking_ones() {
  let mut served = Served::start_signed_out(true);
  let session = served.session();
  let names = served.endpoint().tool_names(&session);
  assert!(names.contains(&"sign_in_state".to_owned()), "{names:?}");
  assert!(
    names.iter().any(|name| name == "create_task"),
    "the domain tools are listed: {names:?}"
  );
  assert!(
    !names
      .iter()
      .any(|name| name.contains("sign_in") && name != "sign_in_state"),
    "no tool signs in: {names:?}"
  );
  for name in &names {
    // A look at the gate that answers is asked once below, with arguments it takes.
    if LOOKS.contains(&name.as_str()) || LOOKS_ON_GATE.contains(&name.as_str()) {
      continue;
    }
    assert_eq!(
      refused(&mut served, &session, name),
      name != "sign_in_state",
      "{name}: refused while signed out unless it is the state"
    );
  }
  let (tree, error) = tool_value(&served.call(&session, "lurq_read_tree", &json!({})));
  assert!(!error, "a QA copy reads the gate: {tree}");
  assert!(
    tree.to_string().contains("screen-sign-in-gate"),
    "the tree is the gate's: {tree}"
  );
  let (state, error) = tool_value(&served.call(&session, "sign_in_state", &json!({})));
  assert!(!error);
  assert_eq!(state, json!({ "signed_in": false, "reason": Value::Null }));
}

#[test]
fn signed_out_the_installed_app_refuses_its_looking_tools_too() {
  let mut served = Served::start_signed_out(true);
  let services = served.launch.services().expect("services").clone();
  // As the installed app binds the session's cloud account: no look at the gate.
  let installed = Build::of(Some("1"), false);
  assert_eq!(installed, Build::Installed);
  served
    .link
    .input_gate()
    .bind_cloud(services.cloud, looks_on_gate(installed));
  let session = served.session();
  for name in LOOKS.iter().chain(LOOKS_ON_GATE) {
    assert!(refused(&mut served, &session, name), "{name}");
  }
  assert!(!refused(&mut served, &session, "sign_in_state"));
}

#[test]
fn signed_out_only_qa_and_development_builds_look_at_the_gate() {
  for (flag, debug, looks) in [
    (Some("1"), false, false),
    (Some("1"), true, true),
    (None, true, true),
    (None, false, true),
    (Some("0"), false, true),
  ] {
    let build = Build::of(flag, debug);
    assert_eq!(looks_on_gate(build), looks, "{flag:?}, debug {debug}: {build:?}");
  }
}

#[test]
fn signed_out_tools_answer_again_after_a_sign_in() {
  let mut served = Served::start_signed_out(true);
  let session = served.session();
  assert!(refused(&mut served, &session, "app_state"));
  assert!(refused(&mut served, &session, "list_projects"));
  let services = served.launch.services().expect("services").clone();
  signed_in::sign_in(&services, served.data.runtime.handle(), &served.data.cloud);
  let (state, error) = tool_value(&served.call(&session, "app_state", &json!({})));
  assert!(!error, "{state}");
  assert!(!refused(&mut served, &session, "list_projects"));
  let (state, _) = tool_value(&served.call(&session, "sign_in_state", &json!({})));
  assert_eq!(state, json!({ "signed_in": true }));

  served.data.cloud.end_token(Some("revoked"), 1_792_147_957_042);
  served
    .data
    .runtime
    .block_on(services.cloud.check_sign_in())
    .expect_err("the cloud ended the token");
  assert!(refused(&mut served, &session, "app_state"), "the gate came back");
  let (state, _) = tool_value(&served.call(&session, "sign_in_state", &json!({})));
  assert_eq!(state, json!({ "signed_in": false, "reason": "revoked" }));
}

#[test]
fn signed_out_the_orchestrators_chat_tools_are_refused_too() {
  let data = Data::new();
  let launch = data.launch_signed_out();
  let services = launch.services().expect("the session starts").clone();
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let scope = ChatScope {
    project: ProjectId::generate(),
    name: "Kontur".to_owned(),
    root: data.dir.path().to_path_buf(),
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
  let call = || {
    let body = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
      "params": { "name": "list_tasks", "arguments": { "project": "Kontur" } } });
    tool_value(&post(
      port,
      &[("Authorization", &endpoint.bearer), ("Mcp-Session-Id", &session)],
      &body,
    ))
  };
  let (refusal, error) = call();
  assert!(error && refusal["code"] == CODE, "{refusal}");
  signed_in::sign_in(&services, data.runtime.handle(), &data.cloud);
  let (answer, _) = call();
  assert_ne!(
    answer["code"], CODE,
    "signed in, the tool is the Orchestrator's again: {answer}"
  );
}
