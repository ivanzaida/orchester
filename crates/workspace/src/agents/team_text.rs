//! What the panels of a team instruction and a team skill show (`Agents · Instructions` `c3jU1R` A4, `Agents · Skills`
//! `c9YnL` A5, `D8WX9` M4): what the team holds of them (their text or files, where they are added, which agents get
//! them), who uses them and who published the version this computer uses. They hold nothing a computer fills in, so
//! there is no readiness here (`docs/design/team-configuration.md`, "Per resource kind").

use orchester_core::{
  id::ResourceId,
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, Skill},
};

use super::{
  team_body_edit::Latest,
  team_mcp::Published,
  team_profile::{Listed, TeamProfileSheet, listed},
  team_sheets::{self, latest_of},
};
use crate::team_config::{TeamConfig, TeamResource};

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
  /// Who published its latest version.
  pub published: Option<Published>,
  /// What an admin's edit starts from: its latest version.
  pub edit: Option<Latest<Instruction>>,
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
  /// What an admin's edit starts from: its latest version.
  pub edit: Option<Latest<Skill>>,
}

/// The skill's own document, which every skill has.
pub const SKILL_DOCUMENT: &str = "SKILL.md";

fn added(config: &TeamConfig, scope: &InstructionScope) -> Added {
  match scope {
    InstructionScope::Project => Added::Project,
    InstructionScope::AgentRole { role } => Added::Role(role.to_string()),
    InstructionScope::Pipeline { pipeline } => Added::Pipeline(listed(config, *pipeline)),
    InstructionScope::Step { pipeline, step } => Added::Step {
      pipeline: listed(config, *pipeline),
      step: step.to_string(),
    },
  }
}

fn reaches(config: &TeamConfig, targets: &AgentTargets) -> Reaches {
  match targets {
    AgentTargets::All => Reaches::Everyone,
    AgentTargets::Roles(roles) => Reaches::Roles(roles.iter().map(ToString::to_string).collect()),
    AgentTargets::Profiles(profiles) => Reaches::Profiles(profiles.iter().map(|id| listed(config, *id)).collect()),
  }
}

/// A sheet for each team instruction of `instructions` whose latest version is an instruction this computer reads.
pub(super) fn instructions(
  config: &TeamConfig,
  instructions: &[&TeamResource],
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Vec<(ResourceId, TeamInstructionSheet)> {
  let mut sheets = Vec::new();
  for resource in instructions.iter().copied() {
    let Some(Ok(body)) = resource.body::<Instruction>() else {
      continue;
    };
    sheets.push((
      resource.id,
      TeamInstructionSheet {
        text: body.text().as_str().to_owned(),
        added: added(config, body.scope()),
        reaches: reaches(config, body.targets()),
        events: body.events().iter().copied().collect(),
        used_by: team_sheets::used_by(config, profiles, |sheet| {
          sheet.instructions.iter().any(|used| used.id == resource.id)
        }),
        published: team_sheets::published(resource),
        edit: latest_of(resource, &body),
      },
    ));
  }
  sheets
}

/// A sheet for each team skill of `skills` whose latest version is a skill this computer reads.
pub(super) fn skills(
  config: &TeamConfig,
  skills: &[&TeamResource],
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Vec<(ResourceId, TeamSkillSheet)> {
  let mut sheets = Vec::new();
  for resource in skills.iter().copied() {
    let Some(Ok(body)) = resource.body::<Skill>() else {
      continue;
    };
    let files = std::iter::once(SKILL_DOCUMENT.to_owned())
      .chain(body.files().iter().map(|file| file.path.as_str().to_owned()))
      .collect();
    sheets.push((
      resource.id,
      TeamSkillSheet {
        description: body.description().to_owned(),
        files,
        added: added(config, body.scope()),
        used_by: team_sheets::used_by(config, profiles, |sheet| {
          sheet.skills.iter().any(|used| used.id == resource.id)
        }),
        published: team_sheets::published(resource),
        edit: latest_of(resource, &body),
      },
    ));
  }
  sheets
}
