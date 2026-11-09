//! What a local profile's panel says of the instructions, skills and tools it gives its agents
//! (`docs/design/team-configuration.md`, "Decided 2026-11-09"): each it chooses by name, an archived one too, which it
//! keeps choosing until a person takes it back, and the project's others its *Add* menus offer. A tool is offered only
//! when an agent can call it, as a check ([`Tool::is_check`]): a profile that gave another would give nothing.

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, Tool},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::local_edit::GivenKind;

/// A resource a profile can give, by id and name.
pub type Named = (ResourceId, String);

/// Instructions, skills and tools, each by id and name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GivenLists {
  pub instructions: Vec<Named>,
  pub skills: Vec<Named>,
  pub tools: Vec<Named>,
}

impl GivenLists {
  pub fn of(&self, kind: GivenKind) -> &[Named] {
    match kind {
      GivenKind::Instruction => &self.instructions,
      GivenKind::Skill => &self.skills,
      GivenKind::Tool => &self.tools,
    }
  }

  fn by(list: impl Fn(GivenKind) -> Vec<Named>) -> Self {
    Self {
      instructions: list(GivenKind::Instruction),
      skills: list(GivenKind::Skill),
      tools: list(GivenKind::Tool),
    }
  }
}

/// The project's instructions, skills and tools, by kind, each in name order.
pub(super) struct GivenNames {
  /// Those not archived, which a profile can be given.
  offered: [Vec<Named>; 3],
  /// Those archived, which a profile keeps choosing until it drops them.
  archived: [Vec<Named>; 3],
}

impl GivenNames {
  pub(super) fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    let named = |resources: Vec<orchester_core::resource::Resource>| -> Vec<Named> {
      let mut named: Vec<Named> = resources
        .iter()
        .map(|resource| (resource.id(), resource.name().to_string()))
        .collect();
      named.sort_by_key(|(_, name)| name.to_lowercase());
      named
    };
    let mut offered: [Vec<Named>; 3] = Default::default();
    let mut archived: [Vec<Named>; 3] = Default::default();
    for (at, kind) in GivenKind::ALL.into_iter().enumerate() {
      let mut resources = Vec::new();
      for resource in db.resources(kind.resource_kind())? {
        if kind != GivenKind::Tool || is_check(db, resource.id())? {
          resources.push(resource);
        }
      }
      offered[at] = named(resources);
      archived[at] = named(
        db.archived_local_resources(kind.resource_kind())?
          .into_iter()
          .map(|(resource, _)| resource)
          .collect(),
      );
    }
    Ok(Self { offered, archived })
  }

  /// What `profile` gives, each list in its order, by name; one the project no longer has at all is left out.
  pub(super) fn chosen(&self, profile: &AgentProfile) -> GivenLists {
    GivenLists::by(|kind| {
      let at = position(kind);
      kind
        .of(profile)
        .ids()
        .iter()
        .filter_map(|id| {
          self.offered[at]
            .iter()
            .chain(&self.archived[at])
            .find(|(found, _)| found == id)
            .cloned()
        })
        .collect()
    })
  }

  /// The project's instructions, skills and tools that are not archived and that `profile` does not give yet.
  pub(super) fn offered(&self, profile: &AgentProfile) -> GivenLists {
    GivenLists::by(|kind| {
      self.offered[position(kind)]
        .iter()
        .filter(|(id, _)| !kind.of(profile).contains(*id))
        .cloned()
        .collect()
    })
  }
}

/// Whether the tool `id`'s latest published version is a check an agent can call; one never published is not.
pub(crate) fn is_check(db: &ProjectDb, id: ResourceId) -> Result<bool, StoreError> {
  Ok(
    db.latest_version::<Tool>(id)?
      .is_some_and(|version| version.body().is_check()),
  )
}

const fn position(kind: GivenKind) -> usize {
  match kind {
    GivenKind::Instruction => 0,
    GivenKind::Skill => 1,
    GivenKind::Tool => 2,
  }
}
