//! The project's skills as a composer's slash menu offers them (ORC-325; `Skills` in `Slash menu` `IQOLN`,
//! `docs/design/slash-menu.md`, "Skills and the chip"): each published skill at its latest published version, with
//! that version's description, in the library's order. A skill that was never published is not offered, and a draft
//! saved over a published version leaves the published one offered. Which agents can take a skill: those of a
//! connection this version runs agents on.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  resource::{ResourceKind, Skill, VersionNumber},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::{self, Message};

/// A published skill as the menu lists it and a composer's chip names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfferedSkill {
  pub id: ResourceId,
  /// The name agents know it by.
  pub slug: Arc<str>,
  /// Its latest published version's description.
  pub description: Arc<str>,
  pub version: VersionNumber,
}

/// The published skills `db` holds, in the library's order. Blocks on SQLite.
pub fn published(db: &ProjectDb) -> Result<Vec<OfferedSkill>, StoreError> {
  let mut offered = Vec::new();
  for resource in db.resources(ResourceKind::Skill)? {
    let Some(latest) = db.latest_version::<Skill>(resource.id())? else {
      continue;
    };
    offered.push(OfferedSkill {
      id: resource.id(),
      slug: resource.slug().as_str().into(),
      description: latest.body().description().trim().into(),
      version: latest.number(),
    });
  }
  Ok(offered)
}

/// [`published`] for the project at `root`, with a failure in the person's words. Blocks on SQLite.
pub fn published_in(root: &Path) -> Result<Vec<OfferedSkill>, Message> {
  ProjectDb::open(root)
    .and_then(|db| published(&db))
    .map_err(|error| messages::store_error(&error))
}

/// Whether every agent of `providers`, the connections a message may reach, can take a skill.
pub fn taken_by(providers: &[AgentProvider]) -> bool {
  providers.iter().all(|provider| provider.runs_agents())
}
