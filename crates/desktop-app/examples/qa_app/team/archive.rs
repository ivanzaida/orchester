//! What the archive states (`team-archive`, `team-archived`, ORC-838) do around Kontur's join of the stand-in team
//! server: before it, the server holds the team's MCP servers and the profile Builder, which names `pencil-design`, so
//! what the read after the join holds, what *Used by* lists and what the stopped run's card says Builder still uses are
//! the server's own answers. `team-archive` also has two runs that pin `pencil-design`, which the confirmation lists as
//! the runs that stop. `team-archived` is what a person's *Archive and stop N runs* leaves behind, which no MCP client
//! can press: `pencil-design` is archived on the server and gone from this computer's lists, as the server's
//! `resource.archived` leaves it, and the run the seed recorded as stopped by it is run 8 of KON-12. Fixture data for
//! QA, never shipped.

use std::{error::Error, path::Path};

use orchester_api::{
  ReferenceTarget, ResourceCreate, SharedResourceKind, VersionContent,
  test_support::team_server::{Person, PinnedRun, RunReport, TeamServer},
};
use orchester_core::{
  id::{IdempotencyKey, ResourceId, RunId},
  resource::{AgentProfile, ResourceKind, TeamAgentProfile, TeamMcpServer, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, ResourceSharing};

use super::State;

/// Holds the resources the archive states need on `server`, before Kontur at `root` joins it: `pencil-design` archived
/// there for `team-archived`.
pub fn hold(state: State, (server, root): (&TeamServer, &Path)) -> Result<(), Box<dyn Error>> {
  if !state.archives() {
    return Ok(());
  }
  let db = ProjectDb::open(root)?;
  let mut pencil = None;
  for resource in db.resources(ResourceKind::McpServer)? {
    let cached = db
      .cached_version::<TeamMcpServer>(resource.id(), VersionNumber::FIRST)?
      .ok_or("the seed caches v1 of each team MCP server")?;
    hold_one(
      server,
      (
        SharedResourceKind::McpServer,
        resource.id(),
        resource.slug().as_str(),
        resource.name().as_str(),
      ),
      VersionContent {
        body: cached.server_body,
        references: Vec::new(),
      },
    );
    if resource.name().as_str() == "pencil-design" {
      pencil = Some(resource.id());
    }
  }
  let pencil = pencil.ok_or("the seed's pencil-design")?;
  // The team's Builder: the catalogue has a local one of the same name.
  let mut builder = None;
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let shared = matches!(db.resource_sharing(resource.id())?, Some(ResourceSharing::Shared(_)));
    if shared && resource.name().as_str() == "Builder" {
      builder = Some(resource);
      break;
    }
  }
  let builder = builder.ok_or("the seed's team Builder")?;
  let cached = db
    .cached_version::<AgentProfile>(builder.id(), VersionNumber::FIRST)?
    .ok_or("the seed caches v1 of Builder")?;
  // Builder names the team's MCP servers in its body, and its version's references repeat them as the server reads
  // them; only those are held here, the tools, skills and instructions it names are not on the stand-in.
  let profile: TeamAgentProfile = serde_json::from_value(cached.server_body.clone())?;
  let references = ReferenceTarget::of_profile(&profile)
    .into_iter()
    .filter(|reference| {
      matches!(
        reference,
        ReferenceTarget::TeamResource {
          kind: SharedResourceKind::McpServer,
          ..
        }
      )
    })
    .collect();
  hold_one(
    server,
    (
      SharedResourceKind::AgentProfile,
      builder.id(),
      builder.slug().as_str(),
      builder.name().as_str(),
    ),
    VersionContent {
      body: cached.server_body,
      references,
    },
  );
  if state == State::TeamArchived {
    server.archive("kontur", pencil);
  }
  Ok(())
}

/// Makes the result state's changes once Kontur joined `server`.
pub fn prepare(state: State, (server, root): (&TeamServer, &Path)) -> Result<(), Box<dyn Error>> {
  match state {
    State::TeamArchive => pin_runs(server, super::edit::pencil(root)?),
    // The server's archive reached this computer: the server is not asked again, the resource leaves the lists.
    State::TeamArchived => ProjectDb::open(root)?
      .mark_shared_archived(super::edit::pencil(root)?, true, Timestamp::now())
      .map(drop)?,
    _ => {}
  }
  Ok(())
}

fn hold_one(
  server: &TeamServer,
  (kind, id, slug, name): (SharedResourceKind, ResourceId, &str, &str),
  version: VersionContent,
) {
  let create = ResourceCreate {
    idempotency_key: IdempotencyKey::generate(),
    id,
    kind,
    slug: slug.to_owned(),
    name: name.to_owned(),
    version,
  };
  server.hold_resource("kontur", &create, 1);
}

/// KON-12 (the person's) and KON-21 (olha's) have run work that pins `pencil-design`'s v1 and has not ended.
fn pin_runs(server: &TeamServer, pencil: ResourceId) {
  for (task_key, holder) in [("KON-12", Person::You), ("KON-21", Person::Named("olha"))] {
    server.pin_run(
      "kontur",
      PinnedRun {
        task_key: task_key.to_owned(),
        run: RunId::generate(),
        work: 1,
        holder: Some(holder),
        last_report: RunReport::Progress,
        pins: vec![(pencil, VersionNumber::FIRST)],
      },
    );
  }
}
