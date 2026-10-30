//! A session for the app's tests and QA examples, with a keychain in memory, never the OS's: signed out as on a first
//! run, or signed in as a person would, through the workspace, to a stand-in cloud on loopback (`account_cloud`), which
//! answers the app's check of its sign-in, so the session stays signed in. Fixture data for tests and QA only: the
//! app's program never includes it.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

pub use orchester_api::test_support::account_cloud;
use orchester_api::{
  Credential,
  client::{CloudAddress, ServerAddress},
};
use orchester_core::secret::{SecretValue, SecretVault, VaultError, VaultKey};
use orchester_workspace::secrets::Vault;
use parking_lot::Mutex;
use tokio::runtime::Handle;

use self::account_cloud::{AccountCloud, EMAIL, PASSWORD};
use crate::{services::Services, startup::Launch};

/// A keychain in memory: nothing reaches the keychain of the user running the tests or the QA copy.
#[derive(Default)]
pub struct MemoryKeychain(Mutex<HashMap<VaultKey, String>>);

impl MemoryKeychain {
  /// Every value it keeps, e.g. the one token a QA server stored in a keychain of its own.
  pub fn values(&self) -> Vec<String> {
    self.0.lock().values().cloned().collect()
  }
}

impl SecretVault for MemoryKeychain {
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

/// A new keychain in memory.
pub fn keychain() -> Vault {
  Vault::with(Arc::new(MemoryKeychain::default()))
}

/// The session of the application database `database` with a keychain in memory, its runners the program `runner`,
/// on `runtime`; signed in to `cloud`. A session that did not start is returned as it is, to say why.
pub fn launch_signed_in(database: &Path, runner: PathBuf, runtime: &Handle, cloud: &AccountCloud) -> Launch {
  let launch = launch(database, runner, runtime);
  if let Some(services) = launch.services() {
    sign_in(services, runtime, cloud);
  }
  launch
}

/// [`launch_signed_in`] without signing in: this computer's projects only, as on a first run.
pub fn launch(database: &Path, runner: PathBuf, runtime: &Handle) -> Launch {
  Launch::begin_with_vault(database, runner, runtime, (None, keychain()))
}

/// The stand-in `cloud` as its web and its API address, the API at `/v1` there as the public cloud's is
/// (`docs/cloud-api.md`, "Base").
pub fn cloud_address(cloud: &AccountCloud) -> CloudAddress {
  let address = ServerAddress::parse(&cloud.url()).expect("the stand-in cloud's address");
  CloudAddress::new(address.clone(), Some(address))
}

/// Signs the session of `services` in to the stand-in `cloud`, as a person does in Settings › Orchester Cloud: it uses
/// that cloud from now on (Advanced) and signs in with its account.
pub fn sign_in(services: &Services, runtime: &Handle, cloud: &AccountCloud) {
  let address = cloud_address(cloud);
  runtime
    .block_on(async {
      services.cloud.set_address(Some(address)).await?;
      services
        .cloud
        .sign_in(EMAIL.into(), Credential::new(PASSWORD.into()))
        .await
    })
    .expect("the stand-in cloud signs this computer in");
}
