//! A desktop with no project open: an application database in a folder of its own, a keychain in memory, and the
//! session's servers, Orchester Cloud account and team joins, as the app builds them. A Cloud sign-in goes to the
//! stand-in cloud on loopback.

use std::{collections::HashMap, future::Future, sync::Arc};

use orchester_api::{
  Credential,
  client::{CloudAddress, ServerAddress},
  test_support::account_cloud::{AccountCloud, EMAIL, PASSWORD},
};
use orchester_core::{
  id::ServerId,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
};
use orchester_store::AppDb;
use orchester_workspace::{
  secrets::Vault,
  sharing::{ServerEntry, Servers, cloud::Cloud, team_join::TeamJoin},
  workspace::Workspace,
};
use parking_lot::Mutex;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// A keychain in memory: tests never touch the keychain of the user running them.
#[derive(Default)]
pub struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

pub struct Desk {
  pub runtime: Runtime,
  pub workspace: Workspace,
  pub vault: Arc<MemoryVault>,
  pub servers: Servers,
  pub cloud: Cloud,
  pub join: TeamJoin,
  _data: TempDir,
}

impl Desk {
  /// A desktop signed in to nothing, with no project registered.
  pub fn new() -> Self {
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    let vault = Arc::new(MemoryVault::default());
    let keychain = Vault::with(vault.clone());
    let servers = Servers::new(workspace.clone(), keychain.clone());
    let cloud = Cloud::new(workspace.clone(), keychain, servers.clone());
    Self {
      runtime: Runtime::new().expect("runtime"),
      join: TeamJoin::new(servers.clone(), cloud.clone()),
      workspace,
      vault,
      servers,
      cloud,
      _data: data,
    }
  }

  /// [`Self::new`] signed in to the stand-in `cloud` as Bohdan.
  pub fn signed_in(cloud: &AccountCloud) -> Self {
    let desk = Self::new();
    let address = ServerAddress::parse(&cloud.url()).expect("the stand-in cloud's address");
    desk
      .run(async {
        desk
          .cloud
          .set_address(Some(CloudAddress::new(address.clone(), Some(address))))
          .await?;
        desk
          .cloud
          .sign_in(EMAIL.into(), Credential::new(PASSWORD.into()))
          .await
          .map(drop)
      })
      .expect("the stand-in cloud signs this computer in");
    desk
  }

  pub fn run<T>(&self, work: impl Future<Output = T>) -> T {
    self.runtime.block_on(work)
  }

  /// The servers of the account signed in now, or of none.
  pub fn entries(&self) -> Vec<ServerEntry> {
    self.servers.entries().expect("the servers")
  }

  /// The device token this computer keeps for `server`.
  pub fn device_token(&self, server: ServerId) -> Option<String> {
    self.vault.0.lock().get(&VaultKey::server_token(server)).cloned()
  }

  /// No project is registered: the joins need none.
  pub fn assert_no_project(&self) {
    assert!(self.workspace.projects().expect("projects").is_empty());
  }
}

/// An address nothing answers at: a port of this computer that was just closed.
pub fn closed_address() -> ServerAddress {
  let port = std::net::TcpListener::bind(("127.0.0.1", 0))
    .expect("listen")
    .local_addr()
    .expect("address")
    .port();
  ServerAddress::parse(&format!("http://127.0.0.1:{port}")).expect("address")
}
