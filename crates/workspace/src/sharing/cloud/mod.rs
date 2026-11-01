//! This computer's Orchester Cloud account (`docs/architecture/server.md`, "Orchester Cloud", row 41): signing in with
//! the person's email and password through the cloud's API, signing out, the cloud's web pages *Create an account* and
//! *Forgot password* open in the browser, the cloud's web and API addresses (the build's, or a test cloud's from
//! Settings › Orchester Cloud › Advanced), and the account's invites ([`invites`]). The account is in the application
//! database's `cloud_account` row; its device token in the keychain, under this installation's id, so a development
//! build or a QA copy never signs the installed app in or out. The app works without a sign-in (the person's decision,
//! 2026-10-28): only team features ask for one. The projects and team servers added while nobody is signed in are this
//! computer's and show to everyone; those added while an account is signed in are that account's: it alone sees them,
//! and they stay, with their tokens, for when it signs in again, their shared projects kept in step meanwhile
//! ([`state`]).

mod accepting;
mod asking;
mod error;
mod invites;
mod moved;
mod seen;
mod signing_in;
mod state;
mod two_factor;

use std::sync::{
  Arc,
  atomic::{AtomicU64, Ordering},
};

use orchester_api::{
  client::{AddressError, CloudAddress, CloudClient, ServerAddress},
  cloud::CloudErrorCode,
};
use orchester_core::{
  id::CloudAccountId,
  secret::{SecretValue, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_store::{CloudSignInEnd, EndedCloudSignIn, StoredCloudAccount, StoredCloudAddress};
use parking_lot::Mutex;
use tokio::sync::watch;

pub use self::{
  accepting::Accepted,
  asking::{Checked, MAX_ASKS, may_ask, waits},
  error::CloudError,
  invites::ServerJoin,
  seen::keep_activity_to_the_account,
  signing_in::{CHALLENGE_LIFE, Challenge, CodeKind, CodeSignedIn, SignInStep},
  state::{CHECK_EVERY, RETRY_AFTER_CAP, RETRY_CAP, RETRY_EVERY, Reach, SignIn, keep_checking, retry_wait},
  two_factor::FEW_RECOVERY_CODES,
};
use super::Servers;
use crate::{background, secrets::Vault, workspace::Workspace};

/// The public Orchester Cloud's web address (`docs/architecture/server.md`, row 59), and the issuer of its identity
/// tokens and key sets wherever its API answers.
pub const PUBLIC_ADDRESS: &str = "https://orchester.cloud";

/// The public cloud's API address, its API at `/v1` there (`docs/cloud-api.md`, "Base"). Until this app version its
/// API was at `/api/v1` on [`PUBLIC_ADDRESS`]; what this computer stored with that moves here at the start ([`moved`]).
pub const PUBLIC_API_ADDRESS: &str = "https://api.orchester.cloud";

/// The public cloud's address until 2026-10-25, which still answers while people update (`docs/cloud-hosting.md` in
/// `ivanzaida/orchester-platform`, "Moving the cloud"): what this computer stored with it moves to the public cloud's
/// addresses at the start ([`moved`]).
const FORMER_PUBLIC_ADDRESS: &str = "https://orchester-cloud.lurq.dev";

/// Where Orchester Cloud's web pages are for this build: `ORCHESTER_CLOUD_URL` at build time, or the public cloud's.
pub const BUILD_ADDRESS: &str = match option_env!("ORCHESTER_CLOUD_URL") {
  Some(address) => address,
  None => PUBLIC_ADDRESS,
};

/// Where Orchester Cloud's API is for this build: `ORCHESTER_CLOUD_API_URL` at build time, or the public cloud's. A
/// build that names another cloud with `ORCHESTER_CLOUD_URL` alone has none: that cloud's API is on its web address.
pub const BUILD_API_ADDRESS: Option<&str> = match (
  option_env!("ORCHESTER_CLOUD_URL"),
  option_env!("ORCHESTER_CLOUD_API_URL"),
) {
  (_, Some(api)) => Some(api),
  (Some(_), None) => None,
  (None, None) => Some(PUBLIC_API_ADDRESS),
};

/// The build's own cloud: [`BUILD_ADDRESS`] and [`BUILD_API_ADDRESS`].
pub fn build_address() -> Result<CloudAddress, AddressError> {
  parse_address(BUILD_ADDRESS, BUILD_API_ADDRESS)
}

/// The cloud at the web address `web` whose API is at `api`, or on `web` without one.
fn parse_address(web: &str, api: Option<&str>) -> Result<CloudAddress, AddressError> {
  Ok(CloudAddress::new(
    ServerAddress::parse(web)?,
    api.map(ServerAddress::parse).transpose()?,
  ))
}

/// The cloud this computer uses, and whether it is the build's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudInUse {
  pub address: CloudAddress,
  /// `false` while Settings › Orchester Cloud › Advanced names another cloud (a test or self-run cloud).
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
  /// *Turn on two-factor authentication*, and making new recovery codes: the account page's Security section,
  /// `/account#security`.
  Security,
}

impl WebPage {
  const fn path(self) -> &'static str {
    match self {
      Self::SignUp => "/signup",
      Self::Forgot => "/forgot",
      Self::SignIn => "/signin",
      Self::Security => "/account#security",
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
  sign_in: watch::Sender<SignIn>,
  reach: watch::Sender<Reach>,
  /// Moves with every sign-in and every end of one, under the database's lock: a refusal of a token that was signed
  /// out or replaced since its request went out ends nothing newer.
  generation: AtomicU64,
  /// Held while a sign-in writes its token and account, and while an end removes them, so a token removed late never
  /// takes a newer sign-in's with it.
  keychain: Mutex<()>,
  /// The recovery codes left after this session's sign-in, when a recovery code finished it ([`two_factor`]).
  recovery_codes_left: Mutex<Option<u32>>,
}

/// This computer's stored sign-in: the account, its token as the keychain gave it, and the sign-in's generation.
struct Kept {
  account: StoredCloudAccount,
  token: Result<SecretValue, VaultError>,
  generation: u64,
}

/// What ending a sign-in found.
enum Forgot {
  /// A newer sign-in or end came first: nothing changed.
  Stale,
  /// Ended; why the last sign-in from here ended, if the database knows.
  Ended(Option<EndedCloudSignIn>),
}

impl PartialEq for Cloud {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl Cloud {
  /// The cloud account of `workspace`'s session, signed in or not as its application database says, with what it
  /// stored at the public cloud's former address moved to its new one. Blocking.
  pub fn new(workspace: Workspace, vault: Vault, servers: Servers) -> Self {
    moved::move_from_former_address(&workspace);
    let sign_in = stored_sign_in(&workspace);
    Self {
      inner: Arc::new(Inner {
        workspace,
        vault,
        servers,
        changes: watch::channel(0).0,
        sign_in: watch::channel(sign_in).0,
        reach: watch::channel(Reach::Unknown).0,
        generation: AtomicU64::new(0),
        keychain: Mutex::new(()),
        recovery_codes_left: Mutex::new(None),
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

  /// The cloud this computer uses. Blocking.
  pub fn address(&self) -> Result<CloudInUse, CloudError> {
    address_in_use(&self.inner.workspace)
  }

  /// The address of `page` on the web address of the cloud this computer uses. Blocking.
  pub fn web_page(&self, page: WebPage) -> Result<String, CloudError> {
    Ok(format!("{}{}", self.address()?.address.web().as_str(), page.path()))
  }

  /// The account signed in from this computer. Blocking.
  pub fn account(&self) -> Result<Option<StoredCloudAccount>, CloudError> {
    Ok(self.inner.workspace.db.lock().cloud_account()?)
  }

  /// Signs this computer out of the cloud: the cloud ends its token if it can be reached, and the token leaves the
  /// keychain and the account the database either way. The account's team servers and their tokens stay, hidden, for
  /// when it signs in again.
  pub async fn sign_out(&self) -> Result<(), CloudError> {
    self.end_session().await;
    let forgotten = self
      .forget((CloudSignInEnd::SignedOut, None), Timestamp::now(), None)
      .await;
    self.changed();
    forgotten.map(drop)
  }

  /// Uses the cloud at `address` from now on, `None` for the build's own; a cloud account signed in elsewhere is
  /// signed out first.
  pub async fn set_address(&self, address: Option<CloudAddress>) -> Result<(), CloudError> {
    if self.account_now().await?.is_some() {
      self.sign_out().await?;
    }
    let workspace = self.inner.workspace.clone();
    background::run(move || {
      let stored = address.map(|address| StoredCloudAddress {
        web: address.web().as_str().to_owned(),
        api: address.api().map(|api| api.as_str().to_owned()),
      });
      workspace.db.lock().set_cloud_address(stored.as_ref())
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
    let kept = match self.kept().await {
      Ok(Some(kept)) => kept,
      Ok(None) => return,
      Err(error) => return tracing::warn!(%error, "cannot read this computer's cloud session to end it"),
    };
    let token = match kept.token {
      Ok(token) => token,
      Err(VaultError::NotFound) => return,
      Err(error) => return tracing::warn!(%error, "cannot read this computer's cloud token to end its session"),
    };
    let client = match account_address(&kept.account).map(CloudClient::new) {
      Ok(Ok(client)) => client,
      Ok(Err(error)) => return tracing::warn!(%error, "cannot reach the cloud to end this computer's session"),
      Err(error) => return tracing::warn!(%error, "the account's cloud has no address to end its session at"),
    };
    if let Err(error) = client.sign_out(&token).await {
      // The token leaves this computer either way; the cloud ends it after 90 days without use.
      tracing::warn!(%error, "Orchester Cloud did not end this computer's session");
    }
  }

  /// Ends this computer's sign-in for `reason` at `at`: signed out from now on, whatever the database or the
  /// keychain answer. The account leaves the database first, then its token the keychain; a token the keychain keeps
  /// (it refused) does no harm without an account, and the next sign-in replaces it. With `generation`, only the
  /// sign-in of that generation ends: a refusal that arrives after a newer sign-in or end changes nothing (`false`).
  /// `invite` names what the invite that asked for a sign-in with a code was to ([`CloudSignInEnd::TwoFactor`]).
  async fn forget(
    &self,
    (reason, invite): (CloudSignInEnd, Option<String>),
    at: Timestamp,
    generation: Option<u64>,
  ) -> Result<bool, CloudError> {
    let cloud = self.clone();
    let ending = (reason, invite.clone());
    let forgot = background::run(move || cloud.forget_stored(ending, at, generation))
      .await
      .map_err(CloudError::from)
      .and_then(|forgot| forgot);
    let ended = match &forgot {
      Ok(Forgot::Stale) => return Ok(false),
      Ok(Forgot::Ended(ended)) => ended.clone(),
      // The database did not take the end: signed out all the same, with what this session knows.
      Err(_) => self.ended_now((reason, invite), at),
    };
    self.recovery_codes_used(None);
    self.inner.reach.send_replace(Reach::Unknown);
    self.inner.sign_in.send_replace(SignIn::SignedOut {
      ended,
      since: Timestamp::now(),
    });
    // The account's servers leave the lists; their tokens stay in the keychain for when it signs in again.
    self.servers().changed();
    forgot.map(|_| true)
  }

  /// [`Self::forget`]'s blocking part: the database's account, then the keychain's token. Blocking.
  fn forget_stored(
    &self,
    ending: (CloudSignInEnd, Option<String>),
    at: Timestamp,
    generation: Option<u64>,
  ) -> Result<Forgot, CloudError> {
    let inner = &self.inner;
    let _keychain = inner.keychain.lock();
    let stored = {
      let mut db = inner.workspace.db.lock();
      if generation.is_some_and(|generation| generation != inner.generation.load(Ordering::SeqCst)) {
        return Ok(Forgot::Stale);
      }
      inner.generation.fetch_add(1, Ordering::SeqCst);
      forget_account(&mut db, ending, at)
    };
    match device_key(&inner.workspace).map(|key| inner.vault.get().delete(&key)) {
      Ok(Ok(()) | Err(VaultError::NotFound)) => {}
      Ok(Err(error)) => tracing::warn!(%error, "this computer's cloud token stays in the keychain, without an account"),
      Err(error) => tracing::warn!(%error, "cannot name this computer's cloud token to remove it"),
    }
    stored.map(Forgot::Ended)
  }

  /// Why the sign-in of now ended, as this session knows it, for when the database cannot say.
  fn ended_now(&self, (reason, invite): (CloudSignInEnd, Option<String>), at: Timestamp) -> Option<EndedCloudSignIn> {
    match self.sign_in_state() {
      SignIn::SignedIn { email, .. } => Some(EndedCloudSignIn {
        email,
        reason,
        ended_at: at,
        invite,
      }),
      SignIn::SignedOut { ended, .. } => ended,
    }
  }

  /// The account signed in from this computer, its token as the keychain gives it, and the sign-in's generation;
  /// `None` while none is.
  async fn kept(&self) -> Result<Option<Kept>, CloudError> {
    let cloud = self.clone();
    background::run(move || -> Result<Option<Kept>, CloudError> {
      let inner = &cloud.inner;
      let (account, generation) = {
        let db = inner.workspace.db.lock();
        (db.cloud_account()?, inner.generation.load(Ordering::SeqCst))
      };
      let Some(account) = account else {
        return Ok(None);
      };
      let token = inner.vault.get().read(&device_key(&inner.workspace)?);
      Ok(Some(Kept {
        account,
        token,
        generation,
      }))
    })
    .await?
  }

  /// The client of the cloud the account signed in at, its device token and the account, all of one stored sign-in;
  /// signed out without either.
  async fn signed_in(&self) -> Result<(CloudClient, SecretValue, CloudAccountId), CloudError> {
    self
      .session()
      .await
      .map(|(client, token, _, account)| (client, token, account))
  }

  /// [`Self::signed_in`] with the sign-in's generation. An account whose token is gone from the keychain, or which the
  /// keychain does not give, is signed out here (reason *other*): a sign-in that cannot be checked does not stay open.
  async fn session(&self) -> Result<(CloudClient, SecretValue, u64, CloudAccountId), CloudError> {
    let Some(kept) = self.kept().await? else {
      return Err(CloudError::SignedOut);
    };
    let token = match kept.token {
      Ok(token) => token,
      Err(error) => {
        if matches!(error, VaultError::NotFound) {
          tracing::warn!("this computer's cloud token is gone from the keychain; it is signed out");
        } else {
          tracing::warn!(%error, "the keychain does not give this computer's cloud token; it is signed out");
        }
        if self
          .forget((CloudSignInEnd::Other, None), Timestamp::now(), Some(kept.generation))
          .await?
        {
          self.changed();
        }
        return Err(CloudError::SignedOut);
      }
    };
    let client = CloudClient::new(account_address(&kept.account)?)?;
    Ok((client, token, kept.generation, kept.account.account))
  }

  /// Runs `call` with the signed-in client and token; a token the cloud no longer accepts signs this computer out with
  /// the reason the cloud gives, unless a newer sign-in or end came meanwhile. Whether the cloud answered is kept in
  /// [`Self::reach`].
  async fn with_token<T, F, Fut>(&self, call: F) -> Result<T, CloudError>
  where
    F: FnOnce(CloudClient, SecretValue) -> Fut,
    Fut: Future<Output = Result<T, CloudError>>,
  {
    let (client, token, generation, _) = self.session().await?;
    let answer = call(client, token).await;
    self.inner.reach.send_if_modified(|reach| {
      let after = reach.after(&answer, Timestamp::now());
      std::mem::replace(reach, after) != after
    });
    match answer {
      Err(error) if error.cloud_code() == Some(CloudErrorCode::Unauthorized) => {
        let (reason, at) = error.ended().map_or((None, None), |(reason, at)| (Some(reason), at));
        let at = at.map_or_else(Timestamp::now, Timestamp::from_unix_millis);
        if self.forget((state::end_of(reason), None), at, Some(generation)).await? {
          self.changed();
        }
        Err(CloudError::SignedOut)
      }
      answer => answer,
    }
  }

  fn servers(&self) -> &Servers {
    &self.inner.servers
  }
}

/// The cloud `workspace`'s computer uses: the one Settings › Orchester Cloud › Advanced names, or the build's own.
/// Blocking.
pub(crate) fn address_in_use(workspace: &Workspace) -> Result<CloudInUse, CloudError> {
  let set = workspace.db.lock().cloud_address()?;
  Ok(match set {
    Some(set) => CloudInUse {
      address: parse_address(&set.web, set.api.as_deref())?,
      default: false,
    },
    None => CloudInUse {
      address: build_address()?,
      default: true,
    },
  })
}

/// Whether `workspace`'s application database has an account signed in, and why the last one ended if not. A database
/// that cannot be read counts as signed out: an account's projects are shown only to a sign-in that can be read.
fn stored_sign_in(workspace: &Workspace) -> SignIn {
  let db = workspace.db.lock();
  match (db.cloud_account(), db.cloud_sign_in_ended()) {
    (Ok(Some(account)), _) => SignIn::SignedIn {
      account: account.account,
      email: account.email,
    },
    (Ok(None), Ok(ended)) => SignIn::SignedOut {
      ended,
      since: Timestamp::now(),
    },
    (Err(error), _) | (_, Err(error)) => {
      tracing::error!(%error, "cannot read this computer's cloud sign-in; it counts as signed out");
      SignIn::SignedOut {
        ended: None,
        since: Timestamp::now(),
      }
    }
  }
}

/// Removes the account from `db` and records that its sign-in ended for `reason` at `at`; why the last sign-in from
/// here ended.
fn forget_account(
  db: &mut orchester_store::AppDb,
  (reason, invite): (CloudSignInEnd, Option<String>),
  at: Timestamp,
) -> Result<Option<EndedCloudSignIn>, CloudError> {
  let ended = db.cloud_account()?.map(|account| EndedCloudSignIn {
    email: account.email,
    reason,
    ended_at: at,
    invite,
  });
  db.set_cloud_account(None)?;
  if ended.is_some() {
    db.set_cloud_sign_in_ended(ended.as_ref())?;
  }
  // The invites accepted here belong to the account that accepted them. Signed out to sign in again with a code, the
  // same person comes back for them; another account forgets them as it signs in ([`signing_in`]).
  if reason != CloudSignInEnd::TwoFactor {
    db.forget_accepted_invites()?;
  }
  Ok(match ended {
    Some(ended) => Some(ended),
    None => db.cloud_sign_in_ended()?,
  })
}

/// Where this installation keeps its cloud device token.
fn device_key(workspace: &Workspace) -> Result<VaultKey, CloudError> {
  Ok(VaultKey::cloud_device(workspace.installation_id()?))
}

/// The cloud `account` signed in at.
fn account_address(account: &StoredCloudAccount) -> Result<CloudAddress, AddressError> {
  parse_address(&account.cloud_address, account.cloud_api_address.as_deref())
}
