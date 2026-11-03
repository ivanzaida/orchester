//! The project's skills as a composer's slash menu offers them (ORC-325; `Skills` in `Slash menu` `IQOLN`,
//! `docs/design/slash-menu.md`, "Skills and the chip"): each published skill at its latest published version, with
//! that version's description, in the library's order. A skill that was never published is not offered, and a draft
//! saved over a published version leaves the published one offered. Which agents can take a skill: those of a
//! connection this version runs agents on. A chosen one goes with the message (ORC-634, [`kept_with`]).

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::AgentProvider,
  conversation::skill::{ChosenSkill, carried},
  id::ResourceId,
  resource::{ResourceKind, Skill, Slug, VersionNumber},
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

impl OfferedSkill {
  /// The skill as a message carries it; `None` for a name that is no slug, which the project never holds.
  pub fn chosen(&self) -> Option<ChosenSkill> {
    Some(ChosenSkill {
      resource: self.id,
      slug: Slug::new(&self.slug).ok()?,
      version: self.version,
    })
  }

  /// The skill `stored`, a message as it was kept, carries, as its chip names it (without its description).
  pub fn carried_by(stored: &str) -> Option<Self> {
    let (chosen, _) = carried(stored)?;
    Some(Self {
      id: chosen.resource,
      slug: chosen.slug.as_str().into(),
      description: "".into(),
      version: chosen.version,
    })
  }
}

/// `words`, a person's message, as it is kept with `chosen`, the skill they chose for it, at the version chosen as
/// `db` holds it (`orchester_core::conversation::skill`); refused once the project no longer has that version.
/// Blocks on SQLite.
pub fn kept_with(db: &ProjectDb, chosen: &ChosenSkill, words: &str) -> Result<String, Message> {
  let version = db
    .version::<Skill>(chosen.resource, chosen.version)
    .map_err(|error| messages::store_error(&error))?
    .ok_or_else(|| Message::new("skills.chosenGone"))?;
  Ok(chosen.kept_with(version.body(), words))
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
