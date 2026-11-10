//! The team's configuration the stream would hold in memory, for tests that keep the team's resources in the project's
//! cache and run no stream: every resource the cache marks as the team's, archived ones too, with its latest version
//! the server confirmed, its body as the server sent it — as a stream caught up with the cache would hold it.
//!
//! orchester-workspace's and orchester-app's tests share this file (`#[path]`).

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{ResourceKind, VersionNumber},
  sharing::TaskRevision,
  time::Timestamp,
};
use orchester_workspace::{
  team_config::{TeamConfig, TeamResource, TeamVersion},
  workspace::Workspace,
};

/// Keeps the team's resources of the project cache at `root` in `workspace`'s memory, as a read of the team server
/// does; a project not shared with a team has none.
pub fn keep_cached_team(workspace: &Workspace, root: &Path) {
  if let Some(config) = cached_config(root) {
    workspace.keep_team_config_for_test(root, config);
  }
}

/// The team's configuration the cache at `root` holds; `None` for a project not shared with a team.
fn cached_config(root: &Path) -> Option<TeamConfig> {
  let connection = rusqlite::Connection::open(root.join(".orchester/orchester.db")).expect("open");
  let shared: bool = connection
    .query_row("SELECT EXISTS (SELECT 1 FROM sharing)", [], |row| row.get(0))
    .expect("the link");
  if !shared {
    return None;
  }
  let mut resources = connection
    .prepare(
      "SELECT id, kind, name, server_revision, \
       EXISTS (SELECT 1 FROM removed_resources WHERE resource_id = resources.id) \
       FROM resources WHERE sharing = 'shared'",
    )
    .expect("the team's resources");
  let rows = resources
    .query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, i64>(3)?,
        row.get::<_, bool>(4)?,
      ))
    })
    .expect("read")
    .map(|row| row.expect("a resource"));
  let kept: Vec<TeamResource> = rows
    .map(|(id, kind, name, revision, archived)| {
      let id: ResourceId = id.parse().expect("a resource id");
      TeamResource {
        id,
        kind: kind_of(&kind),
        name: Name::new(&name).expect("a name"),
        archived,
        revision: TaskRevision::try_from(u64::try_from(revision).expect("a positive revision")).expect("a revision"),
        latest: latest(&connection, id),
      }
    })
    .collect();
  Some(TeamConfig::new(kept))
}

/// The newest version of `id` the server confirmed, as it sent it; a version published here that waits for the server
/// is not the team's yet.
fn latest(connection: &rusqlite::Connection, id: ResourceId) -> Option<TeamVersion> {
  let mut statement = connection
    .prepare(
      "SELECT number, server_body, publisher_name, published_at \
       FROM resource_versions JOIN shared_version_bodies ON version_id = id \
       WHERE resource_id = ?1 AND id NOT IN (SELECT version_id FROM shared_publishes) \
       ORDER BY number DESC LIMIT 1",
    )
    .expect("the latest version");
  let mut rows = statement
    .query_map([id.to_string()], |row| {
      Ok((
        row.get::<_, i64>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, Option<String>>(2)?,
        row.get::<_, i64>(3)?,
      ))
    })
    .expect("read");
  let (number, body, publisher, at) = rows.next()?.expect("a version");
  let number = u32::try_from(number)
    .ok()
    .and_then(std::num::NonZeroU32::new)
    .expect("a version number");
  Some(TeamVersion {
    number: VersionNumber::new(number),
    body: serde_json::from_str(&body).expect("the server's body"),
    published_by: publisher.unwrap_or_default(),
    published_at: Timestamp::from_unix_millis(at),
  })
}

fn kind_of(text: &str) -> ResourceKind {
  match text {
    "agent_profile" => ResourceKind::AgentProfile,
    "instruction" => ResourceKind::Instruction,
    "pipeline" => ResourceKind::Pipeline,
    "tool" => ResourceKind::Tool,
    "skill" => ResourceKind::Skill,
    "team_reviewer" => ResourceKind::TeamReviewer,
    "mcp_server" => ResourceKind::McpServer,
    other => panic!("not a team resource kind: {other}"),
  }
}
