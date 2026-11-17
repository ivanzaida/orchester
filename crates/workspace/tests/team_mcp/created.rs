//! A team project keeps no draft of the team's profiles, instructions, skills or tools on this computer
//! (`docs/design/team-configuration-live.md`, rule 6): the tools that make one create it on the team server at once as
//! its version 1, and nothing of it is written to the project's database. A team pipeline's tools send it to the team
//! server and wait for its answer too (D-043, `team_pipeline_save`). The team's MCP servers are created and changed on
//! the team server at once ([`super::mcp_servers`]); changing the team's instructions, skills and tools is in
//! [`super::team_resources`].

use orchester_core::{resource::ResourceKind, store::ResourceStore};
use serde_json::{Value, json};

use super::mcp_servers::{Answers, start};
use crate::{Checkout, Standing, http::Request, team_server::TeamServer};

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

#[test]
fn team_mcp_the_draft_tools_create_the_teams_resources_on_the_team_server_and_write_nothing_here() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  let program = std::env::current_exe().expect("the test program");
  let calls = [
    (
      "save_instruction_draft",
      json!({ "project": "Kontur", "name": "Tests first", "applies_in": "project", "agents": "all",
              "events": "start", "text": "Write the failing test first." }),
      "instruction",
    ),
    (
      "save_skill_draft",
      json!({ "project": "Kontur", "name": "Checks", "description": "Checks the change.", "body": "Look." }),
      "skill",
    ),
    (
      "save_tool_draft",
      json!({ "project": "Kontur", "name": "Build", "program": program }),
      "tool",
    ),
  ];
  for (tool, args, kind) in calls {
    let made = checkout.call(tool, &args).expect("created");
    assert_eq!(made[kind]["team"], true, "{tool}: {made}");
    assert_eq!(made[kind]["revision"], "1", "{tool}: {made}");
  }
  let seen = seen.lock();
  assert_eq!(seen.calls, ["create", "create", "create"]);
  let kinds: Vec<&str> = seen.created.iter().filter_map(|made| made["kind"].as_str()).collect();
  assert_eq!(kinds, ["instruction", "skill", "tool"]);
  for kind in [ResourceKind::Instruction, ResourceKind::Skill, ResourceKind::Tool] {
    assert!(checkout.db().resources(kind).expect("read").is_empty(), "{kind:?}");
  }
}
