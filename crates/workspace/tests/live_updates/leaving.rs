//! Making the project local keeps nothing the team's resources were filled in with on this computer (ORC-848 D10): the
//! picks, the switches and the secret values by name go, and the values leave the keychain before the project does.

use orchester_core::{
  id::{ConnectionId, ResourceId},
  secret::{SecretName, SecretValue, SecretVault, VaultError, VaultKey},
};
use orchester_store::{ConnectionPick, ProjectDb};
use parking_lot::Mutex;

use super::{ALICE, desktop::Desktop, server::TeamServer, support::Fixture};

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
