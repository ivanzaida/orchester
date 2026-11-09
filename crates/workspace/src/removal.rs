//! Discarding a draft (ORC-107), as the Agents hub and the domain tools ask for it; what removing a team's resource
//! changes besides its own list. A local project archives instead of removing ([`crate::local_archive`]). Every
//! function blocks on SQLite; the UI calls them off the UI thread.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{ResourceBody, ResourceKind, VersionNumber},
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use crate::{
  messages::{self, Message},
  tasks::outside::Outside,
};

/// What removing a resource of `kind` changed besides its own list: a profile's or tool's removal resets pipeline
/// drafts, and a team MCP server's is listed by the Connections window. No screen lists test suites, services or
/// stacks yet.
pub const fn changed_by_removal(kind: ResourceKind) -> &'static [Outside] {
  match kind {
    ResourceKind::AgentProfile => &[Outside::Profiles, Outside::Pipelines],
    ResourceKind::Tool => &[Outside::Tools, Outside::Pipelines],
    ResourceKind::Instruction => &[Outside::Instructions],
    ResourceKind::Skill => &[Outside::Skills],
    ResourceKind::Pipeline => &[Outside::Pipelines],
    ResourceKind::McpServer => &[Outside::Connections],
    ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack | ResourceKind::TeamReviewer => &[],
  }
}

/// Discards the draft of resource `id` while it is still `expected`: the latest published version is the resource
/// again, and that version's number comes back.
pub fn discard_in<B: ResourceBody>(root: &Path, id: ResourceId, expected: &B) -> Result<VersionNumber, Message> {
  ProjectDb::open(root)
    .and_then(|mut db| db.discard_draft_matching(id, expected))
    .map_err(|error| messages::store_error(&error))
}

/// The latest published version of resource `id`: what it is again once its draft is discarded. `None` while it was
/// never published.
pub fn published_in<B: ResourceBody + Clone>(root: &Path, id: ResourceId) -> Result<Option<B>, Message> {
  ProjectDb::open(root)
    .and_then(|db| db.latest_version::<B>(id))
    .map(|version| version.map(|version| version.body().clone()))
    .map_err(|error| messages::store_error(&error))
}
