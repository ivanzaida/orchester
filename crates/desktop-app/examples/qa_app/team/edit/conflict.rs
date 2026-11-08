//! The hub's states of a team MCP server the team changed or archived while the person edited it (`team-edit-newer`,
//! `team-edit-conflicting`, `team-edit-conflict-clean`, `team-edit-archived`, ORC-837; A18 `bk4i9`, A19 `DV8OI`, A20
//! `CAPhQ`, A21 `yqYj9`): a draft of `pencil-design` on v1, and olha's v2 or her archive on the stand-in server. They
//! are made through the workspace's own calls, as a person's press would leave them, so nobody has to press: the
//! conflict is what the team server answers to the version that waited in the outbox, and the archive what it answers
//! to a publish. Fixture data for QA, never shipped.

use std::{error::Error, path::Path};

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::services::Services;
use orchester_core::{
  blob::ContentHash,
  id::{ResourceId, UserId},
  resource::{TeamMcpServer, VersionNumber},
  sharing::SharedPerson,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb};
use orchester_workspace::{
  agents::team_mcp_edit,
  app_mcp::setup::{NewTeamMcpServer, Runs, team_mcp_server},
  sharing::team_edit::{EditRefused, Refusal, Rejection},
};
use serde_json::Value;
use tokio::runtime::Handle;

use super::{State, pencil, said};

/// The folder both sides give the server, which the comparison says they agree on.
const FOLDER: &str = "tools/pencil";
/// The tool olha lets the server run without asking in her version.
const THEIR_TOOL: &str = "get_images";
/// The one who publishes meanwhile, and archives.
const TEAMMATE: &str = "olha";

/// How the arguments of the draft and of olha's version stand against v1's `--stdio --app desktop`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arguments {
  /// Both add a port, differently: a field both changed.
  Both,
  /// Only the draft adds one.
  Mine,
}

/// Makes the state `state` of this module, after Kontur joined `server` and the program path was set.
pub(super) fn prepare(
  state: State,
  (services, runtime): (&Services, &Handle),
  (server, root): (&TeamServer, &Path),
) -> Result<(), Box<dyn Error>> {
  let pencil = pencil(root)?;
  let v1 = services
    .workspace
    .team_hub(root, services.vault.get())?
    .mcp_server(pencil)
    .and_then(|sheet| sheet.edit.as_ref())
    .ok_or("pencil-design has an edit")?
    .published
    .clone();
  let kept = || draft_over(root, pencil, &v1);
  match state {
    State::TeamEditNewer => {
      kept()?;
      teammate_publishes(server, root, pencil, &v1, Arguments::Both)
    }
    State::TeamEditConflicting | State::TeamEditConflictClean => {
      let arguments = if state == State::TeamEditConflicting {
        Arguments::Both
      } else {
        Arguments::Mine
      };
      kept()?;
      // The version made here waits in the outbox; olha's v2 is on the server before it is sent, which answers the
      // conflict, and the pass that sends what waits records it, as when the person left the page offline.
      publish_here(root, pencil)?;
      server.publish_as("kontur", pencil, body(&theirs(&v1, arguments))?, TEAMMATE);
      runtime.block_on(services.servers.send_published(root)).map_err(said)?;
      Ok(())
    }
    State::TeamEditArchived => {
      kept()?;
      server.archive("kontur", pencil);
      let refused = runtime.block_on(services.servers.publish_mcp_edit(root, pencil));
      if !matches!(
        refused,
        Err(EditRefused::Refused(Refusal::Rejected(Rejection::Archived)))
      ) {
        return Err(format!("the publish was meant to be refused as archived: {refused:?}").into());
      }
      // What the team's event stream tells this computer once the archive is made.
      ProjectDb::open(root)?.mark_shared_archived(pencil, true, Timestamp::now())?;
      Ok(())
    }
    _ => Ok(()),
  }
}

/// v1 with the draft's changes: a port, the variable it needs and the folder.
fn mine(v1: &NewTeamMcpServer) -> NewTeamMcpServer {
  let mut fields = v1.clone();
  if let Runs::Program {
    arguments,
    folder,
    variables,
    ..
  } = &mut fields.runs
  {
    arguments.extend(["--port".to_owned(), "3845".to_owned()]);
    *folder = FOLDER.to_owned();
    variables.push("PENCIL_TOKEN".to_owned());
  }
  fields
}

/// v1 with olha's changes: another port where `arguments` says both changed one, a tool that runs without asking and
/// the same folder.
fn theirs(v1: &NewTeamMcpServer, arguments: Arguments) -> NewTeamMcpServer {
  let mut fields = v1.clone();
  if let Runs::Program {
    arguments: args,
    folder,
    ..
  } = &mut fields.runs
  {
    if arguments == Arguments::Both {
      args.extend(["--port".to_owned(), "3846".to_owned()]);
    }
    *folder = FOLDER.to_owned();
  }
  fields.exempt_tools.push(THEIR_TOOL.to_owned());
  fields
}

fn body(fields: &NewTeamMcpServer) -> Result<Value, Box<dyn Error>> {
  let server = team_mcp_server(fields, &[]).map_err(|invalid| format!("{invalid:?}"))?;
  Ok(serde_json::to_value(server)?)
}

/// The draft of the person: v1 with their changes, kept as edited from v1.
fn draft_over(root: &Path, pencil: ResourceId, v1: &NewTeamMcpServer) -> Result<(), Box<dyn Error>> {
  team_mcp_edit::save(root, pencil, &mine(v1), VersionNumber::FIRST)
    .map_err(|error| format!("the draft was not kept: {error:?}"))?;
  Ok(())
}

/// The draft becomes a version published here, which waits to be sent.
fn publish_here(root: &Path, pencil: ResourceId) -> Result<(), Box<dyn Error>> {
  use orchester_core::store::ResourceStore;
  ProjectDb::open(root)?.publish_draft::<TeamMcpServer>(pencil, Timestamp::now())?;
  Ok(())
}

/// olha publishes v2 on the server, and this computer has read it: the cache holds it as it came.
fn teammate_publishes(
  server: &TeamServer,
  root: &Path,
  pencil: ResourceId,
  v1: &NewTeamMcpServer,
  arguments: Arguments,
) -> Result<(), Box<dyn Error>> {
  let theirs = theirs(v1, arguments);
  server.publish_as("kontur", pencil, body(&theirs)?, TEAMMATE);
  let held = server
    .resources("kontur")
    .into_iter()
    .find(|(resource, _)| resource["id"] == pencil.to_string())
    .ok_or("the server holds pencil-design")?;
  let version = held.1.last().ok_or("the server holds its versions")?;
  let server_body = version["body"].clone();
  let cached = CachedVersion {
    number: VersionNumber::FIRST.next().ok_or("a version after the first")?,
    published_at: Timestamp::now(),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: TEAMMATE.to_owned(),
    },
    local_body: serde_json::from_value(server_body.clone())?,
    server_body,
    hash: ContentHash::new(version["hash"].as_str().ok_or("a hash")?)?,
    references: Vec::new(),
    waits_here: false,
  };
  ProjectDb::open(root)?.cache_shared_version::<TeamMcpServer>(pencil, &cached)?;
  Ok(())
}
