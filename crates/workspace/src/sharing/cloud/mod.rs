//! This computer's Orchester Cloud account (`docs/architecture/server.md`, "Orchester Cloud", row 41): signing in with
//! the person's email and password through the cloud's API, signing out, the cloud's web pages *Create an account* and
//! *Forgot password* open in the browser, the cloud's address (the build's, or a test cloud's from Settings ›
//! Orchester › Advanced), and the account's invites ([`invites`]). The account is in the application database's
//! `cloud_account` row; its device token in the keychain, under this installation's id, so a development build or a
//! QA copy never signs the installed app in or out. Team servers joined through the cloud keep working without it.

mod accepting;
mod asking;
mod error;
mod invites;

use std::sync::Arc;

use orchester_api::{
  Credential,
  client::{ClientError, CloudClient, ServerAddress},
  cloud::{CloudErrorCode, CloudSignIn},
};
use orchester_core::{
  secret::{SecretValue, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_store::StoredCloudAccount;
use tokio::sync::watch;

pub use self::{
  accepting::Accepted,
  asking::{Checked, MAX_ASKS, may_ask, waits},
  error::CloudError,
  invites::ServerJoin,
};
use super::{Servers, device};
use crate::{background, secrets::Vault, workspace::Workspace};

/// Where Orchester Cloud runs for this build: `ORCHESTER_CLOUD_URL` at build time, or the public cloud
/// (`docs/architecture/server.md`, row 59).
pub const BUILD_ADDRESS: &str = match option_env!("ORCHESTER_CLOUD_URL") {
  Some(address) => address,
  None => "https://orchester-cloud.lurq.dev",
};

/// The cloud's address this computer uses, and whether it is the build's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudAddress {
  pub address: ServerAddress,
  /// `false` while Settings › Orchester › Advanced names another cloud (a test cloud).
  pub default: bool,
}

/// A page of the cloud's web front, which the app opens in the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebPage {
  /// *Create an account*: `/signup`.
  SignUp,
  /// *Forgot password?*: `/forgot`.
  Forgot,
  /// *Open Orchester Cloud*, e.g. to ask for a new confirmation link: `/signin`.
  SignIn,
}

impl WebPage {
  const fn path(self) -> &'static str {
    match self {
      Self::SignUp => "/signup",
      Self::Forgot => "/forgot",
      Self::SignIn => "/signin",
    }
  }
}

/// The session's Orchester Cloud. Clones share the account and its changes.
#[derive(Clone)]
pub struct Cloud {
  inner: Arc<Inner>,
}

struct Inner {
  workspace: Workspace,
  vault: Vault,
  servers: Servers,
  /// Counts sign-ins, sign-outs and answered invites, for the screens.
  changes: watch::Sender<u64>,
}

impl PartialEq for Cloud {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl Cloud {
  pub fn new(workspace: Workspace, vault: Vault, servers: Servers) -> Self {
    Self {
      inner: Arc::new(Inner {
        workspace,
        vault,
        servers,
        changes: watch::channel(0).0,
      }),
    }
  }

  /// Wakes whenever the account or its invites changed.
  pub fn subscribe(&self) -> watch::Receiver<u64> {
    self.inner.changes.subscribe()
  }

  fn changed(&self) {
    self.inner.changes.send_modify(|count| *count += 1);
  }

  /// The cloud's address this computer uses. Blocking.
  pub fn address(&self) -> Result<CloudAddress, CloudError> {
    let set = self.inner.workspace.db.lock().cloud_address()?;
    Ok(match set {
      Some(text) => CloudAddress {
        address: ServerAddress::parse(&text)?,
        default: false,
      },
      None => CloudAddress {
        address: ServerAddress::parse(BUILD_ADDRESS)?,
        default: true,
      },
    })
  }

  /// The address of `page` on the cloud this computer uses. Blocking.
  pub fn web_page(&self, page: WebPage) -> Result<String, CloudError> {
    Ok(format!("{}{}", self.address()?.address.as_str(), page.path()))
  }

  /// The account signed in from this computer. Blocking.
  pub fn account(&self) -> Result<Option<StoredCloudAccount>, CloudError> {
    Ok(self.inner.workspace.db.lock().cloud_account()?)
  }

  /// Signs this computer in to the cloud with the person's `email` and `password`, in place of any account signed in
  /// before. `invalid_credentials`, `email_unconfirmed` and `rate_limited` come back as the cloud's refusal.
  pub async fn sign_in(&self, email: String, password: Credential) -> Result<StoredCloudAccount, CloudError> {
    let cloud = background::run({
      let cloud = self.clone();
      move || cloud.address()
    })
    .await??;
    let client = CloudClient::new(cloud.address.clone())?;
    let device = device::cloud_device();
    let signed_in = client
      .sign_in(&CloudSignIn {
        email,
        password,
        device: device.clone(),
      })
      .await?;
    // The cloud issues `orcc_` and 64 hex characters; a blank answer is not a token.
    let token = SecretValue::new(signed_in.token.expose().to_owned()).map_err(|_| CloudError::SignedOut)?;
    if self.account_now().await?.is_some() {
      self.end_session().await;
    }
    let account = StoredCloudAccount {
      account: signed_in.account.id,
      email: signed_in.account.email,
      display_name: signed_in.account.display_name,
      device_name: device.name,
      signed_in_at: Timestamp::now(),
      cloud_address: cloud.address.as_str().to_owned(),
    };
    let (workspace, vault, kept) = (self.inner.workspace.clone(), self.inner.vault.clone(), account.clone());
    background::run(move || -> Result<(), CloudError> {
      let key = device_key(&workspace)?;
      vault.get().store(&key, &token)?;
      workspace.db.lock().set_cloud_account(Some(&kept))?;
      Ok(())
    })
    .await??;
    self.changed();
    Ok(account)
  }

  /// Signs this computer out of the cloud: the cloud ends its token if it can be reached, and the token leaves the
  /// keychain and the account the database either way. Team servers joined through the cloud keep working.
  pub async fn sign_out(&self) -> Result<(), CloudError> {
    self.end_session().await;
    self.forget().await?;
    self.changed();
    Ok(())
  }

  /// Uses the cloud at `address` from now on, `None` for the build's own; a cloud account signed in elsewhere is
  /// signed out first.
  pub async fn set_address(&self, address: Option<ServerAddress>) -> Result<(), CloudError> {
    if self.account_now().await?.is_some() {
      self.sign_out().await?;
    }
    let workspace = self.inner.workspace.clone();
    background::run(move || {
      let text = address.as_ref().map(ServerAddress::as_str);
      workspace.db.lock().set_cloud_address(text)
    })
    .await??;
    self.changed();
    Ok(())
  }

  async fn account_now(&self) -> Result<Option<StoredCloudAccount>, CloudError> {
    let cloud = self.clone();
    background::run(move || cloud.account()).await?
  }

  /// Asks the cloud to end this computer's token; a cloud that cannot be reached ends it by itself once unused.
  async fn end_session(&self) {
    match self.signed_in().await {
      Ok((client, token)) => {
        if let Err(error) = client.sign_out(&token).await {
          // The token leaves this computer either way; the cloud ends it after 90 days without use.
          tracing::warn!(%error, "Orchester Cloud did not end this computer's session");
        }
      }
      Err(CloudError::SignedOut) => {}
      Err(error) => tracing::warn!(%error, "cannot read this computer's cloud session to end it"),
    }
  }

  /// Removes the token and the account from this computer.
  async fn forget(&self) -> Result<(), CloudError> {
    let (workspace, vault) = (self.inner.workspace.clone(), self.inner.vault.clone());
    background::run(move || -> Result<(), CloudError> {
      let key = device_key(&workspace)?;
      match vault.get().delete(&key) {
        Ok(()) | Err(VaultError::NotFound) => {}
        Err(error) => return Err(error.into()),
      }
      let mut db = workspace.db.lock();
      db.set_cloud_account(None)?;
      // The invites accepted here belong to the account that accepted them.
      db.forget_accepted_invites()?;
      Ok(())
    })
    .await?
  }

  /// The client of the cloud the account signed in at, and its device token; signed out without either.
  async fn signed_in(&self) -> Result<(CloudClient, SecretValue), CloudError> {
    let (workspace, vault) = (self.inner.workspace.clone(), self.inner.vault.clone());
    let (account, token) = background::run(move || -> Result<_, CloudError> {
      let Some(account) = workspace.db.lock().cloud_account()? else {
        return Err(CloudError::SignedOut);
      };
      match vault.get().read(&device_key(&workspace)?) {
        Ok(token) => Ok((account, token)),
        Err(VaultError::NotFound) => Err(CloudError::SignedOut),
        Err(error) => Err(error.into()),
      }
    })
    .await??;
    let client = CloudClient::new(ServerAddress::parse(&account.cloud_address)?)?;
    Ok((client, token))
  }

  /// Runs `call` with the signed-in client and token; a token the cloud no longer accepts signs this computer out.
  async fn with_token<T, F, Fut>(&self, call: F) -> Result<T, CloudError>
  where
    F: FnOnce(CloudClient, SecretValue) -> Fut,
    Fut: Future<Output = Result<T, CloudError>>,
  {
    let (client, token) = self.signed_in().await?;
    match call(client, token).await {
      Err(CloudError::Client(ClientError::Cloud {
        code: CloudErrorCode::Unauthorized,
        ..
      })) => {
        self.forget().await?;
        self.changed();
        Err(CloudError::SignedOut)
      }
      answer => answer,
    }
  }

  fn servers(&self) -> &Servers {
    &self.inner.servers
  }
}

/// Where this installation keeps its cloud device token.
fn device_key(workspace: &Workspace) -> Result<VaultKey, CloudError> {
  Ok(VaultKey::cloud_device(workspace.installation_id()?))
}
