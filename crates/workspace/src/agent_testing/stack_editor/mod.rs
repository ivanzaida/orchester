//! The stack editor (`03 · Stack editor` `iNxTM`) as the window reads and saves it: a stack's draft or latest version
//! with the project's services and test suites, its fields ([`StackForm`]), the order its services start in with what
//! each waits for ([`order`]), and saving its draft and publishing it through the project database, as
//! `save_stack_draft` and `publish_stack` do. A stack starts only published services, so neither widens what runs
//! start. Every function here blocks on SQLite; the UI calls them off the UI thread.

pub mod order;

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TestSuite, VersionNumber},
  service::{Service, ServiceError, ServiceName, Stack, StackName},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  data::{Entry, create, entries, stack_slug},
  editor::Saved,
};
use crate::{
  drafts::{revision_of, save_draft_expecting_in},
  messages::{Message, store_error},
};

/// A stack as its page opens it, with the project's services and suites, each its draft if it has one, otherwise its
/// latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackRead {
  /// The stack; `None` for a new one.
  pub entry: Option<Entry<Stack>>,
  /// The latest version, which publishing compares the draft with.
  pub latest: Option<Stack>,
  /// What its draft is saved on ([`crate::drafts::draft_revision`]); a save made on it goes over no other change.
  pub revision: Option<String>,
  pub services: Vec<Entry<Service>>,
  /// The suites, which may need the stack by its name.
  pub suites: Vec<Entry<TestSuite>>,
}

impl StackRead {
  /// The bodies of the project's services, which a start resolves the stack's needs from.
  pub fn bodies(&self) -> Vec<Service> {
    self.services.iter().map(|entry| entry.body.clone()).collect()
  }

  /// The suites that need the stack named `name`, by their names.
  pub fn used_by(&self, name: &str) -> Vec<&str> {
    self
      .suites
      .iter()
      .filter(|suite| suite.body.needs().as_slice().iter().any(|need| need.as_str() == name))
      .map(|suite| &*suite.name)
      .collect()
  }
}

/// What the page of stack `id`, or of a new one, opens with.
pub fn load_stack_in(root: &Path, id: Option<ResourceId>) -> Result<StackRead, Message> {
  let read = || -> Result<StackRead, orchester_store::StoreError> {
    let db = ProjectDb::open(root)?;
    let services = entries::<Service, _>(&db)?;
    let suites = entries::<TestSuite, _>(&db)?;
    let Some(id) = id else {
      return Ok(StackRead {
        entry: None,
        latest: None,
        revision: None,
        services,
        suites,
      });
    };
    let entry = entries::<Stack, _>(&db)?.into_iter().find(|entry| entry.id == id);
    let latest = db.latest_version::<Stack>(id)?.map(|version| version.body().clone());
    let revision = revision_of::<Stack>(&db, id)?;
    Ok(StackRead {
      entry,
      latest,
      revision: Some(revision),
      services,
      suites,
    })
  };
  let read = read().map_err(|error| store_error(&error))?;
  if id.is_some() && read.entry.is_none() {
    return Err(Message::new("services.stack.errors.gone"));
  }
  Ok(read)
}

/// The fields as the person left them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackForm {
  /// Only a new stack's; a stack keeps the name it was made with.
  pub name: String,
  /// The services the person chose, in the order they were added.
  pub services: Vec<String>,
}

/// Why the fields do not make a stack yet, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackProblems {
  pub name: Option<Message>,
  pub services: Option<Message>,
}

impl StackForm {
  /// The fields of `stack` as stored.
  pub fn of(stack: &Stack) -> Self {
    Self {
      name: stack.name().as_str().to_owned(),
      services: stack.services().iter().map(|name| name.as_str().to_owned()).collect(),
    }
  }

  /// The stack these fields describe. `base` is the stored stack, whose name it keeps; without one, a new stack under
  /// the typed name.
  pub fn stack(&self, base: Option<&Stack>) -> Result<Stack, Box<StackProblems>> {
    let mut problems = StackProblems::default();
    let name = match base {
      Some(base) => Ok(base.name().clone()),
      None => stack_name(&self.name),
    };
    let name = name.map_err(|error| problems.name = Some(error)).ok();
    let services = self
      .services
      .iter()
      .map(|service| ServiceName::new(service).map_err(|error| stack_error(&error)))
      .collect::<Result<Vec<_>, _>>()
      .map_err(|error| problems.services = Some(error))
      .ok();
    let (Some(name), Some(services)) = (name, services) else {
      return Err(Box::new(problems));
    };
    Stack::new(name, services).map_err(|error| {
      Box::new(StackProblems {
        services: Some(stack_error(&error)),
        ..StackProblems::default()
      })
    })
  }
}

/// The name of a new stack as typed.
fn stack_name(text: &str) -> Result<StackName, Message> {
  let text = text.trim();
  if text.is_empty() {
    return Err(Message::new("services.stack.errors.nameRequired"));
  }
  StackName::new(text).map_err(|error| stack_error(&error))
}

/// Why a part of a stack is not valid.
pub fn stack_error(error: &ServiceError) -> Message {
  match error {
    ServiceError::Name(name) => Message::new("services.stack.errors.name")
      .with("name", name.clone())
      .with("max", ServiceName::MAX_CHARS.to_string()),
    ServiceError::EmptyStack(_) => Message::new("services.stack.errors.empty"),
    ServiceError::DuplicateStackService { service, .. } => {
      Message::new("services.stack.errors.duplicate").with("name", service.to_string())
    }
    _ => Message::new("services.stack.errors.invalid"),
  }
}

/// Saves `stack` as the draft of stack `id` while its draft is the one `expected` names, or makes a new stack under its
/// name, unique among the project's stacks (removed ones too).
pub fn save_stack_in(
  root: &Path,
  id: Option<ResourceId>,
  stack: Stack,
  expected: Option<&str>,
) -> Result<Saved, Message> {
  let id = match id {
    Some(id) => {
      save_draft_expecting_in(root, id, stack, expected).map_err(|error| store_error(&error))?;
      id
    }
    None => {
      let mut db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
      let slug = stack_slug(stack.name());
      if db
        .slugs(ResourceKind::Stack)
        .map_err(|error| store_error(&error))?
        .contains(&slug)
      {
        return Err(Message::new("services.stack.errors.nameTaken").with("name", slug.as_str().to_owned()));
      }
      create(&mut db, slug, &stack, Timestamp::now()).map_err(|error| store_error(&error))?
    }
  };
  let db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
  let revision = revision_of::<Stack>(&db, id).map_err(|error| store_error(&error))?;
  Ok(Saved { id, revision })
}

/// Publishes the draft of stack `id` as its next version while it is still `expected`; refused while a service it
/// starts is not published or the needs form a cycle.
pub fn publish_stack_in(root: &Path, id: ResourceId, expected: &Stack) -> Result<VersionNumber, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
  db.publish_draft_matching(id, expected, Timestamp::now())
    .map(|version| version.number())
    .map_err(|error| store_error(&error))
}
