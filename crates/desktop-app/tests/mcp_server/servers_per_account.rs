//! Team servers per Orchester Cloud account (the person's decision, 2026-10-16) through the domain tools and the
//! Orchestrator chat: a checkout Bohdan linked through his server and forgot, which Olena opens on the same computer,
//! reaches no server of Bohdan's. Its task is refused as on a server not in the list, by the app's MCP server and by
//! the chat's alike, and nothing reaches the server.
//!
//! `cargo test -p orchester-app --test mcp_server servers_per_account_`

use lurq::app::App;
use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
};
use orchester_app::{i18n, mcp::chat::ChatServers, services::Services};
use orchester_core::id::{ChatId, ChatMessageId, IdempotencyKey, ProjectId};
use orchester_workspace::{
  orchestrator::tools::ChatScope,
  sharing::{CodeRequest, Trust, project::LinkFirst},
};
use serde_json::{Value, json};

use super::{
  Endpoint, Served,
  http::post,
  server_support::TestServer,
  support::{
    add_project,
    signed_in::account_cloud::{OTHER_EMAIL, PASSWORD},
  },
  tool_value,
};

/// Bohdan joins `server` from the session of `services` with a code, as its first admin.
fn join(served: &Served, services: &Services, server: &TestServer) {
  let address = ServerAddress::parse(&server.url()).expect("address");
  let request = CodeRequest {
    code: server.code_for("bohdan", true),
    username: Some("bohdan".into()),
    display_name: Some("Bohdan Koval".into()),
    // The setup code, which needs no two-factor identity.
    proof: None,
  };
  served
    .data
    .runtime
    .block_on(
      services
        .servers
        .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request),
    )
    .expect("join with a code");
}

/// Calls `create_task` in Notes on the Orchestrator chat's tools of a turn in Notes, at `root`.
fn create_in_chat(services: &Services, project: ProjectId, root: &std::path::Path) -> (Value, bool) {
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let scope = ChatScope {
    project,
    name: "Notes".to_owned(),
    root: root.to_path_buf(),
    chat: ChatId::generate(),
    on_its_own: false,
  };
  // Its servers stop when it goes.
  let servers = ChatServers::default();
  let tools = servers
    .serve_turn(services, app.i18n().clone(), (scope, ChatMessageId::generate()))
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
    "params": { "name": "create_task", "arguments": { "project": "Notes", "title": "Write the notes" } } });
  tool_value(&post(
    port,
    &[("Authorization", &endpoint.bearer), ("Mcp-Session-Id", &session)],
    &body,
  ))
}

fn not_found(refusal: &Value) -> bool {
  refusal["code"] == "shared_server" && refusal.to_string().contains("no longer in")
}

#[test]
fn servers_per_account_another_accounts_server_is_not_found_by_the_tools_or_the_chat() {
  let mut served = Served::start(false);
  let services = served.launch.services().expect("the session starts").clone();
  let workspace = served.launch.workspace().expect("the session starts").clone();
  let server = TestServer::start();
  join(&served, &services, &server);
  let notes_there = server.create_project("notes", None);
  let root = served.data.repo("repo-notes");
  let notes = ProjectId::generate();
  add_project(&workspace, (notes, "NOT", "Notes"), &root);
  let entry = services.servers.entries().expect("entries").remove(0);
  let link = LinkFirst {
    slug: "notes".into(),
    remote: None,
    key: IdempotencyKey::generate(),
    write_discovery: false,
  };
  served
    .data
    .runtime
    .block_on(services.servers.link_first(&entry, &root, link))
    .expect("Bohdan links Notes");
  workspace.forget(notes).expect("Bohdan forgets Notes");

  served.data.runtime.block_on(async {
    services.cloud.sign_out().await.expect("signed out");
    services
      .cloud
      .sign_in(OTHER_EMAIL.into(), Credential::new(PASSWORD.into()))
      .await
      .expect("Olena signs in");
  });
  workspace.open(&root).expect("Olena opens the checkout");
  assert!(services.servers.entries().expect("entries").is_empty());

  let session = served.session();
  let (refusal, error) = tool_value(&served.call(
    &session,
    "create_task",
    &json!({ "project": "Notes", "title": "Write the notes" }),
  ));
  assert!(error && not_found(&refusal), "the app's MCP server: {refusal}");
  let (refusal, error) = create_in_chat(&services, notes, &root);
  assert!(error && not_found(&refusal), "the Orchestrator chat: {refusal}");
  assert!(server.task_keys(&notes_there).is_empty(), "nothing reached the server");
}
