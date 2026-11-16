//! The MCP server tools in a team project against a fake team server (`docs/design/team-configuration-live.md`): a new
//! server is created on the team server as v1, and an update publishes the next version there, at once; the server's
//! answer is what the team's configuration in memory holds next, and nothing is written to the project's database.
//! A refusal, and a server that cannot be reached, leave nothing changed. A URL with a query or a fragment, which would
//! carry a key to every member, is refused and nothing is sent. A member changes none of it. Removing is in
//! [`super::mcp_server_remove`].

use std::sync::Arc;

use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore};
use orchester_workspace::team_config::TeamConfig;
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::created::{admin, created};
use crate::{
  Checkout, Standing,
  fixture::Config,
  http::{Request, respond},
  team_server::{Extra, TeamServer},
};

/// What the fake server saw, in order, and what each request carried.
#[derive(Default)]
pub(super) struct Seen {
  pub(super) calls: Vec<String>,
  pub(super) created: Vec<Value>,
  pub(super) published: Vec<Value>,
  pub(super) patched: Vec<Value>,
}

/// How the fake server answers: `None` accepts, `Some((status, code))` refuses.
#[derive(Default)]
pub(super) struct Answers {
  pub(super) create: Option<(u16, &'static str)>,
  pub(super) publish: Option<(u16, &'static str)>,
  pub(super) archive: Option<(u16, &'static str)>,
  /// What `used-by` answers; the route is not found when `None`.
  pub(super) used_by: Option<Value>,
}

pub(super) fn start(answers: Answers) -> (TeamServer, Arc<Mutex<Seen>>) {
  let seen = Arc::new(Mutex::new(Seen::default()));
  let recorded = seen.clone();
  let answers = Arc::new(answers);
  let extra: Extra = Arc::new(move |state, request, stream| {
    let project = state.lock().project;
    let Some(route) = request.path.split('?').next().and_then(|path| {
      path
        .strip_prefix(&format!("/api/v1/projects/{project}/resources"))
        .map(str::to_owned)
    }) else {
      return false;
    };
    let mut seen = recorded.lock();
    match (request.method.as_str(), route.as_str()) {
      ("POST", "") => {
        seen.calls.push("create".to_owned());
        seen.created.push(request.json());
        reply(stream, answers.create, || created(request));
      }
      ("POST", versions) if versions.ends_with("/versions") => {
        seen.calls.push("publish".to_owned());
        seen.published.push(request.json());
        reply(
          stream,
          answers.publish,
          || json!({ "version": published(request, versions) }),
        );
      }
      ("PATCH", _) => {
        seen.calls.push("archive".to_owned());
        seen.patched.push(request.json());
        reply(stream, answers.archive, || json!({ "resource": archived(&route) }));
      }
      ("GET", used) if used.ends_with("/used-by") => {
        seen.calls.push("used-by".to_owned());
        match &answers.used_by {
          Some(used_by) => respond(stream, 200, used_by),
          None => respond(
            stream,
            404,
            &json!({"error": "not_found", "message": "No such resource."}),
          ),
        }
      }
      _ => return false,
    }
    true
  });
  (TeamServer::start_with(Config::new("Say hello."), Some(extra)), seen)
}

fn reply(stream: &mut std::net::TcpStream, refusal: Option<(u16, &str)>, accepted: impl FnOnce() -> Value) {
  match refusal {
    Some((status, code)) => respond(stream, status, &json!({"error": code, "message": "Refused."})),
    None => respond(stream, 201, &accepted()),
  }
}

fn attribution() -> Value {
  json!({ "by": { "id": "01a183cf-b73c-71db-8a62-bd509770cd4e", "display_name": "Dana" }, "at": 1 })
}

/// The version the server numbers after the one `request` is based on.
fn published(request: &Request, route: &str) -> Value {
  let publish = request.json();
  let resource = route.trim_start_matches('/').split('/').next().unwrap_or_default();
  json!({
    "resource": resource, "number": publish["based_on"].as_u64().unwrap_or_default() + 1, "body": publish["body"],
    "hash": "b".repeat(64), "references": [], "based_on": publish["based_on"], "published": attribution(),
    "widens": false
  })
}

fn archived(route: &str) -> Value {
  let resource = route.trim_start_matches('/');
  json!({
    "id": resource, "kind": "mcp_server", "slug": "docs", "name": "docs", "archived": true, "revision": 2,
    "latest": 1, "created": attribution(), "updated": attribution()
  })
}

pub(super) fn docs() -> Value {
  json!({
    "project": "Kontur", "name": "docs", "program": "npx", "args": ["-y", "@acme/docs"],
    "variable_names": ["TOKEN"], "exempt_tools": ["search"],
  })
}

fn api_key_header() -> Value {
  json!({ "header": "X-API-Key", "secret": "ISSUES_KEY" })
}

fn change(fields: Value) -> Value {
  let mut args = json!({ "project": "Kontur", "server": "docs" });
  if let (Value::Object(args), Value::Object(fields)) = (&mut args, fields) {
    args.extend(fields);
  }
  args
}

/// The team's configuration as the checkout's memory holds it.
fn held(checkout: &Checkout) -> std::sync::Arc<TeamConfig> {
  checkout
    .workspace
    .team_read(checkout.root())
    .config()
    .cloned()
    .expect("the team's configuration was read")
}

/// How many MCP servers the team has, as memory holds it.
fn count(checkout: &Checkout) -> usize {
  held(checkout).resources(ResourceKind::McpServer).len()
}

/// The team MCP server `docs` of the checkout, and its latest version's number, as memory holds it.
fn docs_held(checkout: &Checkout) -> (ResourceId, u32) {
  let held = held(checkout);
  let server = held.resources(ResourceKind::McpServer)[0];
  (server.id, server.number().expect("a version").get())
}

/// Nothing of the team's MCP servers is in the project's database: the server holds them.
fn none_written(checkout: &Checkout) {
  assert!(
    checkout
      .db()
      .resources(ResourceKind::McpServer)
      .expect("the servers")
      .is_empty()
  );
}

#[test]
fn team_mcp_a_new_mcp_server_is_created_as_v1_once_the_server_accepts_it() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);

  let made = checkout.call("add_mcp_server", &docs()).expect("created");

  assert_eq!(made["server"]["published_version"], 1, "{made}");
  assert_eq!(made["server"]["team"], true, "{made}");
  assert_eq!(made["server"]["launch"]["variable_names"], json!(["TOKEN"]), "{made}");
  assert!(made["server"].get("waiting_for_server").is_none(), "{made}");
  let seen = seen.lock();
  assert_eq!(seen.calls, ["create"]);
  assert_eq!(seen.created[0]["kind"], "mcp_server");
  assert_eq!(
    seen.created[0]["version"]["body"],
    json!({
      "name": "docs",
      "launch": { "kind": "program", "program": "npx", "args": ["-y", "@acme/docs"], "variables": ["TOKEN"] },
      "exempt_tools": ["search"],
    }),
    "the body holds names only"
  );
  assert_eq!(count(&checkout), 1);
  assert_eq!(docs_held(&checkout).1, 1);
  none_written(&checkout);
}

#[test]
fn team_mcp_a_new_mcp_server_the_team_server_refuses_is_not_created() {
  let (server, seen) = start(Answers {
    create: Some((403, "not_admin")),
    ..Answers::default()
  });
  let checkout = admin(&server);

  let refused = checkout.call("add_mcp_server", &docs()).expect_err("refused");

  assert_eq!(refused["error"]["code"], "not_admin", "{refused}");
  assert_eq!(seen.lock().calls, ["create"]);
  assert_eq!(count(&checkout), 0, "nothing shows as created");
  none_written(&checkout);
}

#[test]
fn team_mcp_a_new_mcp_server_is_not_made_while_the_team_server_cannot_be_reached() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  server.set_down(true);

  let refused = checkout.call("add_mcp_server", &docs()).expect_err("refused");

  assert_eq!(refused["error"]["code"], "team_unreachable", "{refused}");
  assert!(seen.lock().calls.is_empty());
  assert_eq!(count(&checkout), 0, "there is no outbox");
  none_written(&checkout);
}

#[test]
fn team_mcp_a_member_changes_no_mcp_server() {
  let (server, seen) = start(Answers::default());
  let member = Standing {
    role: "member",
    widens: true,
  };
  let checkout = Checkout::standing(Some(&server), (&[], member), |_| {});

  assert_eq!(checkout.refused("add_mcp_server", &docs()), "not_admin");

  assert!(
    seen.lock().calls.is_empty(),
    "this computer refused before the server was asked"
  );
  assert_eq!(count(&checkout), 0);
}

#[test]
fn team_mcp_an_update_publishes_the_next_version_on_the_team_server() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");

  let same = checkout
    .call("update_mcp_server", &change(json!({ "exempt_tools": ["search"] })))
    .expect("unchanged");
  assert_eq!(same["unchanged"], true, "{same}");
  assert_eq!(
    seen.lock().calls,
    ["create"],
    "a change that changes nothing publishes nothing"
  );

  let changed = checkout
    .call(
      "update_mcp_server",
      &change(json!({ "args": ["-y", "@acme/docs", "--lang", "en"] })),
    )
    .expect("published");

  assert_eq!(changed["server"]["published_version"], 2, "{changed}");
  assert_eq!(
    changed["server"]["launch"]["args"],
    json!(["-y", "@acme/docs", "--lang", "en"]),
    "{changed}"
  );
  assert!(changed["server"].get("waiting_for_server").is_none(), "{changed}");
  let seen = seen.lock();
  assert_eq!(seen.calls, ["create", "publish"]);
  assert_eq!(seen.published[0]["based_on"], 1);
  assert_eq!(
    seen.published[0]["body"]["launch"]["args"],
    json!(["-y", "@acme/docs", "--lang", "en"])
  );
  drop(seen);
  assert_eq!(
    docs_held(&checkout).1,
    2,
    "memory holds the version the server numbered"
  );
  none_written(&checkout);
}

#[test]
fn team_mcp_an_update_is_refused_offline_and_when_the_server_refuses_it() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");

  server.set_down(true);
  let refused = checkout
    .call("update_mcp_server", &change(json!({ "args": ["--other"] })))
    .expect_err("refused");
  assert_eq!(refused["error"]["code"], "team_unreachable", "{refused}");
  assert_eq!(seen.lock().calls, ["create"]);
  assert_eq!(docs_held(&checkout).1, 1, "nothing waits to be sent");

  let (server, seen) = start(Answers {
    publish: Some((409, "archived")),
    ..Answers::default()
  });
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");
  let refused = checkout
    .call("update_mcp_server", &change(json!({ "args": ["--other"] })))
    .expect_err("refused");
  assert_eq!(refused["error"]["code"], "team_server_refused", "{refused}");
  assert_eq!(seen.lock().calls, ["create", "publish"]);
  assert_eq!(docs_held(&checkout).1, 1, "the team's version stays as it was");
  none_written(&checkout);
}

#[test]
fn team_mcp_a_url_with_a_key_in_it_is_refused_and_nothing_is_stored_or_sent() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  let remote =
    |url: &str| json!({ "project": "Kontur", "name": "issues", "url": url, "secret_headers": [api_key_header()] });

  for url in [
    "https://mcp.example.org/x?key=sk-live-123",
    "https://mcp.example.org/x#sk-live-123",
  ] {
    let refused = checkout.call("add_mcp_server", &remote(url)).expect_err("refused");
    assert_eq!(refused["error"]["code"], "team_value_refused", "{url}: {refused}");
    // The tests' translator answers the key of the message, which is where the words point to a secret header.
    assert!(refused.to_string().contains("teamMcpUrlQuery"), "{refused}");
    assert!(!refused.to_string().contains("sk-live-123"), "{refused}");
  }
  assert_eq!(count(&checkout), 0);
  assert!(seen.lock().calls.is_empty(), "nothing reached the team server");

  checkout
    .call("add_mcp_server", &remote("https://mcp.example.org/x"))
    .expect("a URL without a query is taken");
  let changed = json!({ "project": "Kontur", "server": "issues", "url": "https://mcp.example.org/x?key=sk-live-123" });
  let refused = checkout.call("update_mcp_server", &changed).expect_err("refused");
  assert_eq!(refused["error"]["code"], "team_value_refused", "{refused}");
  assert!(!refused.to_string().contains("sk-live-123"), "{refused}");
  assert_eq!(seen.lock().calls, ["create"], "the update was not sent");
  assert!(seen.lock().created[0].to_string().contains("secret_headers"));
  assert!(!seen.lock().created[0].to_string().contains('?'));
}

#[test]
fn team_mcp_a_secret_header_names_its_secret_and_prefix_and_a_variable_must_be_a_secret_name() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  let remote = |headers: Value| json!({ "project": "Kontur", "name": "issues", "url": "https://mcp.example.org/x", "secret_headers": headers });

  // What a person may have pasted in a name or a prefix field is never repeated.
  for (headers, key) in [
    (
      json!([{ "header": "Authorization", "secret": "ghp_live_123" }]),
      "teamMcpSecretName",
    ),
    (
      json!([{ "header": "Authorization", "secret": "GITHUB_TOKEN", "prefix": "ghp_live_1234567890123456789012345678901234567890" }]),
      "teamMcpHeaderPrefix",
    ),
  ] {
    let refused = checkout.call("add_mcp_server", &remote(headers)).expect_err("refused");
    assert_eq!(refused["error"]["code"], "invalid_server", "{refused}");
    assert!(refused.to_string().contains(key), "{refused}");
    assert!(!refused.to_string().contains("ghp_live"), "{refused}");
  }
  let lowercase = json!({ "project": "Kontur", "name": "docs", "program": "npx", "variable_names": ["token"] });
  let refused = checkout.call("add_mcp_server", &lowercase).expect_err("refused");
  assert!(refused.to_string().contains("teamMcpVariableNotSecret"), "{refused}");
  assert_eq!(count(&checkout), 0);
  assert!(seen.lock().calls.is_empty(), "nothing reached the team server");

  let made = checkout
    .call(
      "add_mcp_server",
      &remote(json!([{ "header": "Authorization", "secret": "GITHUB_TOKEN", "prefix": "Bearer " }])),
    )
    .expect("created");
  let headers = json!([{ "header": "Authorization", "secret": "GITHUB_TOKEN", "prefix": "Bearer " }]);
  assert_eq!(made["server"]["launch"]["secret_headers"], headers, "{made}");
  assert_eq!(
    seen.lock().created[0]["version"]["body"]["launch"]["secret_headers"],
    headers
  );

  // A field left out keeps the headers, secrets and prefixes of the version in use.
  let renamed = json!({ "project": "Kontur", "server": "issues", "exempt_tools": ["search"] });
  let kept = checkout.call("update_mcp_server", &renamed).expect("updated");
  assert_eq!(kept["server"]["launch"]["secret_headers"], headers, "{kept}");
}
