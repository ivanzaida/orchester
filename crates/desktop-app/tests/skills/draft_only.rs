//! A skill that holds a draft and no version, as an imported one does. The editor creates a skill as its v1, so only a
//! store fixture makes this.
use std::path::Path;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Skill, Slug},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

/// Skill `name` with `body` as its draft in the project at `repo`.
pub fn create(repo: &Path, name: &str, body: &Skill) -> ResourceId {
  let slug = Slug::new(&name.to_lowercase()).expect("a slug");
  let name = Name::new(name).expect("a name");
  let mut db = ProjectDb::open(repo).expect("project database");
  db.create_resource(slug, name, body, Timestamp::now())
    .expect("a draft-only skill")
    .id()
}
