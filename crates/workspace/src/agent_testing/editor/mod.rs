//! The service editor (`02 · Service editor` `TGRTt`) as the window reads and saves it: a service's draft or latest
//! version with the project's other services, its fields ([`form`]), what follows from them ([`order`], [`program`]),
//! and saving its draft, publishing it and deleting it through the project database, as `save_service_draft` and
//! `publish_service` do. Every function here blocks on SQLite; the UI calls them off the UI thread.

pub mod form;
pub mod order;
pub mod program;

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{Removal, RemoveMode, ResourceKind, VersionNumber},
  service::Service,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::data::{Entry, create, entries, service_slug};
use crate::{
  drafts::{revision_of, save_draft_expecting_in},
  messages::{Message, store_error},
  removal,
};

/// A service as its page opens it, with the project's services (it among them), each its draft if it has one,
/// otherwise its latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorRead {
  /// The service; `None` for a new one.
  pub entry: Option<Entry<Service>>,
  /// The latest version, which publishing compares the draft with.
  pub latest: Option<Service>,
  /// What its draft is saved on ([`crate::drafts::draft_revision`]); a save made on it goes over no other change.
  pub revision: Option<String>,
  pub services: Vec<Entry<Service>>,
}

/// What the page of service `id`, or of a new one, opens with.
pub fn load_in(root: &Path, id: Option<ResourceId>) -> Result<EditorRead, Message> {
  let read = || -> Result<EditorRead, orchester_store::StoreError> {
    let db = ProjectDb::open(root)?;
    let services = entries::<Service, _>(&db)?;
    let Some(id) = id else {
      return Ok(EditorRead {
        entry: None,
        latest: None,
        revision: None,
        services,
      });
    };
    let entry = services.iter().find(|entry| entry.id == id).cloned();
    let latest = db.latest_version::<Service>(id)?.map(|version| version.body().clone());
    let revision = revision_of::<Service>(&db, id)?;
    Ok(EditorRead {
      entry,
      latest,
      revision: Some(revision),
      services,
    })
  };
  let read = read().map_err(|error| store_error(&error))?;
  if id.is_some() && read.entry.is_none() {
    return Err(Message::new("services.editor.errors.gone"));
  }
  Ok(read)
}

/// What a save stored: the service's id and the revision the next save is made on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
  pub id: ResourceId,
  pub revision: String,
}

/// Saves `service` as the draft of service `id` while its draft is the one `expected` names, or makes a new service
/// under its name, unique among the project's services (removed ones too).
pub fn save_in(
  root: &Path,
  id: Option<ResourceId>,
  service: Service,
  expected: Option<&str>,
) -> Result<Saved, Message> {
  let id = match id {
    Some(id) => {
      save_draft_expecting_in(root, id, service, expected).map_err(|error| store_error(&error))?;
      id
    }
    None => {
      let mut db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
      let slug = service_slug(service.name());
      if db
        .slugs(ResourceKind::Service)
        .map_err(|error| store_error(&error))?
        .contains(&slug)
      {
        return Err(Message::new("services.editor.errors.nameTaken").with("name", slug.as_str().to_owned()));
      }
      create(&mut db, slug, &service, Timestamp::now()).map_err(|error| store_error(&error))?
    }
  };
  let db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
  let revision = revision_of::<Service>(&db, id).map_err(|error| store_error(&error))?;
  Ok(Saved { id, revision })
}

/// Publishes the draft of service `id` as its next version while it is still `expected`; refused while a service it
/// needs is not published or the needs form a cycle.
pub fn publish_in(root: &Path, id: ResourceId, expected: &Service) -> Result<VersionNumber, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
  db.publish_draft_matching(id, expected, Timestamp::now())
    .map(|version| version.number())
    .map_err(|error| store_error(&error))
}

/// Deletes service `id`; its versions stay for the runs that started it. No pipeline step names a service, so the
/// store finds none using it; if one ever did, nothing is deleted and that is said.
pub fn delete_in(root: &Path, id: ResourceId) -> Result<(), Message> {
  match removal::remove_in(root, id, RemoveMode::Unused)? {
    Removal::Removed => Ok(()),
    Removal::InUse(_) => Err(Message::new("services.editor.errors.inUse")),
  }
}
