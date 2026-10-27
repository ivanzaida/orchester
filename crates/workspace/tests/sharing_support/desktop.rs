//! A desktop's cloud and servers on a temporary database with a keychain in memory, and a team server linked to a
//! stand-in cloud: shared by the tests of this computer's Orchester Cloud account.
#![allow(dead_code, reason = "each test binary uses part of it")]

use std::sync::Arc;

use orchester_api::{
  Credential,
  client::{CloudAddress, ServerAddress},
};
use orchester_core::{id::ServerId, secret::VaultKey};
use orchester_store::{AppDb, StoredCloudAddress};
use orchester_workspace::{
  secrets::Vault,
  sharing::{Servers, cloud::Cloud},
  workspace::Workspace,
};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::{
  account_cloud::{AccountCloud, EMAIL, PASSWORD},
  server_support::TestServer,
  support::MemoryVault,
};

/// A desktop with a keychain in memory, using the cloud at `cloud`.
pub struct Desktop {
  pub workspace: Workspace,
  pub vault: Arc<MemoryVault>,
  pub servers: Servers,
  pub cloud: Cloud,
  pub runtime: Runtime,
  _data: TempDir,
}

impl Desktop {
  /// A desktop using the cloud at the web address `cloud`, its API on that address too (`/api/v1`).
  pub fn new(cloud: &str) -> Self {
    Self::at(CloudAddress::new(
      ServerAddress::parse(cloud).expect("cloud address"),
      None,
    ))
  }

  /// A desktop using the cloud at `address`.
  pub fn at(address: CloudAddress) -> Self {
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    let vault = Arc::new(MemoryVault::default());
    let servers = Servers::new(workspace.clone(), Vault::with(vault.clone()));
    let desktop = Self {
      cloud: Cloud::new(workspace.clone(), Vault::with(vault.clone()), servers.clone()),
      workspace,
      vault,
      servers,
      runtime: Runtime::new().expect("runtime"),
      _data: data,
    };
    desktop
      .block_on(desktop.cloud.set_address(Some(address)))
      .expect("test cloud");
    desktop
  }

  pub fn block_on<T>(&self, work: impl Future<Output = T>) -> T {
    self.runtime.block_on(work)
  }

  pub fn signed_in(cloud: &AccountCloud) -> Self {
    let desktop = Self::new(&cloud.url());
    desktop
      .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
      .expect("sign in");
    desktop
  }

  pub fn device_key(&self) -> VaultKey {
    VaultKey::cloud_device(self.workspace.installation_id().expect("installation"))
  }

  /// Records that this computer trusts `server` by `pin` from now on, as if the server made a new certificate.
  pub fn pin_server(&self, server: ServerId, pin: &str) {
    let mut db = AppDb::open(&self._data.path().join("orchester.db")).expect("database");
    db.set_server_trust(server, Some(pin), false).expect("pinned");
  }

  /// Makes the servers of the cloud account `account` nobody's, as servers known before they had owners are when no
  /// account was signed in at the update.
  pub fn disown_servers_of(&self, account: &str) {
    rusqlite::Connection::open(self._data.path().join("orchester.db"))
      .expect("database")
      .execute("UPDATE servers SET owner = NULL WHERE owner = ?1", [account])
      .expect("nobody's");
  }

  /// Keeps `invite` as accepted here, as an earlier ask of this computer left it.
  pub fn keep_accepted(&self, invite: &orchester_store::AcceptedInvite) {
    let mut db = AppDb::open(&self._data.path().join("orchester.db")).expect("database");
    db.keep_accepted_invite(invite).expect("kept");
  }

  /// Records the signed-in account as signed in at the cloud at the web address `address` with its API there, as an
  /// earlier version recorded a sign-in at that address.
  pub fn move_account_to(&self, address: &str) {
    let mut db = AppDb::open(&self._data.path().join("orchester.db")).expect("database");
    let mut account = db.cloud_account().expect("account").expect("signed in");
    account.cloud_address = address.to_owned();
    account.cloud_api_address = None;
    db.set_cloud_account(Some(&account)).expect("moved");
  }

  /// Stores the web address `address` as Settings › Orchester › Advanced names the cloud, as an earlier version left
  /// it.
  pub fn store_cloud_address(&self, address: &str) {
    let mut db = AppDb::open(&self._data.path().join("orchester.db")).expect("database");
    let stored = StoredCloudAddress {
      web: address.to_owned(),
      api: None,
    };
    db.set_cloud_address(Some(&stored)).expect("stored");
  }
}

/// A team server with Alice as its admin, linked to `cloud`; returns it and its id, the identity tokens' audience.
pub fn linked_server(cloud: &AccountCloud) -> (TestServer, String) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  (server, audience)
}
