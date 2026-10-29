//! Reading and making services and stacks through the project database. Each follows the shared lifecycle: Draft
//! saved → Publish vN → Published vN. A service or stack is named by its slug, which is the name its body holds, so
//! needs and stacks name what they start. Every function blocks on SQLite.

use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, ResourceBody, Slug, Version},
  service::{ServiceName, StackName},
  store::ResourceStore,
  time::Timestamp,
};

use crate::agents::data::VersionState;

/// A service or stack as automation reads it: the draft if there is one, otherwise the latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry<B> {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub body: B,
  pub state: VersionState,
}

impl<B: Clone> Entry<B> {
  /// `None` for a resource with neither a draft nor a version, which the store never creates.
  pub fn new(id: ResourceId, slug: &Slug, draft: Option<Draft<B>>, latest: Option<&Version<B>>) -> Option<Self> {
    let published = latest.map(Version::number);
    let (body, state) = match (draft, latest) {
      (Some(draft), _) => {
        let state = if draft.based_on() == published {
          VersionState::Draft { published }
        } else {
          VersionState::StaleDraft {
            based_on: draft.based_on(),
            published,
          }
        };
        (draft.body().clone(), state)
      }
      (None, Some(latest)) => (latest.body().clone(), VersionState::Published(latest.number())),
      (None, None) => return None,
    };
    Some(Self {
      id,
      name: slug.as_str().into(),
      body,
      state,
    })
  }
}

/// The project's resources of `B`'s kind that are not removed, ordered by name.
pub fn entries<B: ResourceBody + Clone, S: ResourceStore>(store: &S) -> Result<Vec<Entry<B>>, S::Error> {
  let mut entries = Vec::new();
  for resource in store.resources(B::KIND)? {
    let draft = store.draft::<B>(resource.id())?;
    let latest = store.latest_version::<B>(resource.id())?;
    entries.extend(Entry::new(resource.id(), resource.slug(), draft, latest.as_ref()));
  }
  Ok(entries)
}

/// The slug a service is stored under: its name.
pub fn service_slug(name: &ServiceName) -> Slug {
  Slug::new(name.as_str()).expect("a service name is a slug")
}

/// The slug a stack is stored under: its name.
pub fn stack_slug(name: &StackName) -> Slug {
  Slug::new(name.as_str()).expect("a stack name is a slug")
}

/// Makes a resource whose first draft is `body`, named and stored under `slug`; the store refuses a slug a resource of
/// its kind holds, removed ones too.
pub fn create<B: ResourceBody, S: ResourceStore>(
  store: &mut S,
  slug: Slug,
  body: &B,
  now: Timestamp,
) -> Result<ResourceId, S::Error> {
  let name = Name::new(slug.as_str()).expect("a slug is a valid name");
  Ok(store.create_resource(slug, name, body, now)?.id())
}
