//! What else reaches a profile's prompts (*Also in this profile's prompts*, 01c, ORC-107): the published project rules
//! that select its agents, the published rules of its role, and the published project and role skills. Pipeline and
//! step rules and skills reach it only through the steps that run it, which *Used in pipelines* lists.

use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{AgentTargets, Instruction, InstructionScope, ResourceKind, Skill},
  store::ResourceStore,
};

/// A published instruction or skill by its name, with where it applies.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Source {
  name: Arc<str>,
  scope: InstructionScope,
  /// Which agents a rule selects; every one for a skill.
  targets: AgentTargets,
}

/// The project's published rules and skills, read once for every profile the screen shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptSources {
  rules: Vec<Source>,
  skills: Vec<Source>,
}

/// What reaches one profile's prompts besides its own role instructions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfilePrompts {
  pub project_rules: Vec<Arc<str>>,
  pub role_rules: Vec<Arc<str>>,
  pub project_skills: Vec<Arc<str>>,
  pub role_skills: Vec<Arc<str>>,
}

impl PromptSources {
  pub fn load<S: ResourceStore>(store: &S) -> Result<Self, S::Error> {
    let mut sources = Self::default();
    for resource in store.resources(ResourceKind::Instruction)? {
      if let Some(version) = store.latest_version::<Instruction>(resource.id())? {
        sources.rules.push(Source {
          name: resource.name().as_str().into(),
          scope: version.body().scope().clone(),
          targets: version.body().targets().clone(),
        });
      }
    }
    for resource in store.resources(ResourceKind::Skill)? {
      if let Some(version) = store.latest_version::<Skill>(resource.id())? {
        sources.skills.push(Source {
          name: resource.slug().as_str().into(),
          scope: version.body().scope().clone(),
          targets: AgentTargets::All,
        });
      }
    }
    Ok(sources)
  }

  /// What reaches the agents of profile `id` with `role`.
  pub fn for_profile(&self, id: ResourceId, role: &Name) -> ProfilePrompts {
    let selects = |targets: &AgentTargets| match targets {
      AgentTargets::All => true,
      AgentTargets::Roles(roles) => roles.contains(role),
      AgentTargets::Profiles(profiles) => profiles.contains(&id),
    };
    let names = |sources: &[Source], wanted: &dyn Fn(&Source) -> bool| -> Vec<Arc<str>> {
      sources
        .iter()
        .filter(|source| wanted(source))
        .map(|source| source.name.clone())
        .collect()
    };
    let project = |source: &Source| source.scope == InstructionScope::Project && selects(&source.targets);
    let of_role = |source: &Source| matches!(&source.scope, InstructionScope::AgentRole { role: of } if of == role);
    ProfilePrompts {
      project_rules: names(&self.rules, &project),
      role_rules: names(&self.rules, &of_role),
      project_skills: names(&self.skills, &project),
      role_skills: names(&self.skills, &of_role),
    }
  }
}
