//! Whose each known project is (the person's decisions, 2026-10-16 and 2026-10-30): an account's is listed,
//! re-registered and forgotten for that account only; this computer's shows to whoever is signed in and no sign-in
//! claims it; a person moves one they see; and those known before projects had owners are the account's signed in then.
//!
//! `cargo test -p orchester-store --test project_owners owners_`

use std::path::Path;

use orchester_core::{
  id::{CloudAccountId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  time::Timestamp,
};
use orchester_store::{AppDb, StoreError};

fn project(name: &str) -> Project {
  Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new(name).expect("valid name"),
  )
}

/// The schema version before projects had owners: the migrations up to `owners`.
const BEFORE_OWNERS: u32 = 15;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn names(db: &AppDb, owner: Option<&CloudAccountId>) -> Vec<String> {
  db.projects_of(owner)
    .expect("list")
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect()
}

fn visible(db: &AppDb, owner: Option<&CloudAccountId>) -> Vec<String> {
  db.projects_visible_to(owner)
    .expect("list")
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect()
}

#[test]
fn owners_list_and_change_only_what_they_see() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let (bohdan, olena) = (CloudAccountId::generate(), CloudAccountId::generate());
  let (kontur, ledger, notes) = (project("Kontur"), project("Ledger"), project("Notes"));
  db.register_project(&kontur, Path::new("H:/projects/kontur"), at(1), Some(&bohdan))
    .expect("Bohdan's");
  db.register_project(&ledger, Path::new("H:/projects/ledger"), at(2), Some(&olena))
    .expect("Olena's");
  db.register_project(&notes, Path::new("H:/projects/notes"), at(3), None)
    .expect("this computer's");
  assert_eq!(names(&db, Some(&bohdan)), ["Kontur"]);
  assert_eq!(names(&db, Some(&olena)), ["Ledger"]);
  assert_eq!(names(&db, None), ["Notes"]);
  assert_eq!(visible(&db, Some(&bohdan)), ["Notes", "Kontur"]);
  assert_eq!(visible(&db, Some(&olena)), ["Notes", "Ledger"]);
  assert_eq!(visible(&db, None), ["Notes"], "signed out: only this computer's");
  assert_eq!(db.every_project().expect("all").len(), 3);
  let waiting: Vec<_> = db
    .projects_of_accounts()
    .expect("what waits")
    .into_iter()
    .map(|project| (project.owner, project.name.as_str().to_owned()))
    .collect();
  assert_eq!(
    waiting,
    [(Some(olena), "Ledger".to_owned()), (Some(bohdan), "Kontur".to_owned())]
  );

  let refused = db.register_project(&kontur, Path::new("H:/projects/kontur"), at(9), Some(&olena));
  assert!(
    matches!(refused, Err(StoreError::ProjectOfAnotherAccount(id)) if id == kontur.id()),
    "{refused:?}"
  );
  let refused = db.register_project(&kontur, Path::new("H:/projects/kontur"), at(9), None);
  assert!(
    matches!(refused, Err(StoreError::ProjectOfSignedOutAccount(id)) if id == kontur.id()),
    "{refused:?}"
  );
  db.forget_project(kontur.id(), Some(&olena)).expect("nothing to forget");
  let kept = db.projects_of(Some(&bohdan)).expect("list");
  assert_eq!(kept.len(), 1, "Olena neither took nor forgot Bohdan's project");
  assert_eq!(kept[0].last_opened_at, at(1));

  db.forget_project(ledger.id(), Some(&olena)).expect("forget");
  assert_eq!(names(&db, Some(&olena)), Vec::<String>::new());
  db.forget_project(notes.id(), Some(&olena)).expect("forget");
  assert_eq!(
    names(&db, None),
    Vec::<String>::new(),
    "an account forgets this computer's too"
  );
}

#[test]
fn owners_opening_a_project_of_this_computers_claims_nothing() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let bohdan = CloudAccountId::generate();
  let notes = project("Notes");
  db.register_project(&notes, Path::new("H:/projects/notes"), at(1), None)
    .expect("this computer's");
  db.register_project(&notes, Path::new("H:/projects/notes"), at(2), Some(&bohdan))
    .expect("opened with an account signed in");
  assert_eq!(names(&db, None), ["Notes"]);
  assert_eq!(db.project_owner(notes.id()).expect("whose"), Some(None));
  assert_eq!(
    db.projects_of(None).expect("list")[0].last_opened_at,
    at(2),
    "its last use is kept"
  );
}

#[test]
fn owners_a_person_moves_a_project_they_see() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let (bohdan, olena) = (CloudAccountId::generate(), CloudAccountId::generate());
  let (notes, ledger) = (project("Notes"), project("Ledger"));
  db.register_project(&notes, Path::new("H:/projects/notes"), at(1), None)
    .expect("this computer's");
  db.register_project(&ledger, Path::new("H:/projects/ledger"), at(2), Some(&olena))
    .expect("Olena's");

  assert!(db.set_project_owner(notes.id(), Some(&bohdan), &bohdan).expect("moved"));
  assert_eq!(names(&db, Some(&bohdan)), ["Notes"]);
  assert_eq!(names(&db, None), Vec::<String>::new());
  assert!(db.set_project_owner(notes.id(), None, &bohdan).expect("moved back"));
  assert_eq!(names(&db, None), ["Notes"]);

  assert!(
    !db
      .set_project_owner(ledger.id(), Some(&bohdan), &bohdan)
      .expect("asked"),
    "another account's project does not move"
  );
  assert!(
    !db.set_project_owner(ledger.id(), None, &bohdan).expect("asked"),
    "nor to this computer"
  );
  assert_eq!(names(&db, Some(&olena)), ["Ledger"]);
}

/// An install from before this change, signed in: every project keeps its account, and its list does not change.
#[test]
fn owners_an_existing_signed_in_install_keeps_every_project() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let raw = AppDb::create_at_version(&path, BEFORE_OWNERS).expect("the schema before owners");
  let bohdan = CloudAccountId::generate();
  raw
    .execute(
      "INSERT INTO cloud_account (id, account_id, email, display_name, device_name, signed_in_at, cloud_address) \
       VALUES (1, ?1, 'bohdan@example.com', 'Bohdan Koval', 'Laptop', 1, 'https://cloud.test')",
      [bohdan.to_string()],
    )
    .expect("signed in");
  for (name, root) in [("Kontur", "H:/k"), ("Ledger", "H:/l")] {
    raw
      .execute(
        "INSERT INTO projects (id, name, repo_root, added_at, last_opened_at) VALUES (?1, ?2, ?3, 1, 1)",
        [ProjectId::generate().to_string(), name.to_owned(), root.to_owned()],
      )
      .expect("a project");
  }
  drop(raw);
  let db = AppDb::open(&path).expect("migrate");
  let mut shown = visible(&db, Some(&bohdan));
  shown.sort();
  assert_eq!(shown, ["Kontur", "Ledger"], "signed in, it sees all it had");
  assert_eq!(visible(&db, None), Vec::<String>::new(), "they wait for it");
  assert_eq!(db.projects_of_accounts().expect("what waits").len(), 2);
}

#[test]
fn owners_of_projects_known_before_are_the_account_signed_in_then() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let raw = AppDb::create_at_version(&path, BEFORE_OWNERS).expect("the schema before owners");
  raw
    .execute(
      "INSERT INTO projects (id, name, repo_root, added_at, last_opened_at) VALUES (?1, 'Kontur', 'H:/k', 1, 1)",
      [ProjectId::generate().to_string()],
    )
    .expect("a project");
  drop(raw);
  let db = AppDb::open(&path).expect("migrate");
  assert_eq!(names(&db, None), ["Kontur"], "no account was signed in: nobody's yet");
  drop(db);

  // The schema before owners, with an account signed in then and a project from then. Every migration commits with
  // its version, so a database at that version has exactly that schema.
  let signed_in = dir.path().join("signed-in.db");
  let raw = AppDb::create_at_version(&signed_in, BEFORE_OWNERS).expect("the schema before owners");
  let bohdan = CloudAccountId::generate();
  raw
    .execute(
      "INSERT INTO cloud_account (id, account_id, email, display_name, device_name, signed_in_at, cloud_address) \
       VALUES (1, ?1, 'bohdan@example.com', 'Bohdan Koval', 'Laptop', 1, 'https://cloud.test')",
      [bohdan.to_string()],
    )
    .expect("signed in");
  raw
    .execute(
      "INSERT INTO projects (id, name, repo_root, added_at, last_opened_at) VALUES (?1, 'Ledger', 'H:/l', 1, 1)",
      [ProjectId::generate().to_string()],
    )
    .expect("a project");
  drop(raw);
  let db = AppDb::open(&signed_in).expect("migrate");
  assert_eq!(
    names(&db, Some(&bohdan)),
    ["Ledger"],
    "the account signed in then takes it"
  );
  assert_eq!(names(&db, None), Vec::<String>::new());
}
