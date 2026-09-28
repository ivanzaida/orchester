//! The plan of an import (06.05 `QqnDd`): the selected resources and everything their copies
//! refer to, each resolved in the target project, and what keeps the import from starting.

use std::collections::{BTreeMap, HashMap};

use super::{
  Entity,
  catalog::{Catalog, CatalogResource, Content, Pick},
  references,
};
use crate::{
  id::ResourceId,
  name::{Name, NameError},
  resource::{InstructionScope, ResourceKind, Slug},
};

/// What the user chose for an item of the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Choice {
  /// Copy the resource under its own name.
  Copy,
  /// Copy the resource under the name typed for it ([`Choices::rename`]).
  Rename,
  /// Use this resource, connection or secret of the target instead.
  Use(Entity),
  /// Leave a connection, MCP server or secret unresolved: the copies keep naming the source's, which the
  /// target does not have, and show it as missing.
  Unresolved,
  /// Do not import an instruction written for a copied pipeline.
  LeaveOut,
}

/// The user's choices so far, by source entity; an item without one gets its default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choices {
  chosen: BTreeMap<Entity, Choice>,
  names: BTreeMap<Entity, String>,
}

impl Choices {
  pub fn choose(&mut self, entity: Entity, choice: Choice) {
    self.chosen.insert(entity, choice);
  }

  /// The name typed for a renamed copy of `entity`, as typed.
  pub fn rename(&mut self, entity: Entity, name: String) {
    self.names.insert(entity, name);
  }

  /// The name typed for `entity`, if one was.
  pub fn name(&self, entity: Entity) -> Option<&str> {
    self.names.get(&entity).map(String::as_str)
  }
}

/// Why an item is in the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
  Selected,
  /// A copy refers to it.
  Dependency,
  /// An instruction for a copied pipeline or one of its steps.
  PipelineInstruction,
}

/// How an item is resolved in the target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
  /// A copy under the source's name, with a slug free in the target.
  Copy {
    name: Name,
    slug: Slug,
  },
  /// A copy under the typed name; `None` while that is not a valid name.
  Rename(Option<(Name, Slug)>),
  Use(Entity),
  Unresolved,
  LeftOut,
}

/// What keeps an import from starting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
  /// A resource of the same kind in the target, or another copy, already has the copy's name.
  NameTaken,
  /// The typed name is blank or too long.
  InvalidName(NameError),
  /// The target has no resource, connection or secret like the one chosen (any more).
  TargetMissing,
}

/// One source resource, connection or secret and how it is resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
  source: Entity,
  role: Role,
  pick: Option<Pick>,
  options: Vec<Choice>,
  choice: Choice,
  resolution: Resolution,
  problem: Option<Problem>,
  used_by: Vec<ResourceId>,
}

impl Item {
  pub const fn source(&self) -> Entity {
    self.source
  }

  pub const fn role(&self) -> Role {
    self.role
  }

  /// The content a copy of a resource takes; `None` for connections and secrets.
  pub const fn pick(&self) -> Option<Pick> {
    self.pick
  }

  /// What the user can choose for the item, in the order a menu lists them.
  pub fn options(&self) -> &[Choice] {
    &self.options
  }

  /// The choice in effect: the user's, or the default.
  pub const fn choice(&self) -> Choice {
    self.choice
  }

  pub const fn resolution(&self) -> &Resolution {
    &self.resolution
  }

  pub const fn problem(&self) -> Option<&Problem> {
    self.problem.as_ref()
  }

  /// The source resources whose copies refer to it, or the pipeline an instruction is for.
  pub fn used_by(&self) -> &[ResourceId] {
    &self.used_by
  }

  /// Whether the import creates a resource for it.
  pub const fn copies(&self) -> bool {
    matches!(self.choice, Choice::Copy | Choice::Rename)
  }
}

/// How many items the import copies, resolves to the target's own and leaves unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
  pub copies: usize,
  pub reused: usize,
  pub unresolved: usize,
}

/// The selected resources, then everything their copies refer to, breadth first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
  items: Vec<Item>,
}

impl Plan {
  /// The plan for copying `selection` from `source` into `target` with `choices`. Selected ids
  /// the source does not have are skipped; a pick the resource does not have is its default.
  pub fn new(source: &Catalog, target: &Catalog, selection: &[(ResourceId, Pick)], choices: &Choices) -> Self {
    let mut builder = Builder {
      source,
      target,
      choices,
      items: Vec::new(),
      index: HashMap::new(),
    };
    for &(id, pick) in selection {
      if let Some(resource) = source.resource(id) {
        builder.add(
          Entity::Resource(id),
          Role::Selected,
          Some(resource.available(pick)),
          None,
        );
      }
    }
    let mut next = 0;
    while next < builder.items.len() {
      builder.follow(next);
      next += 1;
    }
    let mut items = builder.items;
    name_copies(&mut items, source, target, choices);
    Self { items }
  }

  pub fn items(&self) -> &[Item] {
    &self.items
  }

  pub fn item(&self, entity: Entity) -> Option<&Item> {
    self.items.iter().find(|item| item.source == entity)
  }

  /// The first item that keeps the import from starting.
  pub fn blocker(&self) -> Option<&Item> {
    self.items.iter().find(|item| item.problem.is_some())
  }

  /// Whether the import can start: it copies something and nothing keeps it from starting.
  pub fn is_ready(&self) -> bool {
    self.blocker().is_none() && self.items.iter().any(Item::copies)
  }

  pub fn counts(&self) -> Counts {
    self.items.iter().fold(Counts::default(), |mut counts, item| {
      match item.choice {
        Choice::Copy | Choice::Rename => counts.copies += 1,
        Choice::Use(_) => counts.reused += 1,
        Choice::Unresolved => counts.unresolved += 1,
        Choice::LeaveOut => {}
      }
      counts
    })
  }
}

struct Builder<'a> {
  source: &'a Catalog,
  target: &'a Catalog,
  choices: &'a Choices,
  items: Vec<Item>,
  index: HashMap<Entity, usize>,
}

impl Builder<'_> {
  /// Adds `entity` unless it is in the plan already; either way `used_by` refers to it.
  fn add(&mut self, entity: Entity, role: Role, pick: Option<Pick>, used_by: Option<ResourceId>) {
    if let Some(&position) = self.index.get(&entity) {
      let item = &mut self.items[position];
      if let Some(user) = used_by.filter(|user| !item.used_by.contains(user)) {
        item.used_by.push(user);
      }
      return;
    }
    let options = options(entity, role, self.source, self.target);
    let default = default_choice(entity, role, self.source, self.target);
    let (choice, problem) = match self.choices.chosen.get(&entity).copied() {
      Some(chosen) if options.contains(&chosen) => (chosen, None),
      Some(chosen @ Choice::Use(_)) => (chosen, Some(Problem::TargetMissing)),
      _ => (default, None),
    };
    let resolution = match choice {
      Choice::Use(target) => Resolution::Use(target),
      Choice::Unresolved => Resolution::Unresolved,
      Choice::LeaveOut => Resolution::LeftOut,
      // Named once every item is known (`name_copies`).
      Choice::Copy | Choice::Rename => Resolution::Rename(None),
    };
    self.index.insert(entity, self.items.len());
    self.items.push(Item {
      source: entity,
      role,
      pick,
      options,
      choice,
      resolution,
      problem,
      used_by: used_by.into_iter().collect(),
    });
  }

  /// Adds what the copy of the item at `position` refers to, and the instructions of a copied
  /// pipeline.
  fn follow(&mut self, position: usize) {
    let source = self.source;
    let item = &self.items[position];
    let (Entity::Resource(id), Some(pick), true) = (item.source, item.pick, item.copies()) else {
      return;
    };
    let Some(content) = source.resource(id).and_then(|resource| resource.content(pick)) else {
      return;
    };
    for uuid in references::values(&content.to_value()) {
      match source.entity(uuid) {
        Some(Entity::Resource(dependency)) if dependency == id => {}
        Some(entity) => {
          let pick = self.pick_of(entity);
          self.add(entity, Role::Dependency, pick, Some(id));
        }
        None => {}
      }
    }
    if matches!(content, Content::Pipeline(_)) {
      for instruction in instructions_for(source, id) {
        let entity = Entity::Resource(instruction.resource().id());
        self.add(
          entity,
          Role::PipelineInstruction,
          Some(instruction.default_pick()),
          Some(id),
        );
      }
    }
  }

  fn pick_of(&self, entity: Entity) -> Option<Pick> {
    match entity {
      Entity::Resource(id) => self.source.resource(id).map(CatalogResource::default_pick),
      Entity::Connection(_) | Entity::McpServer(_) | Entity::Secret(_) => None,
    }
  }
}

/// The source's instructions for `pipeline` or one of its steps, as last saved.
fn instructions_for(source: &Catalog, pipeline: ResourceId) -> impl Iterator<Item = &CatalogResource> {
  source.resources().iter().filter(move |resource| {
    let content = resource.content(resource.default_pick());
    matches!(content, Some(Content::Instruction(instruction)) if matches!(
      instruction.scope(),
      InstructionScope::Pipeline { pipeline: scoped } | InstructionScope::Step { pipeline: scoped, .. }
        if *scoped == pipeline
    ))
  })
}

/// What can be chosen for `entity`: a selected resource is copied, a resource a copy needs is
/// copied or replaced by one of the target's of its kind, an instruction for a copied pipeline
/// can be left out, and a connection, MCP server or secret is mapped to one of the target's (an
/// agent connection of the same kind) or left unresolved.
fn options(entity: Entity, role: Role, source: &Catalog, target: &Catalog) -> Vec<Choice> {
  let copies = [Choice::Copy, Choice::Rename];
  match (entity, role) {
    (Entity::Resource(_), Role::Selected) => copies.to_vec(),
    (Entity::Resource(_), Role::PipelineInstruction) => copies.into_iter().chain([Choice::LeaveOut]).collect(),
    (Entity::Resource(id), Role::Dependency) => {
      let kind = source.resource(id).map(|resource| resource.resource().kind());
      let uses = target
        .resources()
        .iter()
        .filter(|resource| Some(resource.resource().kind()) == kind)
        .map(|resource| Choice::Use(Entity::Resource(resource.resource().id())));
      copies.into_iter().chain(uses).collect()
    }
    (Entity::Connection(id), _) => {
      let kind = source.connection(id).map(|connection| connection.kind().clone());
      target
        .connections()
        .iter()
        .filter(|connection| Some(connection.kind()) == kind.as_ref())
        .map(|connection| Choice::Use(Entity::Connection(connection.id())))
        .chain([Choice::Unresolved])
        .collect()
    }
    (Entity::McpServer(_), _) => target
      .mcp_servers()
      .iter()
      .map(|server| Choice::Use(Entity::McpServer(server.id())))
      .chain([Choice::Unresolved])
      .collect(),
    (Entity::Secret(_), _) => target
      .secrets()
      .iter()
      .map(|secret| Choice::Use(Entity::Secret(secret.id())))
      .chain([Choice::Unresolved])
      .collect(),
  }
}

/// A resource a copy needs is the target's resource of its kind with its slug or name when there
/// is one, otherwise a copy; a connection, MCP server or secret is left for the user to map.
fn default_choice(entity: Entity, role: Role, source: &Catalog, target: &Catalog) -> Choice {
  match (entity, role) {
    (Entity::Resource(id), Role::Dependency) => source
      .resource(id)
      .and_then(|resource| counterpart(resource, target))
      .map_or(Choice::Copy, |existing| Choice::Use(Entity::Resource(existing))),
    (Entity::Resource(_), Role::Selected | Role::PipelineInstruction) => Choice::Copy,
    (Entity::Connection(_) | Entity::McpServer(_) | Entity::Secret(_), _) => Choice::Unresolved,
  }
}

fn counterpart(resource: &CatalogResource, target: &Catalog) -> Option<ResourceId> {
  let wanted = resource.resource();
  let same_kind = || {
    target
      .resources()
      .iter()
      .map(CatalogResource::resource)
      .filter(|existing| existing.kind() == wanted.kind())
  };
  same_kind()
    .find(|existing| existing.slug() == wanted.slug())
    .or_else(|| same_kind().find(|existing| same_name(existing.name(), wanted.name())))
    .map(|existing| existing.id())
}

fn same_name(one: &Name, other: &Name) -> bool {
  one.as_str().to_lowercase() == other.as_str().to_lowercase()
}

/// Names every copy, in plan order: a name the target or an earlier copy of the kind already has
/// is taken, and each copy gets a slug no resource of its kind has.
fn name_copies(items: &mut [Item], source: &Catalog, target: &Catalog, choices: &Choices) {
  let mut taken: HashMap<ResourceKind, (Vec<Name>, Vec<Slug>)> = HashMap::new();
  for existing in target.resources().iter().map(CatalogResource::resource) {
    let (names, slugs) = taken.entry(existing.kind()).or_default();
    names.push(existing.name().clone());
    slugs.push(existing.slug().clone());
  }
  for item in items.iter_mut().filter(|item| item.copies()) {
    let Entity::Resource(id) = item.source else {
      continue;
    };
    let Some(resource) = source.resource(id).map(CatalogResource::resource) else {
      continue;
    };
    let name = match item.choice {
      Choice::Rename => Name::new(choices.name(item.source).unwrap_or(resource.name().as_str())),
      _ => Ok(resource.name().clone()),
    };
    let name = match name {
      Ok(name) => name,
      Err(error) => {
        item.problem = Some(Problem::InvalidName(error));
        continue;
      }
    };
    let (names, slugs) = taken.entry(resource.kind()).or_default();
    if names.iter().any(|existing| same_name(existing, &name)) {
      item.problem = Some(Problem::NameTaken);
    }
    let slug = Slug::unique_for(&name, resource.slug(), slugs);
    names.push(name.clone());
    slugs.push(slug.clone());
    item.resolution = match item.choice {
      Choice::Rename => Resolution::Rename(Some((name, slug))),
      _ => Resolution::Copy { name, slug },
    };
  }
}
