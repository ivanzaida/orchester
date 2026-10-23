//! Select resources (`DsqQJ` in Import resources `Dzzks`): which of the source's resources a kind tab and the
//! search show, what each is and refers to, and which ones the selection takes along.

use orchester_core::{
  id::ResourceId,
  import::{CatalogResource, Choices, Content, Entity, Plan, Role},
  resource::{InstructionScope, ResourceKind, ToolKind},
};

use super::{data::Catalogs, review};

/// The source's resources of `kind` (every kind: `None`) whose name holds `search`, ignoring case and the
/// spaces around it.
pub fn shown<'a>(catalogs: &'a Catalogs, kind: Option<ResourceKind>, search: &str) -> Vec<&'a CatalogResource> {
  let search = search.trim().to_lowercase();
  catalogs
    .source
    .resources()
    .iter()
    .filter(|entry| kind.is_none_or(|kind| entry.resource().kind() == kind))
    .filter(|entry| search.is_empty() || entry.resource().name().as_str().to_lowercase().contains(&search))
    .collect()
}

/// How many of the source's resources are of `kind` (every kind: `None`).
pub fn count(catalogs: &Catalogs, kind: Option<ResourceKind>) -> usize {
  shown(catalogs, kind, "").len()
}

/// Which of the source's resources refer to a resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Users {
  Nobody,
  One(String),
  Many(usize),
}

/// Where an instruction applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
  Project,
  Role(String),
  /// A pipeline or one of its steps, by the pipeline's name.
  Pipeline(String),
}

/// What a resource is, as the line under its name says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Summary {
  /// Its steps, and what a copy of it alone needs resolved.
  Pipeline {
    steps: usize,
    dependencies: usize,
  },
  AgentProfile(Users),
  Instruction(Scope),
  Tool(ToolKind, Users),
}

/// A resource of the list, as Select resources shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
  pub summary: Summary,
  /// The names of what a copy of it alone refers to, in the order the plan resolves them.
  pub depends_on: Vec<String>,
  /// The selected resource whose import takes it along, while it is not selected itself.
  pub included_with: Option<String>,
}

/// How `entry` reads in the list while `selection` is the plan of what is selected.
pub fn line(catalogs: &Catalogs, entry: &CatalogResource, selection: &Plan) -> Line {
  let id = entry.resource().id();
  let alone = Plan::new(
    &catalogs.source,
    &catalogs.target,
    &[(id, entry.default_pick())],
    &Choices::default(),
  );
  let depends_on: Vec<String> = alone
    .items()
    .iter()
    .filter(|item| item.role() != Role::Selected)
    .map(|item| review::name(&catalogs.source, item.source()))
    .collect();
  Line {
    summary: summary(catalogs, entry, depends_on.len()),
    depends_on,
    included_with: included_with(catalogs, selection, id),
  }
}

fn summary(catalogs: &Catalogs, entry: &CatalogResource, dependencies: usize) -> Summary {
  let id = entry.resource().id();
  match entry.content(entry.default_pick()) {
    Some(Content::Pipeline(pipeline)) => Summary::Pipeline {
      steps: pipeline.steps().len(),
      dependencies,
    },
    Some(Content::Instruction(instruction)) => Summary::Instruction(match instruction.scope() {
      InstructionScope::Project => Scope::Project,
      InstructionScope::AgentRole { role } => Scope::Role(role.as_str().to_owned()),
      InstructionScope::Pipeline { pipeline } | InstructionScope::Step { pipeline, .. } => {
        Scope::Pipeline(review::name(&catalogs.source, Entity::Resource(*pipeline)))
      }
    }),
    Some(Content::Tool(tool)) => Summary::Tool(tool.kind(), users(catalogs, id)),
    Some(Content::AgentProfile(_)) | None => Summary::AgentProfile(users(catalogs, id)),
  }
}

fn users(catalogs: &Catalogs, id: ResourceId) -> Users {
  let users: Vec<&CatalogResource> = catalogs.source.users(Entity::Resource(id)).collect();
  match users.as_slice() {
    [] => Users::Nobody,
    [user] => Users::One(user.resource().name().as_str().to_owned()),
    several => Users::Many(several.len()),
  }
}

/// The selected resource whose plan takes `id` along, following what refers to it back to a selected one.
fn included_with(catalogs: &Catalogs, selection: &Plan, id: ResourceId) -> Option<String> {
  let mut item = selection
    .item(Entity::Resource(id))
    .filter(|item| item.role() != Role::Selected)?;
  // Each step goes to an earlier item of the plan, so it ends within as many steps as the plan has items.
  for _ in 0..selection.items().len() {
    let user = selection.item(Entity::Resource(*item.used_by().first()?))?;
    if user.role() == Role::Selected {
      return Some(review::name(&catalogs.source, user.source()));
    }
    item = user;
  }
  None
}
