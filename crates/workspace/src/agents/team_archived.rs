//! What the team archived, as the Agents hub's *Archived* filter lists it (`Agents · Archived · admin · Restore` A36
//! `qvMZD`, `· member` M7 `Ml7HN`): every team resource the team archived, with who archived it and when (its last
//! change), the runs of this project its archive stopped, and what the team holds of it, which the panel shows
//! read-only until an admin restores it. What it holds is read as the hub reads a resource it lists; who uses it are
//! the profiles the team has not archived.

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
};
use orchester_store::{McpServerProgram, ProjectDb, StoreError};

use super::{
  local_hub::{self, StoppedRun},
  team_mcp::{self, TeamMcpSheet},
  team_profile::{self, TeamProfileSheet},
  team_text::{self, TeamInstructionSheet, TeamSkillSheet},
  team_tool::{self, TeamToolSheet},
};
use crate::team_config::{Changed, TeamConfig, TeamResource};

/// A team resource the team archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamArchived {
  pub id: ResourceId,
  pub kind: ResourceKind,
  pub name: String,
  /// Its latest version; `None` while the server has numbered none.
  pub number: Option<VersionNumber>,
  /// Who archived it and when: its last change.
  pub archived: Changed,
  /// The runs of this project its archive stopped, oldest first.
  pub stopped: Vec<StoppedRun>,
  /// What the team holds of it; `None` for a latest version this computer cannot read.
  pub sheet: Option<ArchivedSheet>,
}

/// What the team holds of an archived resource, as the panel of its kind reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchivedSheet {
  Profile(Box<TeamProfileSheet>),
  McpServer(Box<TeamMcpSheet>),
  Tool(Box<TeamToolSheet>),
  Instruction(Box<TeamInstructionSheet>),
  Skill(Box<TeamSkillSheet>),
}

impl ArchivedSheet {
  /// The team's profiles, not archived, that still name it; a profile is named by none.
  pub fn used_by(&self) -> &[String] {
    match self {
      Self::Profile(_) => &[],
      Self::McpServer(sheet) => &sheet.used_by,
      Self::Tool(sheet) => &sheet.used_by,
      Self::Instruction(sheet) => &sheet.used_by,
      Self::Skill(sheet) => &sheet.used_by,
    }
  }
}

/// The kinds the hub lists.
const KINDS: [ResourceKind; 5] = [
  ResourceKind::AgentProfile,
  ResourceKind::McpServer,
  ResourceKind::Tool,
  ResourceKind::Instruction,
  ResourceKind::Skill,
];

/// Every resource of a kind the hub lists that the team archived, the latest archived first. `profiles` are the sheets
/// of the profiles the team has not archived, which say who still uses one.
pub(super) fn archived(
  (db, config): (&ProjectDb, &TeamConfig),
  programs: &[McpServerProgram],
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Result<Vec<TeamArchived>, StoreError> {
  let mut found = Vec::new();
  for kind in KINDS {
    let resources: Vec<&TeamResource> = config.archived_of(kind).collect();
    let sheets = sheets((db, config), kind, &resources, (programs, profiles))?;
    for resource in resources {
      found.push(TeamArchived {
        id: resource.id,
        kind,
        name: resource.name.as_str().to_owned(),
        number: resource.number(),
        archived: resource.updated.clone(),
        stopped: local_hub::stopped(db, &db.archive_stops_of_resource(resource.id)?)?,
        sheet: sheets
          .iter()
          .find_map(|(id, sheet)| (*id == resource.id).then(|| sheet.clone())),
      });
    }
  }
  found.sort_by_key(|archived| std::cmp::Reverse(archived.archived.at));
  Ok(found)
}

/// What the team holds of each of `resources`, all of `kind`, by id.
fn sheets(
  (db, config): (&ProjectDb, &TeamConfig),
  kind: ResourceKind,
  resources: &[&TeamResource],
  (programs, profiles): (&[McpServerProgram], &[(ResourceId, TeamProfileSheet)]),
) -> Result<Vec<(ResourceId, ArchivedSheet)>, StoreError> {
  fn boxed<T>(sheets: Vec<(ResourceId, T)>, as_sheet: fn(Box<T>) -> ArchivedSheet) -> Vec<(ResourceId, ArchivedSheet)> {
    sheets
      .into_iter()
      .map(|(id, sheet)| (id, as_sheet(Box::new(sheet))))
      .collect()
  }
  Ok(match kind {
    ResourceKind::AgentProfile => boxed(team_profile::sheets((db, config), resources)?, ArchivedSheet::Profile),
    ResourceKind::McpServer => boxed(
      team_mcp::sheets((db, config), resources, programs, profiles)?,
      ArchivedSheet::McpServer,
    ),
    ResourceKind::Tool => boxed(team_tool::sheets(config, resources, profiles), ArchivedSheet::Tool),
    ResourceKind::Instruction => boxed(
      team_text::instructions(config, resources, profiles),
      ArchivedSheet::Instruction,
    ),
    ResourceKind::Skill => boxed(team_text::skills(config, resources, profiles), ArchivedSheet::Skill),
    _ => Vec::new(),
  })
}
