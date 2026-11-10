//! What the hub's edit states (`team-edit*`, `team-create-name-taken`, ORC-836) do once Kontur joined the stand-in team
//! server: the server holds the team MCP servers this computer cached, each at the version it cached, so an edit's
//! *Publish for the team* is judged by it, and the state's own switch is set: a server that no longer takes this
//! person's writes, one that cannot be reached, or one that rejects the content of what is sent. The *result* states
//! (`team-edit-published`, `-refused-admin`, `-waiting`, `-rejected`, `team-create-name-taken`) are what a person's
//! press leaves behind, which no MCP client can press: they are made here through the workspace's own calls (the same a
//! press makes), so the store and the outbox hold what they hold after one. Fixture data for QA, never shipped.

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
  agents::{team_hub::TeamHubRead, team_mcp_edit},
  app_mcp::setup::{NewTeamMcpServer, Runs, create_team_mcp_server, team_mcp_names, team_mcp_server},
  sharing::team_edit::{EditRefused, Edited, Refusal},
};
use tokio::runtime::Handle;

mod conflict;

use super::State;

/// The team's MCP server this computer has not read, which the server holds under the name a new server may not take.
const TAKEN: &str = "figma-local";

/// Sets up the hub's edit state `state`, once Kontur joined `server`; nothing for any other state.
pub fn prepare(
  state: State,
  (services, runtime): (&Services, &Handle),
  (server, root): (&TeamServer, &Path),
) -> Result<(), Box<dyn Error>> {
  if !state.edits() {
    return Ok(());
  }
  after_join(state, server, root)?;
  result(state, (services, runtime), (server, root))
}

/// Holds Kontur's cached team MCP servers (and `figma-local`) on `server`, and sets the switch `state` names.
fn after_join(state: State, server: &TeamServer, root: &Path) -> Result<(), Box<dyn Error>> {
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

pub(super) fn hold(server: &TeamServer, (id, slug, name): (ResourceId, &str, &str), body: serde_json::Value) {
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

/// Takes the result state `state` to where a person's press leaves it, after [`after_join`]: nothing for the states a
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
        Ok(Edited::Published(_)) => Ok(()),
        other => Err(format!("the edit was meant to be published: {other:?}").into()),
      }
    }
    State::TeamEditRefusedAdmin => {
      server.refuse_writes_as_not_admin("kontur");
      let edited = edit(services, runtime, root)?;
      if !matches!(edited, Err(EditRefused::Refused(Refusal::NotAdmin))) {
        return Err(format!("the edit was meant to be refused as no longer an admin: {edited:?}").into());
      }
      // The team's next read tells this computer what the server said: the person is a member now.
      server.stand_as(Standing::Member);
      let mut db = ProjectDb::open(root)?;
      db.record_sharing_state(Some(MemberRole::Member), Some(Timestamp::now()), None)?;
      db.set_server_admin(false)?;
      Ok(())
    }
    State::TeamEditWaiting => {
      server.set_down(true);
      let edited = edit(services, runtime, root)?;
      match edited {
        Ok(Edited::Waiting(_)) => Ok(()),
        other => Err(format!("the edit was meant to wait in the outbox: {other:?}").into()),
      }
    }
    State::TeamEditRejected => {
      // Published and then sent by the pass that sends what waits, as when the person left the panel before the server
      // answered: the server rejects it, and the outbox keeps it refused.
      let pencil = pencil(root)?;
      draft(services, root, pencil)?;
      ProjectDb::open(root)?.publish_draft::<TeamMcpServer>(pencil, Timestamp::now())?;
      server.reject_content("kontur", "body");
      runtime.block_on(services.servers.send_published(root)).map_err(said)?;
      Ok(())
    }
    State::TeamCreateNameTaken => {
      // A new server kept in the outbox, then sent by the pass that sends what waits, as when the person left the form
      // before the server answered: refused as the name being taken, taken back whole, and the hub says so.
      create_team_mcp_server(
        root,
        &team_mcp_server(&named(TAKEN), &team_mcp_names(root)?).map_err(|invalid| format!("{invalid:?}"))?,
      )?;
      runtime.block_on(services.servers.send_published(root)).map_err(said)?;
      Ok(())
    }
    State::TeamEditNewer | State::TeamEditConflicting | State::TeamEditConflictClean | State::TeamEditArchived => {
      conflict::prepare(state, (services, runtime), (server, root))
    }
    _ => Ok(()),
  }
}

/// An error of the workspace's typed answers (none of which is a `std::error::Error`) as the QA app reports it.
pub(super) fn said(error: impl std::fmt::Debug) -> String {
  format!("{error:?}")
}

/// A new team MCP server of `name` that runs a program.
fn named(name: &str) -> NewTeamMcpServer {
  NewTeamMcpServer {
    name: name.to_owned(),
    runs: Runs::Program {
      program: "npx".to_owned(),
      arguments: vec!["figma-developer-mcp".to_owned()],
      folder: String::new(),
      variables: Vec::new(),
    },
    exempt_tools: Vec::new(),
  }
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

/// The fields `pencil-design` has as published, as its form in the hub starts from.
pub(super) fn published(
  services: &Services,
  root: &Path,
  pencil: ResourceId,
) -> Result<NewTeamMcpServer, Box<dyn Error>> {
  let TeamHubRead::Read(hub) = services.workspace.team_hub(root, services.vault.get())? else {
    return Err("the team's configuration has not been read from the team server".into());
  };
  Ok(
    hub
      .mcp_server(pencil)
      .and_then(|sheet| sheet.edit.as_ref())
      .ok_or("pencil-design has an edit")?
      .published
      .clone(),
  )
}

/// Types `pencil-design`'s arguments and keeps them as its draft, as the form does.
fn draft(services: &Services, root: &Path, pencil: ResourceId) -> Result<(), Box<dyn Error>> {
  let mut fields = published(services, root, pencil)?;
  if let Runs::Program { arguments, .. } = &mut fields.runs {
    *arguments = ARGUMENTS.map(str::to_owned).to_vec();
  }
  team_mcp_edit::save(root, pencil, &fields, orchester_core::resource::VersionNumber::FIRST)
    .map_err(|error| format!("the draft was not kept: {error:?}"))?;
  Ok(())
}

/// The edit a person makes of `pencil-design`: its arguments typed, kept as the draft, and *Publish for the team*.
fn edit(services: &Services, runtime: &Handle, root: &Path) -> Result<Result<Edited, EditRefused>, Box<dyn Error>> {
  let pencil = pencil(root)?;
  draft(services, root, pencil)?;
  Ok(runtime.block_on(services.servers.publish_mcp_edit(root, pencil)))
}
