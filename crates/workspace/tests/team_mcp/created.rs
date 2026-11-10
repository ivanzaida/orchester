//! A team project keeps no draft of the team's profiles, pipelines, instructions, skills or tools on this computer
//! (`docs/design/team-configuration-live.md`, rule 6): the tools that make or publish one are refused `team_no_drafts`
//! before anything is read, written or sent. The team's MCP servers are created and changed on the team server at once
//! ([`super::mcp_servers`]).

use std::sync::Arc;

use orchester_core::{resource::ResourceKind, store::ResourceStore};
use parking_lot::Mutex;
use serde_json::{Value, json};

use crate::{
  Checkout, Standing,
  fixture::Config,
  http::Request,
  team_server::{Extra, TeamServer},
};

/// The server's answer to the `ResourceCreate` in `request`: the resource and its v1.
pub(super) fn created(request: &Request) -> Value {
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
pub(super) fn admin(server: &TeamServer) -> Checkout {
  let owner = Standing {
    role: "owner",
    widens: true,
  };
  Checkout::signed_in(server, owner)
}

/// A team server that counts every request about the project's resources.
fn counting() -> (TeamServer, Arc<Mutex<usize>>) {
  let asked = Arc::new(Mutex::new(0));
  let counted = asked.clone();
  let extra: Extra = Arc::new(move |state, request, _stream| {
    let project = state.lock().project;
    if request
      .path
      .starts_with(&format!("/api/v1/projects/{project}/resources"))
    {
      *counted.lock() += 1;
    }
    false
  });
  (TeamServer::start_with(Config::new("Say hello."), Some(extra)), asked)
}

#[test]
fn team_mcp_the_draft_tools_are_refused_in_a_team_project_and_nothing_is_made_or_sent() {
  let (server, asked) = counting();
  let checkout = admin(&server);
  let program = std::env::current_exe().expect("the test program");
  let calls = [
    (
      "save_instruction_draft",
      json!({ "project": "Kontur", "name": "Tests first", "applies_in": "project", "agents": "all",
              "events": "start", "text": "Write the failing test first." }),
    ),
    (
      "save_profile_draft",
      json!({ "project": "Kontur", "role": "Reviewer", "connection": "Codex", "model": "gpt-5.5" }),
    ),
    (
      "save_skill_draft",
      json!({ "project": "Kontur", "name": "Checks", "description": "Checks the change.", "body": "Look." }),
    ),
    (
      "save_tool_draft",
      json!({ "project": "Kontur", "name": "Build", "program": program }),
    ),
    ("create_pipeline", json!({ "project": "Kontur", "name": "Delivery" })),
  ];
  for (tool, args) in calls {
    let refused = checkout.call(tool, &args).expect_err("refused");
    assert_eq!(refused["error"]["code"], "team_no_drafts", "{tool}: {refused}");
  }
  assert_eq!(*asked.lock(), 0, "nothing reached the team server");
  for kind in [
    ResourceKind::Instruction,
    ResourceKind::AgentProfile,
    ResourceKind::Skill,
    ResourceKind::Tool,
    ResourceKind::Pipeline,
  ] {
    assert!(checkout.db().resources(kind).expect("read").is_empty(), "{kind:?}");
  }
}
