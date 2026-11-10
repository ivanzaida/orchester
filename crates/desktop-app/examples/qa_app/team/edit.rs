//! What the hub's edit states (`team-edit*`, ORC-836) do around Kontur's join of the stand-in team server: before it,
//! the server holds the team MCP servers this computer cached, each at the version it cached, so the read after the
//! join holds them and an edit's *Save* is judged by the server; once that read is done, the state's own switch is set:
//! a server that no longer takes this person's writes, one that cannot be reached, or one that rejects the content of
//! what is sent. The *result* states (`team-edit-published`, `-refused-admin`) are what a person's press leaves behind,
//! which no MCP client can press: they are made here through the workspace's own calls (the same a press makes).
//! Fixture data for QA, never shipped.

use std::{error::Error, fs, path::Path};

use orchester_api::{
  ResourceCreate, SharedResourceKind, VersionContent,
  test_support::team_server::{Standing, TeamServer},
};
use orchester_app::services::Services;
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::{ResourceKind, TeamMcpServer, VersionNumber},
  run::Answerer,
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  agents::{
    team_hub::TeamHubRead,
    team_mcp_edit::{self, EditError},
  },
  app_mcp::setup::Runs,
  sharing::team_edit::{EditRefused, Refusal},
};
use tokio::runtime::Handle;

use super::State;

/// The team's MCP server this computer has not read, which the server holds under the name a new server may not take.
const TAKEN: &str = "figma-local";

/// Holds Kontur's cached team MCP servers on `server` for the hub's edit state `state`, before Kontur at `root` joins
/// it; nothing for any other state.
pub fn hold(state: State, server: &TeamServer, root: &Path) -> Result<(), Box<dyn Error>> {
  if !state.edits() {
    return Ok(());
  }
  let db = ProjectDb::open(root)?;
  for resource in db.resources(ResourceKind::McpServer)? {
    let cached = db
      .cached_version::<TeamMcpServer>(resource.id(), VersionNumber::FIRST)?
      .ok_or("the seed caches v1 of each team MCP server")?;
    hold_one(
      server,
      (resource.id(), resource.slug().as_str(), resource.name().as_str()),
      cached.server_body,
    );
  }
  Ok(())
}

/// Sets up the hub's edit state `state` once Kontur joined `server` and read what it holds: `figma-local` is held after
/// that read, so this computer has not read it; nothing for any other state.
pub fn prepare(
  state: State,
  (services, runtime): (&Services, &Handle),
  (server, root): (&TeamServer, &Path),
) -> Result<(), Box<dyn Error>> {
  if !state.edits() {
    return Ok(());
  }
  super::hub::read(services, root)?;
  let taken = serde_json::json!({"name": TAKEN, "launch": {
    "kind": "program", "program": "npx", "args": ["figma-developer-mcp", "--local"], "variables": []
  }});
  hold_one(server, (ResourceId::generate(), TAKEN, TAKEN), taken);
  match state {
    State::TeamEditDemoted => server.refuse_writes_as_not_admin("kontur"),
    State::TeamEditOffline => server.set_down(true),
    State::TeamEditRejects => server.reject_content("kontur", "body"),
    _ => {}
  }
  result(state, (services, runtime), (server, root))
}

pub(super) fn hold_one(server: &TeamServer, (id, slug, name): (ResourceId, &str, &str), body: serde_json::Value) {
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

/// The arguments the edit types: the hub's own tests and the design's A13 type these.
const ARGUMENTS: [&str; 4] = ["--stdio", "--app", "desktop", "--no-telemetry"];

/// Takes the result state `state` to where a person's press leaves it, after its switch: nothing for the states a
/// person presses in. The resource of the edit is the seed's `pencil-design`.
fn result(
  state: State,
  (services, runtime): (&Services, &Handle),
  (server, root): (&TeamServer, &Path),
) -> Result<(), Box<dyn Error>> {
  if state.program_set() {
    set_program(services, root)?;
  }
  match state {
    State::TeamEditPublished => {
      let edited = edit(services, runtime, root)?;
      match edited {
        Ok(_) => Ok(()),
        other => Err(format!("the edit was meant to be published: {other:?}").into()),
      }
    }
    State::TeamEditRefusedAdmin => {
      server.refuse_writes_as_not_admin("kontur");
      let edited = edit(services, runtime, root)?;
      if !matches!(edited, Err(EditError::Refused(EditRefused::Refused(Refusal::NotAdmin)))) {
        return Err(format!("the edit was meant to be refused as no longer an admin: {edited:?}").into());
      }
      // The team's next read tells this computer what the server said: the person is a member now.
      server.stand_as(Standing::Member);
      let mut db = ProjectDb::open(root)?;
      db.record_sharing_state(Some(MemberRole::Member), Some(Timestamp::now()), None)?;
      db.set_server_admin(false)?;
      Ok(())
    }
    _ => Ok(()),
  }
}

/// An error of the workspace's typed answers (none of which is a `std::error::Error`) as the QA app reports it.
pub(super) fn said(error: impl std::fmt::Debug) -> String {
  format!("{error:?}")
}

/// The program path of `pencil-design` set on this computer, as a person's *Choose…* leaves it (an empty file of the QA
/// folder of programs).
fn set_program(services: &Services, root: &Path) -> Result<(), Box<dyn Error>> {
  let file = root.join(".orchester").join("qa-bin").join("pencil-mcp");
  let folder = file.parent().ok_or("the QA folder of programs")?;
  fs::create_dir_all(folder)?;
  fs::write(&file, "")?;
  services
    .workspace
    .set_team_mcp_program(root, pencil(root)?, &file, Answerer::Person)
    .map_err(said)?;
  Ok(())
}

/// The seed's `pencil-design`.
pub(super) fn pencil(root: &Path) -> Result<ResourceId, Box<dyn Error>> {
  Ok(
    ProjectDb::open(root)?
      .resources(ResourceKind::McpServer)?
      .into_iter()
      .find(|resource| resource.name().as_str() == "pencil-design")
      .ok_or("the seed's pencil-design")?
      .id(),
  )
}

/// The edit a person makes of `pencil-design`: its arguments typed, and *Save*, which publishes them at once on the
/// version the hub shows.
fn edit(
  services: &Services,
  runtime: &Handle,
  root: &Path,
) -> Result<Result<orchester_core::resource::VersionNumber, EditError>, Box<dyn Error>> {
  let pencil = pencil(root)?;
  let TeamHubRead::Read(hub) = services.workspace.team_hub(root, services.vault.get())? else {
    return Err("the team's configuration has not been read from the team server".into());
  };
  let opened = hub
    .mcp_server(pencil)
    .and_then(|sheet| sheet.edit.clone())
    .ok_or("pencil-design has an edit")?;
  let mut fields = opened.published;
  if let Runs::Program { arguments, .. } = &mut fields.runs {
    *arguments = ARGUMENTS.map(str::to_owned).to_vec();
  }
  let config = services
    .workspace
    .team_read(root)
    .config()
    .cloned()
    .ok_or("the team's configuration has not been read from the team server")?;
  Ok(runtime.block_on(team_mcp_edit::save(
    &services.servers,
    (root, &config),
    (pencil, opened.number),
    &fields,
  )))
}
