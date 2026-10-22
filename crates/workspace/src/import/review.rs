//! Resolve dependencies (`xp4dr`, `jsMUX` in Import resources `Dzzks`): what each item of the plan is, what can
//! be chosen for it and what that choice means in this project, and what keeps the import from going on. The
//! screen words these; the rules are here.

use orchester_core::{
  connection::{AgentProvider, ConnectionKind, mcp::McpTransport},
  import::{Catalog, CatalogResource, Choice, Content, Entity, Item, Pick, Plan, Problem, Resolution, Role},
  name::NameError,
  resource::{ResourceKind, ToolKind, VersionNumber},
  secret::Environment,
};

use super::data::Catalogs;

/// What an item of the plan is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
  Pipeline,
  AgentProfile,
  Instruction,
  Tool(ToolKind),
  /// An agent CLI or API connection, or GitHub (`None`).
  Connection(Option<AgentProvider>),
  /// An MCP server a CLI starts (`false`) or one reached over HTTP (`true`).
  McpServer {
    remote: bool,
  },
  Secret(Environment),
}

impl Subject {
  /// What a resource's `content` is.
  pub const fn of_content(content: &Content) -> Self {
    match content {
      Content::Pipeline(_) => Self::Pipeline,
      Content::AgentProfile(_) => Self::AgentProfile,
      Content::Instruction(_) => Self::Instruction,
      Content::Tool(tool) => Self::Tool(tool.kind()),
    }
  }

  /// The kind of resource it is; `None` for a connection, an MCP server or a secret.
  pub const fn resource_kind(self) -> Option<ResourceKind> {
    match self {
      Self::Pipeline => Some(ResourceKind::Pipeline),
      Self::AgentProfile => Some(ResourceKind::AgentProfile),
      Self::Instruction => Some(ResourceKind::Instruction),
      Self::Tool(_) => Some(ResourceKind::Tool),
      Self::Connection(_) | Self::McpServer { .. } | Self::Secret(_) => None,
    }
  }
}

/// What `entity` is in `catalog`, with the content `pick` of a resource; `None` when the catalog does not have it.
pub fn subject(catalog: &Catalog, entity: Entity, pick: Option<Pick>) -> Option<Subject> {
  match entity {
    Entity::Resource(id) => {
      let entry = catalog.resource(id)?;
      entry
        .content(pick.unwrap_or_else(|| entry.default_pick()))
        .map(Subject::of_content)
    }
    Entity::Connection(id) => catalog.connection(id).map(|connection| match connection.kind() {
      ConnectionKind::Agent(provider) => Subject::Connection(Some(*provider)),
      ConnectionKind::GitHub { .. } => Subject::Connection(None),
    }),
    Entity::McpServer(id) => catalog.mcp_server(id).map(|server| Subject::McpServer {
      remote: matches!(server.transport(), McpTransport::Http(_)),
    }),
    Entity::Secret(id) => catalog.secret(id).map(|secret| Subject::Secret(secret.environment())),
  }
}

/// The name of `entity` in `catalog`; empty when the catalog does not have it.
pub fn name(catalog: &Catalog, entity: Entity) -> String {
  let name = match entity {
    Entity::Resource(id) => catalog.resource(id).map(|entry| entry.resource().name().as_str()),
    Entity::Connection(id) => catalog.connection(id).map(|connection| connection.name().as_str()),
    Entity::McpServer(id) => catalog.mcp_server(id).map(|server| server.name().as_str()),
    Entity::Secret(id) => catalog.secret(id).map(|secret| secret.name().as_str()),
  };
  name.unwrap_or_default().to_owned()
}

/// The name an item's copy gets; `None` while it is not copied or its typed name is not valid.
pub fn copy_name(item: &Item) -> Option<&str> {
  match item.resolution() {
    Resolution::Copy { name, .. } | Resolution::Rename(Some((name, _))) => Some(name.as_str()),
    Resolution::Rename(None) | Resolution::Use(_) | Resolution::Unresolved | Resolution::LeftOut => None,
  }
}

/// How an item is resolved in this project, and what that means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
  /// A copy under the source's name: a new local resource of this kind.
  Ready(Option<ResourceKind>),
  /// A copy under the typed name; `taken`: this project already has the source's name.
  Renamed {
    taken: bool,
    source: String,
  },
  /// The copy's name is this project's (`in_target`) or another copy's in this import.
  NameTaken {
    name: String,
    in_target: bool,
  },
  NameNeeded,
  NameTooLong {
    max: usize,
  },
  /// This project's resource replaces it.
  Reused {
    name: String,
  },
  /// This project's connection, MCP server or secret replaces it.
  Mapped {
    name: String,
  },
  /// This project's resource, connection or secret chosen for it is gone.
  ChooseAgain,
  Unresolved,
  LeftOut,
}

pub fn status(catalogs: &Catalogs, item: &Item) -> Status {
  let source = name(&catalogs.source, item.source());
  match (item.problem(), item.resolution()) {
    (Some(Problem::NameTaken), _) => {
      let name = copy_name(item).map_or_else(|| source.clone(), str::to_owned);
      let in_target = target_has_name(catalogs, item.source(), &name);
      Status::NameTaken { name, in_target }
    }
    (Some(Problem::InvalidName(NameError::Blank)), _) => Status::NameNeeded,
    (Some(Problem::InvalidName(NameError::TooLong { .. })), _) => Status::NameTooLong {
      max: orchester_core::name::Name::MAX_CHARS,
    },
    (Some(Problem::TargetMissing), _) => Status::ChooseAgain,
    (None, Resolution::Copy { .. }) => {
      Status::Ready(subject(&catalogs.source, item.source(), item.pick()).and_then(Subject::resource_kind))
    }
    (None, Resolution::Rename(_)) => Status::Renamed {
      taken: target_has_name(catalogs, item.source(), &source),
      source,
    },
    (None, Resolution::Use(entity @ Entity::Resource(_))) => Status::Reused {
      name: name(&catalogs.target, *entity),
    },
    (None, Resolution::Use(entity)) => Status::Mapped {
      name: name(&catalogs.target, *entity),
    },
    (None, Resolution::Unresolved) => Status::Unresolved,
    (None, Resolution::LeftOut) => Status::LeftOut,
  }
}

/// Whether this project has a resource of `entity`'s kind called `wanted`, ignoring case.
fn target_has_name(catalogs: &Catalogs, entity: Entity, wanted: &str) -> bool {
  let Entity::Resource(id) = entity else {
    return false;
  };
  let Some(kind) = catalogs.source.resource(id).map(|entry| entry.resource().kind()) else {
    return false;
  };
  let wanted = wanted.to_lowercase();
  catalogs
    .target
    .resources()
    .iter()
    .map(CatalogResource::resource)
    .any(|existing| existing.kind() == kind && existing.name().as_str().to_lowercase() == wanted)
}

/// What a choice of an item's menu reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offer {
  /// A copy under the source's name.
  Copy(String),
  Rename,
  /// This project's resource, at its latest published version (`None`: a draft only).
  UseResource {
    name: String,
    published: Option<VersionNumber>,
  },
  /// This project's connection or MCP server.
  UseConnection(String),
  UseSecret {
    name: String,
    environment: Environment,
  },
  Unresolved,
  LeaveOut,
}

/// What a choice means, under it in the menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OfferDetail {
  /// This project already has a resource of the kind with this name.
  TargetHasName(String),
  /// The copy's name is typed in the field beside the menu.
  NameBeside,
  /// The copies show it as missing.
  MissingInCopies,
  NotImported,
}

/// The menu entry of `choice` for `item`.
pub fn offer(catalogs: &Catalogs, item: &Item, choice: Choice) -> (Offer, Option<OfferDetail>) {
  let source = name(&catalogs.source, item.source());
  match choice {
    Choice::Copy => {
      let taken = target_has_name(catalogs, item.source(), &source);
      (
        Offer::Copy(source.clone()),
        taken.then_some(OfferDetail::TargetHasName(source)),
      )
    }
    Choice::Rename => (Offer::Rename, Some(OfferDetail::NameBeside)),
    Choice::Use(entity) => (use_offer(&catalogs.target, entity), None),
    Choice::Unresolved => (Offer::Unresolved, Some(OfferDetail::MissingInCopies)),
    Choice::LeaveOut => (Offer::LeaveOut, Some(OfferDetail::NotImported)),
  }
}

fn use_offer(target: &Catalog, entity: Entity) -> Offer {
  let name = name(target, entity);
  match entity {
    Entity::Resource(id) => Offer::UseResource {
      name,
      published: target.resource(id).and_then(CatalogResource::published),
    },
    Entity::Connection(_) | Entity::McpServer(_) => Offer::UseConnection(name),
    Entity::Secret(id) => Offer::UseSecret {
      name,
      environment: target
        .secret(id)
        .map_or(Environment::NEW_SECRET, |secret| secret.environment()),
    },
  }
}

/// A choice not available yet, which the menu shows locked: importing cannot create secrets or authorize
/// connections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Planned {
  /// Copy the secret's value into this project's keychain.
  ImportSecret,
  /// Enter a value for this project.
  SupplyValue,
  /// Add the MCP server to this project, checked there.
  Authorize,
}

/// The choices of `item`'s menu that are planned but not available.
pub const fn planned(item: &Item) -> &'static [Planned] {
  match item.source() {
    Entity::Secret(_) => &[Planned::ImportSecret, Planned::SupplyValue],
    Entity::McpServer(_) => &[Planned::Authorize],
    Entity::Resource(_) | Entity::Connection(_) => &[],
  }
}

/// What the item that keeps the import from going on needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocker {
  /// Its copy's name is taken.
  Rename,
  /// Its copy needs a valid name.
  Name,
  /// What was chosen for it is gone.
  ChooseAgain,
}

/// Whether the import can go on to Create drafts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
  /// The first item with a problem, by its source name.
  Blocked { blocker: Blocker, name: String },
  /// Ready; `unresolved`: some connections, MCP servers or secrets are left unresolved.
  Ready { unresolved: bool },
  /// Nothing is selected.
  Empty,
}

pub fn readiness(catalogs: &Catalogs, plan: &Plan) -> Readiness {
  if let Some(item) = plan.blocker() {
    let blocker = match item.problem() {
      Some(Problem::NameTaken) => Blocker::Rename,
      Some(Problem::InvalidName(_)) => Blocker::Name,
      Some(Problem::TargetMissing) | None => Blocker::ChooseAgain,
    };
    return Readiness::Blocked {
      blocker,
      name: name(&catalogs.source, item.source()),
    };
  }
  if plan.is_ready() {
    Readiness::Ready {
      unresolved: plan.counts().unresolved > 0,
    }
  } else {
    Readiness::Empty
  }
}

/// The selection the import is about, after the two projects' names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
  /// One resource and the content its copy takes.
  One {
    name: String,
    pick: Option<Pick>,
  },
  Several(usize),
}

pub fn selection(catalogs: &Catalogs, plan: &Plan) -> Selection {
  let selected: Vec<&Item> = plan
    .items()
    .iter()
    .filter(|item| item.role() == Role::Selected)
    .collect();
  match selected.as_slice() {
    [item] => Selection::One {
      name: name(&catalogs.source, item.source()),
      pick: item.pick(),
    },
    several => Selection::Several(several.len()),
  }
}

/// How many of the plan's items were not selected: what the selection needs resolved.
pub fn dependencies(plan: &Plan) -> usize {
  plan.items().iter().filter(|item| item.role() != Role::Selected).count()
}
