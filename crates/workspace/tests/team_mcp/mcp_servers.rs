//! The MCP server tools in a team project against a fake team server: a new server is created as v1 once the server
//! accepts it and taken back when it refuses; an update publishes the next version, which waits in the outbox while the
//! server cannot be reached and never replaces an unpublished draft; a URL with a query or a fragment, which would
//! carry a key to every member, is refused and nothing is stored or sent. A member changes none of it. Removing is in
//! [`super::mcp_server_remove`].

use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  resource::{Draft, ResourceKind, TeamMcpServer},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::PublishState;
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

fn change(fields: Value) -> Value {
  let mut args = json!({ "project": "Kontur", "server": "docs" });
  if let (Value::Object(args), Value::Object(fields)) = (&mut args, fields) {
    args.extend(fields);
  }
  args
}

fn count(checkout: &Checkout) -> usize {
  checkout
    .db()
    .resources(ResourceKind::McpServer)
    .expect("the servers")
    .len()
}

/// The id of the team MCP server `docs` of the checkout.
fn docs_id(checkout: &Checkout) -> ResourceId {
  checkout
    .db()
    .resources(ResourceKind::McpServer)
    .expect("the servers")
    .remove(0)
    .id()
}

fn versions(checkout: &Checkout) -> usize {
  checkout
    .db()
    .versions::<TeamMcpServer>(docs_id(checkout))
    .expect("versions")
    .len()
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
  assert_eq!(versions(&checkout), 1);
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());
  let id = docs_id(&checkout);
  assert!(
    checkout
      .db()
      .draft::<TeamMcpServer>(id)
      .expect("a draft read")
      .is_none()
  );
}

#[test]
fn team_mcp_a_new_mcp_server_the_team_server_refuses_is_not_created() {
  let (server, seen) = start(Answers {
    create: Some((403, "not_admin")),
    ..Answers::default()
  });
  let checkout = admin(&server);

  let refused = checkout.call("add_mcp_server", &docs()).expect_err("refused");

  assert_eq!(refused["error"]["code"], "team_server_refused", "{refused}");
  assert_eq!(seen.lock().calls, ["create"]);
  assert_eq!(count(&checkout), 0, "nothing shows as created");
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());
}

#[test]
fn team_mcp_a_new_mcp_server_made_offline_waits_in_the_outbox() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  server.set_down(true);

  let made = checkout.call("add_mcp_server", &docs()).expect("created");

  assert_eq!(
    made["server"]["waiting_for_server"]["code"], "team_server_unreachable",
    "{made}"
  );
  assert!(seen.lock().calls.is_empty());
  assert_eq!(count(&checkout), 1);
  let waiting = checkout.db().waiting_publishes().expect("the outbox");
  assert_eq!(waiting.len(), 1);
  assert_eq!(waiting[0].state, PublishState::Waiting);
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
  assert_eq!(versions(&checkout), 2);
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());
}

#[test]
fn team_mcp_an_update_made_offline_waits_and_one_the_server_refuses_stays_unsent() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");

  server.set_down(true);
  let waiting = checkout
    .call("update_mcp_server", &change(json!({ "args": ["--other"] })))
    .expect("kept here");
  assert_eq!(
    waiting["server"]["waiting_for_server"]["code"], "team_server_unreachable",
    "{waiting}"
  );
  assert_eq!(checkout.db().waiting_publishes().expect("the outbox").len(), 1);
  assert_eq!(seen.lock().calls, ["create"]);

  let (server, seen) = start(Answers {
    publish: Some((403, "not_admin")),
    ..Answers::default()
  });
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");
  let refused = checkout
    .call("update_mcp_server", &change(json!({ "args": ["--other"] })))
    .expect_err("refused");
  assert_eq!(refused["error"]["code"], "team_server_refused", "{refused}");
  assert_eq!(seen.lock().calls, ["create", "publish"]);
  let outbox = checkout.db().waiting_publishes().expect("the outbox");
  assert_eq!(
    outbox[0].state,
    PublishState::Refused {
      code: "not_admin".to_owned()
    },
    "the version stays here, unsent, for a person to decide"
  );
}

#[test]
fn team_mcp_a_url_with_a_key_in_it_is_refused_and_nothing_is_stored_or_sent() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  let remote =
    |url: &str| json!({ "project": "Kontur", "name": "issues", "url": url, "secret_headers": ["X-API-Key"] });

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
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());

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
fn team_mcp_an_update_does_not_replace_an_unpublished_draft_that_differs() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");
  let id = docs_id(&checkout);
  let mut db = checkout.db();
  let published = db
    .latest_version::<TeamMcpServer>(id)
    .expect("read")
    .expect("a version");
  let work: TeamMcpServer = serde_json::from_value(json!({
    "name": "docs",
    "launch": { "kind": "program", "program": "npx", "args": ["--someone-elses-work"] },
  }))
  .expect("a body");
  let at = Timestamp::from_unix_millis(5);
  db.save_draft(id, &Draft::new(work.clone(), Some(published.number()), at))
    .expect("a draft");

  let refused = checkout.refused("update_mcp_server", &change(json!({ "args": ["--other"] })));

  assert_eq!(refused, "draft_changed");
  assert_eq!(seen.lock().calls, ["create"]);
  assert_eq!(versions(&checkout), 1);
  let kept = db
    .draft::<TeamMcpServer>(id)
    .expect("read")
    .expect("the draft is still there");
  assert_eq!(kept.body(), &work);

  // A draft that holds what is published is nobody's work: it is replaced.
  db.save_draft(id, &Draft::new(published.body().clone(), Some(published.number()), at))
    .expect("a draft equal to the published version");
  let made = checkout
    .call("update_mcp_server", &change(json!({ "args": ["--other"] })))
    .expect("published");
  assert_eq!(made["server"]["published_version"], 2, "{made}");
}
