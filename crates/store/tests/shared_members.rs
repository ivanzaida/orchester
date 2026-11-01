//! The members of a shared project and its repository hint as last read (`docs/architecture/server.md`, row 63): kept
//! in the project database so Sharing shows them while the server cannot be reached; a local project has none and
//! refuses to record them, a new list replaces the old one, owners come first, who read them is kept so the list names
//! the person after this computer signed out, and Make local forgets them. A shared
//! project's database from before them opens with neither read yet.
//!
//! `cargo test -p orchester-store --test shared_members members_`

mod support;

use std::{collections::HashMap, fs};

use orchester_core::{
  id::{ProjectId, ServerId, UserId},
  project::{Project, ProjectKey},
  sharing::MemberRole,
};
use orchester_store::{ProjectDb, ServerAdmin, SharedMember, SharingLink, StoreError};
use support::{at, copied_at_version, name};
use tempfile::TempDir;

/// The schema before the members and the hint were kept (`shared_members`, the 68th). Migrations are append-only.
const BEFORE_MEMBERS: u32 = 67;

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("project");
  (repo, db)
}

fn shared(db: &mut ProjectDb) {
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Member),
    joined_at: at(100),
    cursor: 0,
    last_synced_at: at(100),
    removed_at: None,
    address: Some("https://orchester.kontur.lan/kontur".into()),
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("shared");
}

fn member(display_name: &str, username: &str, role: MemberRole, added: i64) -> SharedMember {
  SharedMember {
    user: UserId::generate(),
    display_name: display_name.to_owned(),
    username: username.to_owned(),
    role,
    added_at: at(added),
  }
}

#[test]
fn members_of_a_local_project_are_none_and_cannot_be_recorded() {
  let (_repo, mut db) = project();
  assert_eq!(db.shared_members().expect("members"), None);
  assert_eq!(db.remote_hint().expect("hint"), None);
  let members = [member("Alice Moreau", "alice", MemberRole::Owner, 10)];
  assert!(matches!(
    db.record_shared_members(&members, at(200), None),
    Err(StoreError::NotShared)
  ));
  assert!(matches!(
    db.record_remote_hint(Some("github.com/acme/kontur"), at(200)),
    Err(StoreError::NotShared)
  ));
  assert_eq!(db.shared_members().expect("members"), None, "nothing was kept");
}

#[test]
fn members_are_kept_as_last_read_with_owners_first_and_replaced_by_the_next_read() {
  let (_repo, mut db) = project();
  shared(&mut db);
  assert_eq!(db.shared_members().expect("members"), None, "not read yet");

  let maria = member("Maria Lind", "maria", MemberRole::Member, 10);
  let oleh = member("Oleh Petrenko", "oleh", MemberRole::Member, 30);
  let alice = member("Alice Moreau", "alice", MemberRole::Owner, 20);
  assert_eq!(db.members_read_as().expect("reader"), None, "not read yet");
  db.record_shared_members(&[maria.clone(), oleh.clone(), alice.clone()], at(200), Some(maria.user))
    .expect("recorded");
  assert_eq!(
    db.members_read_as().expect("reader"),
    Some(maria.user),
    "who read them, which names the person once this computer signed out"
  );
  assert_eq!(
    db.shared_members().expect("members"),
    Some((at(200), vec![alice.clone(), maria, oleh])),
    "owners first, then by when they were added"
  );

  db.record_shared_members(std::slice::from_ref(&alice), at(300), None)
    .expect("read again");
  assert_eq!(
    db.members_read_as().expect("reader"),
    None,
    "read by someone not in the list"
  );
  assert_eq!(db.shared_members().expect("members"), Some((at(300), vec![alice])));

  db.record_shared_members(&[], at(400), None).expect("an empty list");
  assert_eq!(
    db.shared_members().expect("members"),
    Some((at(400), Vec::new())),
    "read, with nobody in it"
  );
}

#[test]
fn members_name_the_servers_admins_as_last_read_and_make_local_forgets_them() {
  let (_repo, mut db) = project();
  let olha = ServerAdmin {
    user: UserId::generate(),
    display_name: "Olha".to_owned(),
    username: "olha".to_owned(),
  };
  assert!(matches!(
    db.record_server_admins(std::slice::from_ref(&olha)),
    Err(StoreError::NotShared)
  ));
  shared(&mut db);
  assert_eq!(db.server_admins().expect("admins"), [], "none named yet");
  db.record_server_admins(&[olha.clone(), olha.clone()])
    .expect("recorded");
  assert_eq!(db.server_admins().expect("admins"), std::slice::from_ref(&olha));
  db.record_server_admins(&[]).expect("replaced");
  assert_eq!(db.server_admins().expect("admins"), []);

  db.record_server_admins(std::slice::from_ref(&olha)).expect("recorded");
  db.make_local().expect("local");
  assert_eq!(db.server_admins().expect("admins"), [], "forgotten");
}

#[test]
fn members_and_the_hint_are_forgotten_by_make_local() {
  let (_repo, mut db) = project();
  shared(&mut db);
  assert_eq!(db.remote_hint().expect("hint"), None, "not read yet");
  db.record_remote_hint(None, at(150)).expect("no hint");
  assert_eq!(db.remote_hint().expect("hint"), Some((None, at(150))));
  db.record_remote_hint(Some("github.com/acme/kontur"), at(250))
    .expect("a hint");
  assert_eq!(
    db.remote_hint().expect("hint"),
    Some((Some("github.com/acme/kontur".to_owned()), at(250)))
  );
  let alice = member("Alice Moreau", "alice", MemberRole::Owner, 20);
  db.record_shared_members(std::slice::from_ref(&alice), at(200), Some(alice.user))
    .expect("recorded");

  db.make_local().expect("local");
  assert_eq!(db.shared_members().expect("members"), None);
  assert_eq!(db.members_read_as().expect("reader"), None);
  assert_eq!(db.remote_hint().expect("hint"), None);
  assert!(matches!(
    db.record_shared_members(&[], at(500), None),
    Err(StoreError::NotShared)
  ));
}

#[test]
fn members_of_a_database_from_before_them_are_not_read_yet() {
  let (repo, mut db) = project();
  shared(&mut db);
  drop(db);
  let old = copied_at_version(&repo.path().join(".orchester").join("orchester.db"), BEFORE_MEMBERS);

  let mut db = ProjectDb::open(old.path()).expect("migrated");
  assert!(db.sharing().expect("link").is_some(), "the link is kept");
  assert_eq!(db.shared_members().expect("members"), None);
  assert_eq!(db.remote_hint().expect("hint"), None);
  assert_eq!(db.members_read_as().expect("reader"), None);
  let alice = member("Alice Moreau", "alice", MemberRole::Owner, 20);
  db.record_shared_members(std::slice::from_ref(&alice), at(200), Some(alice.user))
    .expect("recorded");
  assert_eq!(db.members_read_as().expect("reader"), Some(alice.user));
  assert_eq!(db.shared_members().expect("members"), Some((at(200), vec![alice])));
}
