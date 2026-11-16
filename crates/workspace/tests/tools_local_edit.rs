//! A local tool's panel in the Agents hub publishes what it holds as the next version at once, asks first for what
//! widens what agents may run, and stores nothing it refuses.
mod support;
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Tool, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::{
  connections::mcp::Allowed,
  tools::{
    editor::{self, Edit},
    form::CommandText,
    local_edit::{SaveError, Saved, save_and_publish_in},
  },
};
use support::Fixture;

fn fields() -> CommandText {
  CommandText {
    program: std::env::current_exe()
      .expect("test binary")
      .to_string_lossy()
      .into_owned(),
    ..CommandText::new_tool()
  }
}

fn tool(text: &CommandText) -> Tool {
  text.tool(None).expect("command")
}

/// A tool published as v1, and the body it was published with.
fn create(f: &Fixture) -> (ResourceId, Tool) {
  let id = editor::apply_in(
    f.repo.path(),
    Edit::Save {
      name: Name::new("Checks").expect("name"),
      tool: Box::new(tool(&fields())),
      before: None,
    },
    &Allowed::Anything,
  )
  .expect("created");
  let published = f
    .db
    .latest_version::<Tool>(id)
    .expect("read")
    .expect("v1")
    .body()
    .clone();
  (id, published)
}

fn latest(f: &Fixture, id: ResourceId) -> (VersionNumber, Tool) {
  let version = f.db.latest_version::<Tool>(id).expect("read").expect("a version");
  (version.number(), version.body().clone())
}

#[test]
fn tools_local_edit_publishes_a_harmless_change_as_the_next_version() {
  let f = Fixture::new();
  let (id, base) = create(&f);
  let mut text = fields();
  text.timeout = "60".into();
  let saved = save_and_publish_in(f.repo.path(), id, (&base, &tool(&text)), &[]).expect("saved");
  let Saved::Published(published) = saved else {
    panic!("a harmless change asks nothing: {saved:?}")
  };
  let (number, body) = latest(&f, id);
  assert_eq!(published, body);
  assert_eq!(number.get(), 2);
  assert_eq!(CommandText::of(&body).expect("command").timeout, "60");
  assert!(f.db.draft::<Tool>(id).expect("read").is_none());
}

#[test]
fn tools_local_edit_asks_before_widening_and_publishes_once_allowed() {
  let f = Fixture::new();
  let (id, base) = create(&f);
  let mut text = fields();
  text.arguments = "--other".into();
  let changed = tool(&text);
  let Saved::Confirm(reasons) = save_and_publish_in(f.repo.path(), id, (&base, &changed), &[]).expect("asked") else {
    panic!("new arguments widen what agents may run")
  };
  assert!(!reasons.is_empty());
  assert_eq!(latest(&f, id).0.get(), 1);
  assert!(f.db.draft::<Tool>(id).expect("read").is_none());
  let saved = save_and_publish_in(f.repo.path(), id, (&base, &changed), &reasons).expect("saved");
  assert!(matches!(saved, Saved::Published(_)));
  let (number, body) = latest(&f, id);
  assert_eq!(number.get(), 2);
  assert_eq!(CommandText::of(&body).expect("command").arguments, "--other");
}

#[test]
fn tools_local_edit_refuses_a_base_that_is_no_longer_the_latest() {
  let f = Fixture::new();
  let (id, base) = create(&f);
  let mut text = fields();
  text.timeout = "60".into();
  save_and_publish_in(f.repo.path(), id, (&base, &tool(&text)), &[]).expect("v2");
  text.timeout = "90".into();
  let refused = save_and_publish_in(f.repo.path(), id, (&base, &tool(&text)), &[]);
  assert!(matches!(refused, Err(SaveError::Changed)), "{refused:?}");
  assert_eq!(latest(&f, id).0.get(), 2);
}

#[test]
fn tools_local_edit_replaces_a_draft_stored_meanwhile() {
  let mut f = Fixture::new();
  let (id, base) = create(&f);
  let mut drafted = fields();
  drafted.timeout = "45".into();
  f.db
    .save_draft(
      id,
      &Draft::new(tool(&drafted), Some(VersionNumber::FIRST), Timestamp::now()),
    )
    .expect("an MCP client's draft");
  let mut text = fields();
  text.timeout = "60".into();
  let saved = save_and_publish_in(f.repo.path(), id, (&base, &tool(&text)), &[]).expect("saved");
  assert!(matches!(saved, Saved::Published(_)));
  let (number, body) = latest(&f, id);
  assert_eq!(number.get(), 2);
  assert_eq!(CommandText::of(&body).expect("command").timeout, "60");
  assert!(f.db.draft::<Tool>(id).expect("read").is_none());
}

#[test]
fn tools_local_edit_stores_nothing_for_a_program_it_cannot_fix() {
  let f = Fixture::new();
  let (id, base) = create(&f);
  let mut text = fields();
  text.program = "scripts/program".into();
  let refused = save_and_publish_in(f.repo.path(), id, (&base, &tool(&text)), &[]);
  assert!(matches!(refused, Err(SaveError::Refused(_))), "{refused:?}");
  assert_eq!(latest(&f, id), (VersionNumber::FIRST, base));
  assert!(f.db.draft::<Tool>(id).expect("read").is_none());
}

#[test]
fn tools_local_edit_says_nothing_changed_for_the_published_body() {
  let f = Fixture::new();
  let (id, base) = create(&f);
  let saved = save_and_publish_in(f.repo.path(), id, (&base, &base), &[]).expect("saved");
  assert!(matches!(saved, Saved::Published(_)));
  assert_eq!(latest(&f, id).0, VersionNumber::FIRST);
}
