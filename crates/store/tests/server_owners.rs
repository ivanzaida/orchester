//! Whose each team server is (the person's decisions, 2026-10-16 and 2026-10-30: servers per owner, as projects are):
//! an account's is listed for that account only, this computer's beside it, an address once per owner, and those known
//! before servers had owners are the account's signed in then, with all they kept.
//!
//! `cargo test -p orchester-store --test server_owners server_owners_`

use orchester_core::{
  id::{CloudAccountId, ServerId, TeamServerId, UserId},
  time::Timestamp,
};
use orchester_store::{AppDb, JoinedWith, ServerAccount, StoredServer};

/// The schema version before servers had owners: the migrations up to `server_owners`.
const BEFORE_SERVER_OWNERS: u32 = 16;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn server(url: &str, added_at: i64) -> StoredServer {
  StoredServer {
    id: ServerId::generate(),
    url: url.to_owned(),
    name: "Kontur team".to_owned(),
    plain_http_allowed: false,
    certificate_pin: None,
    added_at: at(added_at),
    joined_with: None,
    account: None,
  }
}

fn ids(db: &AppDb, owner: Option<&CloudAccountId>) -> Vec<ServerId> {
  db.servers_of(owner)
    .expect("list")
    .iter()
    .map(|server| server.id)
    .collect()
}

#[test]
fn server_owners_list_only_their_own_and_an_address_once_each() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let (bohdan, olena) = (CloudAccountId::generate(), CloudAccountId::generate());
  let kontur = server("https://orchester.kontur.lan", 1);
  let olenas_kontur = server("https://orchester.kontur.lan", 2);
  let lab = server("http://10.0.4.20:8740", 3);
  db.add_server(&kontur, Some(&bohdan)).expect("Bohdan's");
  db.add_server(&olenas_kontur, Some(&olena))
    .expect("the same address is Olena's own entry");
  db.add_server(&lab, None).expect("nobody's yet");
  assert_eq!(ids(&db, Some(&bohdan)), [kontur.id]);
  assert_eq!(ids(&db, Some(&olena)), [olenas_kontur.id]);
  assert_eq!(ids(&db, None), [lab.id]);
  assert_eq!(db.every_server().expect("all").len(), 3);
  let owners = db.server_owners().expect("owners");
  assert_eq!(owners.len(), 3);
  assert!(owners.contains(&None) && owners.contains(&Some(bohdan)) && owners.contains(&Some(olena)));

  assert!(
    db.add_server(&server("https://orchester.kontur.lan", 4), Some(&bohdan))
      .is_err(),
    "an address once per account"
  );
  assert!(
    db.add_server(&server("http://10.0.4.20:8740", 4), None).is_err(),
    "and once among the servers nobody took"
  );
}

#[test]
fn server_owners_this_computers_servers_show_beside_an_accounts_own() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let (bohdan, olena) = (CloudAccountId::generate(), CloudAccountId::generate());
  let lab = server("http://10.0.4.20:8740", 1);
  let bohdans_lab = server("http://10.0.4.20:8740", 2);
  let olenas = server("https://orchester.olena.lan", 3);
  db.add_server(&lab, None).expect("this computer's");
  db.add_server(&bohdans_lab, Some(&bohdan))
    .expect("Bohdan's, at the same address");
  db.add_server(&olenas, Some(&olena)).expect("Olena's");
  let visible = |db: &AppDb, owner: Option<&CloudAccountId>| -> Vec<ServerId> {
    db.servers_visible_to(owner)
      .expect("list")
      .iter()
      .map(|server| server.id)
      .collect()
  };
  assert_eq!(visible(&db, Some(&bohdan)), [lab.id, bohdans_lab.id]);
  assert_eq!(visible(&db, Some(&olena)), [lab.id, olenas.id]);
  assert_eq!(visible(&db, None), [lab.id], "signed out: only this computer's");
  assert_eq!(db.server_owner(lab.id).expect("whose"), Some(None));
  assert_eq!(db.server_owner(olenas.id).expect("whose"), Some(Some(olena)));
  assert_eq!(db.server_owner(ServerId::generate()).expect("whose"), None);
}

#[test]
fn server_owners_a_signed_in_account_uses_this_computers_server_and_its_own_only() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let lab = server("http://10.0.4.20:8740", 1);
  let (kontur, kontur_owner) = (server("https://orchester.kontur.lan", 2), CloudAccountId::generate());
  db.add_server(&lab, None).expect("this computer's");
  db.add_server(&kontur, Some(&kontur_owner)).expect("an account's");
  assert!(
    db.is_signed_in_accounts_server(lab.id).expect("asked"),
    "with nobody signed in too"
  );
  assert!(!db.is_signed_in_accounts_server(kontur.id).expect("asked"));
  assert!(db.sign_out_of_server(lab.id).expect("signed out"));
  assert!(!db.sign_out_of_server(kontur.id).expect("asked"), "an account's stays");
}

/// A server row as the schema before owners kept it: joined with the cloud, pinned, signed in.
fn insert_old_server(raw: &rusqlite::Connection, id: ServerId, team_server: TeamServerId, user: UserId) {
  raw
    .execute(
      "INSERT INTO servers (id, url, name, plain_http_allowed, added_at, user_id, username, display_name, admin, \
       signed_in_at, certificate_pin, joined_with, team_server) \
       VALUES (?1, 'https://orchester.kontur.lan', 'Kontur team', 0, 1, ?2, 'bohdan', 'Bohdan Koval', 1, 2, ?3, \
       'cloud', ?4)",
      rusqlite::params![
        id.to_string(),
        user.to_string(),
        "4b".repeat(32),
        team_server.to_string()
      ],
    )
    .expect("a server");
}

#[test]
fn server_owners_of_servers_known_before_are_the_account_signed_in_then() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let (id, team_server, user) = (ServerId::generate(), TeamServerId::generate(), UserId::generate());

  let path = dir.path().join("orchester.db");
  let raw = AppDb::create_at_version(&path, BEFORE_SERVER_OWNERS).expect("the schema before server owners");
  insert_old_server(&raw, id, team_server, user);
  drop(raw);
  let mut db = AppDb::open(&path).expect("migrate");
  assert_eq!(ids(&db, None), [id], "no account was signed in: nobody's yet");
  let kept = db.servers_of(None).expect("list").remove(0);
  assert_eq!(kept.certificate_pin, Some("4b".repeat(32)));
  assert_eq!(kept.joined_with, Some(JoinedWith::Cloud(team_server)));
  assert_eq!(
    kept.account,
    Some(ServerAccount {
      user,
      username: "bohdan".into(),
      display_name: "Bohdan Koval".into(),
      admin: true,
      signed_in_at: at(2),
    })
  );
  let olena = CloudAccountId::generate();
  assert!(
    db.add_server(&server("https://orchester.kontur.lan", 3), Some(&olena))
      .is_ok(),
    "after the migration an address is unique per account"
  );

  let signed_in = dir.path().join("signed-in.db");
  let raw = AppDb::create_at_version(&signed_in, BEFORE_SERVER_OWNERS).expect("the schema before server owners");
  let bohdan = CloudAccountId::generate();
  raw
    .execute(
      "INSERT INTO cloud_account (id, account_id, email, display_name, device_name, signed_in_at, cloud_address) \
       VALUES (1, ?1, 'bohdan@example.com', 'Bohdan Koval', 'Laptop', 1, 'https://cloud.test')",
      [bohdan.to_string()],
    )
    .expect("signed in");
  insert_old_server(&raw, id, team_server, user);
  drop(raw);
  let db = AppDb::open(&signed_in).expect("migrate");
  assert_eq!(ids(&db, Some(&bohdan)), [id], "the account signed in then takes it");
  assert!(ids(&db, None).is_empty());
  assert_eq!(
    db.cloud_account().expect("account").map(|account| account.account),
    Some(bohdan),
    "still signed in"
  );
}
