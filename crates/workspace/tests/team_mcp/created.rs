//! A new instruction, skill, tool or agent profile of a team project is created as v1 with no draft
//! (`docs/design/team-configuration.md`, decision 6): the tool counts it as created only once the team server accepts
//! it, takes it back whole when the server refuses it, and leaves it in the outbox (`docs/architecture/server.md`,
//! row 36) while the server cannot be reached.

use std::sync::Arc;

use orchester_core::{
  resource::{Instruction, ResourceKind, TeamAgentProfile},
  store::ResourceStore,
};
use orchester_store::PublishState;
use parking_lot::Mutex;
use serde_json::{Value, json};

use crate::{
  Checkout, Standing,
  fixture::Config,
  http::{Request, respond},
  team_server::{Extra, TeamServer},
};

/// What the fake server did with the resources it was sent.
#[derive(Default)]
struct Sent {
  /// Every `POST …/resources` that reached the server.
  created: usize,
  /// The version of the last one: its body and references, as sent.
  version: Value,
}

/// A team server that answers `POST …/resources` with `refusal` (a status and error code), or accepts the resource as
/// its v1 when it has none.
fn server(refusal: Option<(u16, &'static str)>) -> (TeamServer, Arc<Mutex<Sent>>) {
  let sent = Arc::new(Mutex::new(Sent::default()));
  let counted = sent.clone();
  let extra: Extra = Arc::new(move |state, request, stream| {
    let project = state.lock().project;
    let path = request.path.split('?').next().unwrap_or_default();
    if request.method != "POST" || path != format!("/api/v1/projects/{project}/resources") {
      return false;
    }
    let mut counted = counted.lock();
    counted.created += 1;
    counted.version = request.json()["version"].clone();
    drop(counted);
    match refusal {
      Some((status, code)) => respond(stream, status, &json!({"error": code, "message": "Refused."})),
      None => respond(stream, 201, &created(request)),
    }
    true
  });
  (TeamServer::start_with(Config::new("Say hello."), Some(extra)), sent)
}

/// The server's answer to the `ResourceCreate` in `request`: the resource and its v1.
fn created(request: &Request) -> Value {
  let create = request.json();
  let by = json!({ "id": "01a183cf-b73c-71db-8a62-bd509770cd4e", "display_name": "Dana" });
  let attribution = json!({ "by": by, "at": 1 });
  json!({
    "resource": {
      "id": create["id"], "kind": create["kind"], "slug": create["slug"], "name": create["name"], "archived": false,
      "revision": 1, "latest": 1, "created": attribution, "updated": attribution
    },
    "version": {
      "resource": create["id"], "number": 1, "body": create["version"]["body"], "hash": "a".repeat(64),
      "references": create["version"]["references"], "based_on": null, "published": attribution, "widens": false
    }
  })
}

/// A checkout linked as the team's owner, who let automation change agent permissions, signed in to `server`.
fn admin(server: &TeamServer) -> Checkout {
  let owner = Standing {
    role: "owner",
    widens: true,
  };
  Checkout::signed_in(server, owner)
}

fn rule() -> Value {
  json!({
    "project": "Kontur",
    "name": "Tests first",
    "applies_in": "project",
    "agents": "all",
    "events": "start",
    "text": "Write the failing test first.",
  })
}

fn instructions(checkout: &Checkout) -> usize {
  checkout
    .db()
    .resources(ResourceKind::Instruction)
    .expect("the instructions")
    .len()
}

#[test]
fn team_mcp_a_new_instruction_is_created_as_v1_once_the_server_accepts_it() {
  let (server, sent) = server(None);
  let checkout = admin(&server);

  let made = checkout.call("save_instruction_draft", &rule()).expect("created");

  assert_eq!(made["instruction"]["published_version"], 1, "{made}");
  assert_eq!(made["instruction"]["state"], "published", "{made}");
  assert!(made["instruction"].get("waiting_for_server").is_none(), "{made}");
  assert_eq!(sent.lock().created, 1);
  let db = checkout.db();
  assert!(db.waiting_publishes().expect("the outbox").is_empty());
  let resource = db.resources(ResourceKind::Instruction).expect("instructions").remove(0);
  assert_eq!(db.versions::<Instruction>(resource.id()).expect("versions").len(), 1);
  assert!(db.draft::<Instruction>(resource.id()).expect("a draft read").is_none());
}

#[test]
fn team_mcp_an_instruction_the_server_refuses_is_not_created() {
  let (server, sent) = server(Some((403, "not_admin")));
  let checkout = admin(&server);

  let refused = checkout.call("save_instruction_draft", &rule()).expect_err("refused");

  assert_eq!(refused["error"]["code"], "team_server_refused", "{refused}");
  assert_eq!(sent.lock().created, 1);
  assert_eq!(instructions(&checkout), 0, "nothing shows as created");
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());
}

#[test]
fn team_mcp_an_instruction_made_offline_waits_in_the_outbox() {
  let (server, sent) = server(None);
  let checkout = admin(&server);
  server.set_down(true);

  let made = checkout.call("save_instruction_draft", &rule()).expect("created");

  assert_eq!(
    made["instruction"]["waiting_for_server"]["code"], "team_server_unreachable",
    "{made}"
  );
  assert_eq!(sent.lock().created, 0);
  assert_eq!(instructions(&checkout), 1);
  let waiting = checkout.db().waiting_publishes().expect("the outbox");
  assert_eq!(waiting.len(), 1);
  assert_eq!(waiting[0].state, PublishState::Waiting);
}

#[test]
fn team_mcp_a_member_cannot_create_an_instruction() {
  let (server, sent) = server(None);
  let checkout = Checkout::standing(
    Some(&server),
    (
      &[],
      Standing {
        role: "member",
        widens: true,
      },
    ),
    |_| {},
  );

  let refused = checkout.refused("save_instruction_draft", &rule());

  assert_eq!(refused, "not_admin");
  assert_eq!(
    sent.lock().created,
    0,
    "this computer refused before the server was asked"
  );
  assert_eq!(instructions(&checkout), 0);
}

/// The other kinds a tool creates: the tool that makes one, its resource kind, and the key its answer holds it under.
const KINDS: [(&str, ResourceKind, &str); 3] = [
  ("save_profile_draft", ResourceKind::AgentProfile, "profile"),
  ("save_skill_draft", ResourceKind::Skill, "skill"),
  ("save_tool_draft", ResourceKind::Tool, "tool"),
];

/// The kinds the server is asked to take, a profile in the team's form (`team_mcp_a_new_profile_is_sent_...`).
const SENT_KINDS: [(&str, ResourceKind, &str); 3] = KINDS;

/// The arguments of a new resource of the kind answered under `key`.
fn new_resource(key: &str) -> Value {
  match key {
    "profile" => json!({ "project": "Kontur", "role": "Reviewer", "connection": "Codex", "model": "gpt-5.5" }),
    "skill" => json!({ "project": "Kontur", "name": "Checks", "description": "Checks the change.", "body": "Look." }),
    _ => {
      let program = std::env::current_exe().expect("the test program");
      json!({ "project": "Kontur", "name": "Build", "program": program })
    }
  }
}

fn count(checkout: &Checkout, kind: ResourceKind) -> usize {
  checkout.db().resources(kind).expect("the resources").len()
}

#[test]
fn team_mcp_a_new_profile_skill_or_tool_is_created_as_v1_once_the_server_accepts_it() {
  for (tool, kind, key) in SENT_KINDS {
    let (server, sent) = server(None);
    let checkout = admin(&server);

    let made = checkout.call(tool, &new_resource(key)).expect(key);

    assert_eq!(sent.lock().created, 1, "{key}: {made}");
    assert_eq!(count(&checkout, kind), 1, "{key}");
    assert!(made[key].get("waiting_for_server").is_none(), "{key}: {made}");
    assert!(
      checkout.db().waiting_publishes().expect("the outbox").is_empty(),
      "{key}"
    );
  }
}

#[test]
fn team_mcp_a_profile_skill_or_tool_the_server_refuses_is_not_created() {
  for (tool, kind, key) in SENT_KINDS {
    let (server, sent) = server(Some((403, "not_admin")));
    let checkout = admin(&server);

    let refused = checkout.call(tool, &new_resource(key)).expect_err(key);

    assert_eq!(refused["error"]["code"], "team_server_refused", "{key}: {refused}");
    assert_eq!(sent.lock().created, 1, "{key}");
    assert_eq!(count(&checkout, kind), 0, "{key}: nothing shows as created");
    assert!(
      checkout.db().waiting_publishes().expect("the outbox").is_empty(),
      "{key}"
    );
  }
}

#[test]
fn team_mcp_a_profile_skill_or_tool_made_offline_waits_in_the_outbox() {
  for (tool, kind, key) in SENT_KINDS {
    let (server, sent) = server(None);
    let checkout = admin(&server);
    server.set_down(true);

    let made = checkout.call(tool, &new_resource(key)).expect(key);

    assert_eq!(
      made[key]["waiting_for_server"]["code"], "team_server_unreachable",
      "{key}: {made}"
    );
    assert_eq!(sent.lock().created, 0, "{key}");
    assert_eq!(count(&checkout, kind), 1, "{key}");
    let waiting = checkout.db().waiting_publishes().expect("the outbox");
    assert_eq!(waiting.len(), 1, "{key}");
    assert_eq!(waiting[0].state, PublishState::Waiting, "{key}");
  }
}

/// A profile names no connection of this computer: the team's body names the CLI kind of the connection it was made
/// for, and its references repeat that.
#[test]
fn team_mcp_a_new_profile_is_sent_in_the_teams_form_with_the_cli_kind_of_its_connection() {
  let (server, sent) = server(None);
  let checkout = admin(&server);

  let made = checkout
    .call("save_profile_draft", &new_resource("profile"))
    .expect("created");

  let version = sent.lock().version.clone();
  let body = &version["body"];
  assert_eq!(body["cli"], "codex_cli", "{version}");
  assert!(body.get("connection").is_none(), "{version}");
  assert!(
    serde_json::from_value::<TeamAgentProfile>(body.clone()).is_ok(),
    "{version}"
  );
  assert_eq!(
    version["references"],
    json!([{"type": "cli_kind", "cli": "codex_cli"}]),
    "{version}"
  );
  assert!(made["profile"].get("waiting_for_server").is_none(), "{made}");
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());
}
