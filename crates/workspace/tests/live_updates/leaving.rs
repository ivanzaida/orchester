//! Making the project local keeps nothing the team's resources were filled in with on this computer (ORC-848 D10): the
//! picks, the switches and the secret values by name go, and the values leave the keychain before the project does.

use orchester_core::{
  id::{ConnectionId, ResourceId},
  secret::{SecretName, SecretValue, SecretVault, VaultError, VaultKey},
};
use orchester_store::{ConnectionPick, ProjectDb};
use parking_lot::Mutex;

use super::{
  ALICE,
  desktop::Desktop,
  server::TeamServer,
  support::{Fixture, at},
};

/// A keychain in memory for the value the test keeps.
#[derive(Default)]
struct Memory(Mutex<Vec<VaultKey>>);

impl SecretVault for Memory {
  fn store(&self, key: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().push(key.clone());
    Ok(())
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_make_local_removes_the_fill_ins_and_their_keychain_values() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let desktop = Desktop::signed_in(&server, ALICE, Fixture::new(), &[]);
  let (profile, mcp) = (ResourceId::generate(), ResourceId::generate());
  let token = SecretName::new("GITHUB_TOKEN").expect("name");
  let mut db = ProjectDb::open(desktop.root()).expect("the project database");
  db.set_connection_pick(
    profile,
    ConnectionPick {
      connection: ConnectionId::generate(),
      account: None,
    },
  )
  .expect("a pick");
  db.set_team_mcp_on_here(mcp, true).expect("a switch");
  let value = SecretValue::new("fake-value-7Qx2".to_owned()).expect("a value");
  db.set_team_secret_value(&Memory::default(), &token, &value)
    .expect("a value by name");
  assert!(desktop.keychain.removed.lock().is_empty());

  desktop.servers.make_local(desktop.root()).await.expect("made local");

  let db = ProjectDb::open(desktop.root()).expect("the project database");
  assert_eq!(db.connection_pick(profile).expect("read"), None);
  assert_eq!(db.team_mcp_switch(mcp).expect("read"), None);
  assert!(!db.has_team_secret_value(&token).expect("read"));
  assert_eq!(
    desktop.keychain.removed.lock().len(),
    1,
    "the secret's value left the keychain"
  );
}

/// A keychain that refuses to remove a team secret's value stops Make local before anything is released: the claims the
/// person holds stay on the server, and the project stays a team project.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_make_local_keeps_the_claims_while_the_keychain_refuses_to_remove_a_value() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut fixture = Fixture::new();
  let task = fixture.task("Status bar", at(10));
  let claimed = server.claimed(task, "Status bar", ALICE);
  let desktop = Desktop::signed_in(&server, ALICE, fixture, &[claimed]);
  let token = SecretName::new("GITHUB_TOKEN").expect("name");
  let value = SecretValue::new("fake-value-7Qx2".to_owned()).expect("a value");
  let mut db = ProjectDb::open(desktop.root()).expect("the project database");
  db.set_team_secret_value(&Memory::default(), &token, &value)
    .expect("a value by name");
  desktop.keychain.refuse_deletes();

  let refused = desktop.servers.make_local(desktop.root()).await;

  assert!(refused.is_err(), "the value could not be removed");
  assert!(server.claimed_now(task), "the claim was not released");
  let db = ProjectDb::open(desktop.root()).expect("the project database");
  assert!(
    db.sharing().expect("read").is_some(),
    "the project is still a team project"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_make_local_releases_the_claims_the_person_holds() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut fixture = Fixture::new();
  let task = fixture.task("Status bar", at(10));
  let claimed = server.claimed(task, "Status bar", ALICE);
  let desktop = Desktop::signed_in(&server, ALICE, fixture, &[claimed]);

  desktop.servers.make_local(desktop.root()).await.expect("made local");

  assert!(!server.claimed_now(task), "the claim was released");
}
