use orchester_core::import::{Entity, Pick};

use super::fixture::{source, version};

#[test]
fn import_catalog_lists_what_refers_to_a_resource() {
  let source = source();

  let users = |entity| -> Vec<&str> {
    source
      .catalog
      .users(entity)
      .map(|resource| resource.resource().name().as_str())
      .collect()
  };

  assert_eq!(users(Entity::Resource(source.builder)), ["Preview release", "Nightly"]);
  assert_eq!(
    users(Entity::Connection(source.codex)),
    ["Builder", "Reviewer"],
    "profiles refer to their connection"
  );
  assert_eq!(users(Entity::Resource(source.pipeline)), ["Release rules"]);
  assert!(users(Entity::Resource(source.rules)).is_empty());
}

#[test]
fn import_catalog_offers_the_published_version_first() {
  let source = source();
  let preview = source.catalog.resource(source.pipeline).expect("Preview release");

  assert_eq!(preview.default_pick(), Pick::Published(version(3)));
  assert_eq!(preview.picks(), [Pick::Published(version(3)), Pick::Draft]);
  assert!(
    preview.content(Pick::Published(version(2))).is_none(),
    "only the latest version is offered"
  );
}
