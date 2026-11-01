//! The team of `returning-team` (09 `i5G19A`): the stand-in team server *Acme* on loopback, sharing Kontur and
//! Billing, which the QA session joins by its code before the window opens, with the seeded Kontur linked to the
//! team's Kontur. Billing has no checkout here, so All projects offers it. The server answers while it is held.

use std::{error::Error, path::Path};

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, TeamServer, USERNAME},
};
use orchester_app::services::Services;
use orchester_core::{
  id::{ProjectId, ServerId},
  project::ProjectKey,
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::sharing::{CodeRequest, Trust, team_join::JoinOutcome};
use tokio::runtime::Runtime;

/// The team's name, as the design gives it.
const TEAM: &str = "Acme";

/// Joins *Acme* from the session of `services` and links the Kontur at `kontur` to the team's; the server, to hold
/// while the window is open.
pub fn join(services: &Services, runtime: &Runtime, kontur: &Path) -> Result<TeamServer, Box<dyn Error>> {
  let server = TeamServer::named(TEAM);
  server.share(&[("Kontur", "kontur", None), ("Billing", "billing", None)]);
  let address = ServerAddress::parse(&server.url())?;
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: None,
    proof: None,
  };
  let outcome = runtime.block_on(services.team_join.join_by_code(
    &address,
    Trust::System(PlainHttp::NotAllowed),
    request,
  ))?;
  let JoinOutcome::Joined(team) = outcome else {
    return Err(format!("the stand-in team did not take its code: {outcome:?}").into());
  };
  let projects = runtime.block_on(services.team_join.projects(team.id))?;
  let shared = projects
    .iter()
    .find(|listed| listed.project.name == "Kontur")
    .ok_or("the team shares Kontur")?;
  link(kontur, (team.id, shared.project.id))?;
  Ok(server)
}

/// Links the project at `repo` to `project` on `server`, as linking a checkout does.
fn link(repo: &Path, (server, project): (ServerId, ProjectId)) -> Result<(), Box<dyn Error>> {
  let mut db = ProjectDb::open(repo)?;
  let link = SharingLink {
    server,
    project,
    key: ProjectKey::new("KON")?,
    role: Some(MemberRole::Member),
    joined_at: Timestamp::now(),
    cursor: 1,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[], &Default::default()))?;
  Ok(())
}
