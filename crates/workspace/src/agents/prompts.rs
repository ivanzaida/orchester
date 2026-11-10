//! What else reaches a profile's prompts (*Also in this profile's prompts*, 01c, ORC-107): the published instructions
//! and skills it gives its agents (`docs/design/team-configuration.md`, "Decided 2026-11-09"), which nothing else gives
//! them. One it gives that has no published version, or that the project no longer offers, reaches no prompt.

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, ChosenResources, Instruction, ResourceKind, Skill},
  store::ResourceStore,
};

/// The project's published instructions by name and skills by slug, read once for every profile the screen shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptSources {
  instructions: BTreeMap<ResourceId, Arc<str>>,
  skills: BTreeMap<ResourceId, Arc<str>>,
}

/// What reaches one profile's prompts besides its own role instructions, each in the order the profile gives them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfilePrompts {
  pub instructions: Vec<Arc<str>>,
  pub skills: Vec<Arc<str>>,
}

impl PromptSources {
  pub fn load<S: ResourceStore>(store: &S) -> Result<Self, S::Error> {
    let mut sources = Self::default();
    for resource in store.resources(ResourceKind::Instruction)? {
      if store.latest_version::<Instruction>(resource.id())?.is_some() {
        sources
          .instructions
          .insert(resource.id(), resource.name().as_str().into());
      }
    }
    for resource in store.resources(ResourceKind::Skill)? {
      if store.latest_version::<Skill>(resource.id())?.is_some() {
        sources.skills.insert(resource.id(), resource.slug().as_str().into());
      }
    }
    Ok(sources)
  }

  /// What reaches the agents of `profile`.
  pub fn for_profile(&self, profile: &AgentProfile) -> ProfilePrompts {
    let names = |given: &ChosenResources, published: &BTreeMap<ResourceId, Arc<str>>| -> Vec<Arc<str>> {
      given.ids().iter().filter_map(|id| published.get(id).cloned()).collect()
    };
    ProfilePrompts {
      instructions: names(profile.instruction_resources(), &self.instructions),
      skills: names(profile.skills(), &self.skills),
    }
  }
}
