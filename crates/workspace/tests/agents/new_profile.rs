//! A new profile approves automatically (ORC-619): the editor's actions take that default without asking, keep it
//! turned off once the person turns it off, and still ask when a profile that has it off turns it on.
use orchester_core::{connection::invocation::Platform, resource::AgentProfile, store::ResourceStore};
use orchester_workspace::{
  agents::{
    data::ProfileEntry,
    editor::{self, Action, Outcome, Request},
  },
  messages::Message,
};

use super::*;

fn with_auto(profile: &AgentProfile, on: bool) -> AgentProfile {
  let mut permissions = profile.permissions().clone();
  permissions.set_auto_approve(on);
  profile.clone().with_permissions(permissions)
}

fn entry(db: &ProjectDb, id: orchester_core::id::ResourceId) -> ProfileEntry {
  ProjectAgents::load(db)
    .expect("load")
    .profile(id)
    .expect("profile")
    .clone()
}

fn request(existing: Option<ProfileEntry>, profile: AgentProfile, action: Action) -> Request {
  Request {
    existing,
    profile,
    action,
    allowed: vec![],
    prompt_decided: true,
  }
}

/// A profile made as the editor makes a new one on `connection`.
fn new_profile(connection: &AgentConnection) -> AgentProfile {
  let permissions = AgentPermissions::for_new_profile(connection.provider(), Platform::current());
  assert!(permissions.auto_approve(), "a new profile approves automatically");
  let selection = ModelSelection::Listed(model("gpt-5.5"));
  ProfileForm {
    permissions: &permissions,
    ..form(connection, &selection, None)
  }
  .profile()
  .expect("profile")
}

#[test]
fn new_profile_with_the_default_is_saved_and_published_without_asking() {
  let (repo, mut db) = project_db();
  let connection = codex(&repo);
  db.create_connection(&connection).expect("connection");
  let profile = new_profile(&connection);
  let Outcome::Saved(id) = editor::apply(repo.path(), &request(None, profile.clone(), Action::Save)).expect("saved")
  else {
    panic!("the default widens nothing")
  };
  let publish = request(Some(entry(&db, id)), profile, Action::Publish);
  assert_eq!(
    editor::apply(repo.path(), &publish).expect("published"),
    Outcome::Saved(id)
  );
  let v1 = db.latest_version::<AgentProfile>(id).expect("read").expect("v1");
  assert!(v1.body().permissions().auto_approve(), "v1 approves automatically");
}

#[test]
fn new_profile_turned_off_keeps_it_off_and_turning_it_on_again_asks() {
  let (repo, mut db) = project_db();
  let connection = codex(&repo);
  db.create_connection(&connection).expect("connection");
  let off = with_auto(&new_profile(&connection), false);
  let Outcome::Saved(id) = editor::apply(repo.path(), &request(None, off.clone(), Action::Save)).expect("saved") else {
    panic!("turning it off narrows")
  };
  assert!(
    !entry(&db, id).profile.permissions().auto_approve(),
    "the draft keeps it off"
  );
  let publish = request(Some(entry(&db, id)), off.clone(), Action::Publish);
  assert_eq!(
    editor::apply(repo.path(), &publish).expect("published"),
    Outcome::Saved(id)
  );
  let v1 = db.latest_version::<AgentProfile>(id).expect("read").expect("v1");
  assert!(!v1.body().permissions().auto_approve(), "v1 keeps it off");
  let on = request(Some(entry(&db, id)), with_auto(&off, true), Action::Save);
  assert_eq!(
    editor::apply(repo.path(), &on).expect("offered"),
    Outcome::Confirm(vec![Message::new("appMcp.widens.autoApprove")]),
    "turning it on where the published version has it off widens"
  );
  assert!(
    !entry(&db, id).profile.permissions().auto_approve(),
    "nothing saved before the person confirms"
  );
}

#[test]
fn new_profile_default_leaves_existing_profiles_as_they_were() {
  let (repo, mut db) = project_db();
  let connection = codex(&repo);
  db.create_connection(&connection).expect("connection");
  let before = form(&connection, &ModelSelection::Listed(model("gpt-5.5")), None)
    .profile()
    .expect("profile");
  assert!(!before.permissions().auto_approve());
  let id = create_profile(&mut db, &before, now()).expect("create");
  db.publish_draft::<AgentProfile>(id, now()).expect("v1");
  assert!(
    !entry(&db, id).profile.permissions().auto_approve(),
    "an existing profile keeps it off"
  );
  let on = request(Some(entry(&db, id)), with_auto(&before, true), Action::Save);
  assert_eq!(
    editor::apply(repo.path(), &on).expect("offered"),
    Outcome::Confirm(vec![Message::new("appMcp.widens.autoApprove")])
  );
}
