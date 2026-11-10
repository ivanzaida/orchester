//! The team's configuration in a QA copy joined to the stand-in team server: it is read from the team server and held
//! in memory (`docs/design/team-configuration-live.md`), so the stand-in holds every resource of the team the seed
//! cached, at the version it cached and with the body the server confirmed, and the window reads it from there as it
//! reads a real team's. The Agents hub's normal states (`team-hub`, `team-hub-member`) wait for that read. Fixture data
//! for QA, never shipped.

use std::{
  error::Error,
  path::Path,
  thread,
  time::{Duration, Instant},
};

use orchester_api::{ResourceCreate, SharedResourceKind, VersionContent, test_support::team_server::TeamServer};
use orchester_app::services::Services;
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::{
    AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Skill, TeamMcpServer, TeamReviewer, Tool,
    VersionNumber,
  },
  sharing::MemberRole,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, ResourceSharing, SharingLink};
use serde_json::Value;

/// The team's kinds the seed caches.
const KINDS: [ResourceKind; 7] = [
  ResourceKind::AgentProfile,
  ResourceKind::Instruction,
  ResourceKind::Skill,
  ResourceKind::Tool,
  ResourceKind::McpServer,
  ResourceKind::Pipeline,
  ResourceKind::TeamReviewer,
];

/// Holds on `server` every team resource Kontur's cache at `root` holds, at its latest cached version, but for one
/// the server holds already.
pub fn hold_cached(server: &TeamServer, root: &Path) -> Result<(), Box<dyn Error>> {
  let db = ProjectDb::open(root)?;
  let held: Vec<String> = server
    .resources("kontur")
    .iter()
    .filter_map(|(resource, _)| resource["id"].as_str().map(str::to_owned))
    .collect();
  for kind in KINDS {
    let shared = SharedResourceKind::from_core(kind).ok_or("a team kind")?;
    for resource in db.resources(kind)? {
      let on_server = held.contains(&resource.id().to_string());
      if on_server || !matches!(db.resource_sharing(resource.id())?, Some(ResourceSharing::Shared(_))) {
        continue;
      }
      let Some(number) = db.latest_version_number(resource.id())? else {
        continue;
      };
      let Some(body) = server_body(&db, (kind, resource.id()), number)? else {
        continue;
      };
      let create = ResourceCreate {
        idempotency_key: IdempotencyKey::generate(),
        id: resource.id(),
        kind: shared,
        slug: resource.slug().as_str().to_owned(),
        name: resource.name().as_str().to_owned(),
        version: VersionContent {
          body,
          references: Vec::new(),
        },
      };
      server.hold_resource("kontur", &create, number.get());
    }
  }
  Ok(())
}

/// The body the server confirmed of version `number` of the resource `id` of `kind`; `None` for one not cached from a
/// server.
fn server_body(
  db: &ProjectDb,
  (kind, id): (ResourceKind, ResourceId),
  number: VersionNumber,
) -> Result<Option<Value>, Box<dyn Error>> {
  fn of<B: ResourceBody>(
    db: &ProjectDb,
    id: ResourceId,
    number: VersionNumber,
  ) -> Result<Option<Value>, Box<dyn Error>> {
    Ok(db.cached_version::<B>(id, number)?.map(|cached| cached.server_body))
  }
  match kind {
    ResourceKind::AgentProfile => of::<AgentProfile>(db, id, number),
    ResourceKind::Instruction => of::<Instruction>(db, id, number),
    ResourceKind::Skill => of::<Skill>(db, id, number),
    ResourceKind::Tool => of::<Tool>(db, id, number),
    ResourceKind::McpServer => of::<TeamMcpServer>(db, id, number),
    ResourceKind::Pipeline => of::<Pipeline>(db, id, number),
    ResourceKind::TeamReviewer => of::<TeamReviewer>(db, id, number),
    ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => Ok(None),
  }
}

/// How long the first read of the team's configuration may take before the state says it failed.
const FIRST_READ: Duration = Duration::from_secs(30);

/// Waits until this computer read Kontur's team configuration at `root` from the team server, as the stream reads it
/// once it opens after the join, so what the state then changes on the server comes after that read.
pub fn read(services: &Services, root: &Path) -> Result<(), Box<dyn Error>> {
  let deadline = Instant::now() + FIRST_READ;
  loop {
    let read = services.workspace.team_read(root);
    if read.config().is_some() && read.unreached().is_none() {
      return Ok(());
    }
    if Instant::now() >= deadline {
      return Err(format!("the team's configuration was not read from the stand-in: {read:?}").into());
    }
    thread::sleep(Duration::from_millis(20));
  }
}

/// Moves the link the seed made to `link`: the hub's seeds link Kontur to the team whose configuration they cache
/// (`shared_config`), and in a QA copy that team is the stand-in, so the link names it and its cache is kept, where
/// linking again would be refused as already shared.
pub fn move_link(repo: &Path, link: &SharingLink) -> Result<(), Box<dyn Error>> {
  let role = link.role.map(|role| match role {
    MemberRole::Owner => "owner",
    MemberRole::Member => "member",
  });
  let moved = rusqlite::Connection::open(repo.join(".orchester").join("orchester.db"))?.execute(
    "UPDATE sharing SET server_id = ?1, project_id = ?2, role = ?3, event_cursor = 0, project_address = ?4",
    rusqlite::params![link.server.to_string(), link.project.to_string(), role, link.address],
  )?;
  if moved == 1 {
    Ok(())
  } else {
    Err(format!("Kontur has one link to move, found {moved}").into())
  }
}
