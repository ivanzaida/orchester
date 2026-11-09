//! The test suite editor (`04 · Test suite editor` `uxYcD`) as the window reads and saves it: a suite's draft or
//! latest version with the project's services and stacks, its fields ([`form`]), the services its needs start in
//! order, and saving its draft and publishing it through the project database. A suite runs its program with the
//! person's rights, as a service does. Every function here blocks on SQLite; the UI calls them off the UI thread.

pub mod form;

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TestSuite, VersionNumber},
  service::{Service, ServiceName, Stack, StartPlan},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::form::suite_name;
use super::{
  data::{Entry, create, entries},
  editor::{Saved, order::order_error},
};
use crate::{
  drafts::{revision_of, save_draft_expecting_in},
  messages::{Message, store_error},
};

/// A suite as its page opens it, with the project's services and stacks, each its draft if it has one, otherwise its
/// latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiteRead {
  /// The suite; `None` for a new one.
  pub entry: Option<Entry<TestSuite>>,
  /// The latest version, which publishing compares the draft with.
  pub latest: Option<TestSuite>,
  /// What its draft is saved on ([`crate::drafts::draft_revision`]).
  pub revision: Option<String>,
  pub services: Vec<Entry<Service>>,
  pub stacks: Vec<Entry<Stack>>,
}

impl SuiteRead {
  /// The stacks' names, which a need may name.
  pub fn stack_names(&self) -> Vec<&str> {
    self.stacks.iter().map(|stack| &*stack.name).collect()
  }

  /// The services a start of `needs` brings up, in start order: a need that names a stack starts its services, one
  /// that names a service starts it; each with what it needs, once.
  pub fn started(&self, needs: &[String]) -> Result<Vec<Arc<str>>, Message> {
    let mut wanted: Vec<ServiceName> = Vec::new();
    for need in needs {
      let named = match self.stacks.iter().find(|stack| *stack.name == **need) {
        Some(stack) => stack.body.services().to_vec(),
        None => vec![ServiceName::new(need).map_err(|_| Message::new("services.suite.errors.need"))?],
      };
      for service in named {
        if !wanted.contains(&service) {
          wanted.push(service);
        }
      }
    }
    let services: Vec<Service> = self.services.iter().map(|entry| entry.body.clone()).collect();
    let plan = StartPlan::for_services(&services, &wanted).map_err(|error| match error {
      orchester_core::service::NeedsError::Missing { name, needed_by: None } => {
        Message::new("services.suite.errors.missing").with("name", name.to_string())
      }
      other => order_error(&other),
    })?;
    Ok(plan.steps().iter().flatten().map(|name| name.as_str().into()).collect())
  }
}

/// What the page of suite `id`, or of a new one, opens with.
pub fn load_suite_in(root: &Path, id: Option<ResourceId>) -> Result<SuiteRead, Message> {
  let read = || -> Result<SuiteRead, orchester_store::StoreError> {
    let db = ProjectDb::open(root)?;
    let services = entries::<Service, _>(&db)?;
    let stacks = entries::<Stack, _>(&db)?;
    let Some(id) = id else {
      return Ok(SuiteRead {
        entry: None,
        latest: None,
        revision: None,
        services,
        stacks,
      });
    };
    let entry = entries::<TestSuite, _>(&db)?.into_iter().find(|entry| entry.id == id);
    let latest = db
      .latest_version::<TestSuite>(id)?
      .map(|version| version.body().clone());
    let revision = revision_of::<TestSuite>(&db, id)?;
    Ok(SuiteRead {
      entry,
      latest,
      revision: Some(revision),
      services,
      stacks,
    })
  };
  let read = read().map_err(|error| store_error(&error))?;
  if id.is_some() && read.entry.is_none() {
    return Err(Message::new("services.suite.errors.gone"));
  }
  Ok(read)
}

/// Saves `suite` as the draft of suite `id` while its draft is the one `expected` names, or makes a new suite named
/// `name`, unique among the project's suites (removed ones too).
pub fn save_suite_in(
  root: &Path,
  (id, name): (Option<ResourceId>, &str),
  suite: TestSuite,
  expected: Option<&str>,
) -> Result<Saved, Message> {
  let id = match id {
    Some(id) => {
      save_draft_expecting_in(root, id, suite, expected).map_err(|error| store_error(&error))?;
      id
    }
    None => {
      let slug = suite_name(name)?;
      let mut db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
      if db
        .slugs(ResourceKind::TestSuite)
        .map_err(|error| store_error(&error))?
        .contains(&slug)
      {
        return Err(Message::new("services.suite.errors.nameTaken").with("name", slug.as_str().to_owned()));
      }
      create(&mut db, slug, &suite, Timestamp::now()).map_err(|error| store_error(&error))?
    }
  };
  let db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
  let revision = revision_of::<TestSuite>(&db, id).map_err(|error| store_error(&error))?;
  Ok(Saved { id, revision })
}

/// Publishes the draft of suite `id` as its next version while it is still `expected`.
pub fn publish_suite_in(root: &Path, id: ResourceId, expected: &TestSuite) -> Result<VersionNumber, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
  db.publish_draft_matching(id, expected, Timestamp::now())
    .map(|version| version.number())
    .map_err(|error| store_error(&error))
}
