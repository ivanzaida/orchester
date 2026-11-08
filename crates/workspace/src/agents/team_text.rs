//! What the panels of a team instruction and a team skill show (`Agents · Instructions` `c3jU1R` A4, `Agents · Skills`
//! `c9YnL` A5, `D8WX9` M4): what the team holds of them (their text or files, where they are added, which agents get
//! them), who uses them and who published the version this computer uses. They hold nothing a computer fills in, so
//! there is no readiness here (`docs/design/team-configuration.md`, "Per resource kind").

use orchester_core::{
  id::ResourceId,
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, ResourceKind, Skill},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  team_mcp::Published,
  team_profile::{Listed, TeamProfileSheet},
  team_sheets,
};

/// Where an instruction or a skill is added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Added {
  /// Every run of the project.
  Project,
  /// The runs of the agents with a role.
  Role(String),
  /// The runs of a pipeline.
  Pipeline(Listed),
  /// One step of a pipeline.
  Step { pipeline: Listed, step: String },
}

/// Which agents get an instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reaches {
  Everyone,
  Roles(Vec<String>),
  Profiles(Vec<Listed>),
}

/// One team instruction as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamInstructionSheet {
  pub text: String,
  pub added: Added,
  pub reaches: Reaches,
  /// The points of an agent's life it is injected at, in order.
  pub events: Vec<InjectionEvent>,
  /// The names of the team's profiles that use it.
  pub used_by: Vec<String>,
  /// `None` while the server has not confirmed the version this computer uses.
  pub published: Option<Published>,
}

/// One team skill as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamSkillSheet {
  pub description: String,
  /// Its paths: `SKILL.md`, then its reference files in path order.
  pub files: Vec<String>,
  pub added: Added,
  pub used_by: Vec<String>,
  pub published: Option<Published>,
}

/// The skill's own document, which every skill has.
pub const SKILL_DOCUMENT: &str = "SKILL.md";

fn listed(db: &ProjectDb, id: ResourceId) -> Result<Listed, StoreError> {
  let name = match db.resource(id)? {
    Some(found) => found.name().to_string(),
    None => id.to_string(),
  };
  Ok(Listed { id, name })
}

fn added(db: &ProjectDb, scope: &InstructionScope) -> Result<Added, StoreError> {
  Ok(match scope {
    InstructionScope::Project => Added::Project,
    InstructionScope::AgentRole { role } => Added::Role(role.to_string()),
    InstructionScope::Pipeline { pipeline } => Added::Pipeline(listed(db, *pipeline)?),
    InstructionScope::Step { pipeline, step } => Added::Step {
      pipeline: listed(db, *pipeline)?,
      step: step.to_string(),
    },
  })
}

fn reaches(db: &ProjectDb, targets: &AgentTargets) -> Result<Reaches, StoreError> {
  Ok(match targets {
    AgentTargets::All => Reaches::Everyone,
    AgentTargets::Roles(roles) => Reaches::Roles(roles.iter().map(ToString::to_string).collect()),
    AgentTargets::Profiles(profiles) => {
      Reaches::Profiles(profiles.iter().map(|id| listed(db, *id)).collect::<Result<_, _>>()?)
    }
  })
}

/// A sheet for each team instruction that has a version cached here.
pub(super) fn instructions(
  db: &ProjectDb,
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Result<Vec<(ResourceId, TeamInstructionSheet)>, StoreError> {
  let mut sheets = Vec::new();
  for resource in db.resources(ResourceKind::Instruction)? {
    let Some(version) = db.latest_version::<Instruction>(resource.id())? else {
      continue;
    };
    let body = version.body();
    sheets.push((
      resource.id(),
      TeamInstructionSheet {
        text: body.text().as_str().to_owned(),
        added: added(db, body.scope())?,
        reaches: reaches(db, body.targets())?,
        events: body.events().iter().copied().collect(),
        used_by: team_sheets::used_by(db, profiles, |sheet| {
          sheet.instructions.iter().any(|used| used.id == resource.id())
        })?,
        published: team_sheets::published::<Instruction>(db, resource.id(), version.number())?,
      },
    ));
  }
  Ok(sheets)
}

/// A sheet for each team skill that has a version cached here.
pub(super) fn skills(
  db: &ProjectDb,
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Result<Vec<(ResourceId, TeamSkillSheet)>, StoreError> {
  let mut sheets = Vec::new();
  for resource in db.resources(ResourceKind::Skill)? {
    let Some(version) = db.latest_version::<Skill>(resource.id())? else {
      continue;
    };
    let body = version.body();
    let files = std::iter::once(SKILL_DOCUMENT.to_owned())
      .chain(body.files().iter().map(|file| file.path.as_str().to_owned()))
      .collect();
    sheets.push((
      resource.id(),
      TeamSkillSheet {
        description: body.description().to_owned(),
        files,
        added: added(db, body.scope())?,
        used_by: team_sheets::used_by(db, profiles, |sheet| {
          sheet.skills.iter().any(|used| used.id == resource.id())
        })?,
        published: team_sheets::published::<Skill>(db, resource.id(), version.number())?,
      },
    ));
  }
  Ok(sheets)
}
