//! What the hub's edit states (`team-edit*`, ORC-836) do once Kontur joined the stand-in team server: the server holds
//! the team MCP servers this computer cached, each at the version it cached, so an edit's *Publish for the team* is
//! judged by it, and the state's own switch is set: a server that no longer takes this person's writes, one that cannot
//! be reached, or one that rejects the content of what is sent. Fixture data for QA, never shipped.

use std::{error::Error, path::Path};

use orchester_api::{ResourceCreate, SharedResourceKind, VersionContent, test_support::team_server::TeamServer};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::{ResourceKind, TeamMcpServer, VersionNumber},
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use super::State;

/// The team's MCP server this computer has not read, which the server holds under the name a new server may not take.
const TAKEN: &str = "figma-local";

/// Holds Kontur's cached team MCP servers (and `figma-local`) on `server`, and sets the switch `state` names.
pub fn after_join(state: State, server: &TeamServer, root: &Path) -> Result<(), Box<dyn Error>> {
  let db = ProjectDb::open(root)?;
  for resource in db.resources(ResourceKind::McpServer)? {
    let cached = db
      .cached_version::<TeamMcpServer>(resource.id(), VersionNumber::FIRST)?
      .ok_or("the seed caches v1 of each team MCP server")?;
    hold(
      server,
      (resource.id(), resource.slug().as_str(), resource.name().as_str()),
      cached.server_body,
    );
  }
  let taken = serde_json::json!({"name": TAKEN, "launch": {
    "kind": "program", "program": "npx", "args": ["figma-developer-mcp", "--local"], "variables": []
  }});
  hold(server, (ResourceId::generate(), TAKEN, TAKEN), taken);
  match state {
    State::TeamEditDemoted => server.refuse_writes_as_not_admin("kontur"),
    State::TeamEditOffline => server.set_down(true),
    State::TeamEditRejects => server.reject_content("kontur", "body"),
    _ => {}
  }
  Ok(())
}

fn hold(server: &TeamServer, (id, slug, name): (ResourceId, &str, &str), body: serde_json::Value) {
  let create = ResourceCreate {
    idempotency_key: IdempotencyKey::generate(),
    id,
    kind: SharedResourceKind::McpServer,
    slug: slug.to_owned(),
    name: name.to_owned(),
    version: VersionContent {
      body,
      references: Vec::new(),
    },
  };
  server.hold_resource("kontur", &create, 1);
}
