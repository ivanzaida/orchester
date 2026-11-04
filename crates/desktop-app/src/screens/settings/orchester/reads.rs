//! What Orchester Cloud reads of the cloud beside its snapshot ([`super::cloud_snapshot`]), each a lurq query keyed by
//! the signed-in account and shared with the All projects notice and the invite landing: the invites waiting, the
//! account's two-factor authentication, its team servers as the cloud lists them (those joined in the browser too) and
//! the invites accepted here whose join did not finish. They are live state: a screen that shows one shows its last
//! value at once and reads it again behind it as it mounts ([`Again::live`]), and only an entry never read yet is
//! missing. The window's root invalidates them all at each change of the servers' connections or of the cloud account
//! (an invite accepted, declined, finished or set aside, a sign-in, a sign-out, two-factor authentication turned on),
//! and the two-factor authentication again whenever the window gains or loses the focus, so turning it on in the
//! browser shows on coming back ([`invalidate`]). Nobody signed in reads nothing.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, query::QueryClient};
use orchester_core::id::CloudAccountId;

use super::{Invites, Kept, browser_joined::Listed, two_factor::TwoFactor, words};
use crate::queries::{Again, Session};

/// The invites waiting for the account `_account`, as the cloud lists them; a failed read is one of its answers.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
async fn cloud_invites(session: Session, _account: CloudAccountId) -> Result<Invites, ()> {
  let Session(services) = session;
  services.hold.released().await;
  Ok(match services.cloud.invites().await {
    Ok(invites) => Invites::Listed(Arc::new(invites)),
    Err(error) => {
      tracing::warn!(%error, "the cloud's invites could not be read");
      Invites::Failed(words::cloud_error(&error))
    }
  })
}

/// The two-factor authentication of the account `_account` (`GET /me`); `None` when the cloud cannot say (logged).
#[lurq::query(stale_time = "0s", gc_time = "24h")]
async fn cloud_two_factor(session: Session, _account: CloudAccountId) -> Result<TwoFactor, ()> {
  let Session(services) = session;
  services.hold.released().await;
  Ok(TwoFactor(services.cloud.two_factor().await.map_or_else(
    |error| {
      tracing::warn!(%error, "the account's two-factor authentication could not be read");
      None
    },
    Some,
  )))
}

/// The team servers of the account `_account`, as the cloud lists them (logged when it cannot say).
#[lurq::query(stale_time = "0s", gc_time = "24h")]
async fn cloud_servers(session: Session, _account: CloudAccountId) -> Result<Listed, ()> {
  let Session(services) = session;
  services.hold.released().await;
  let listed = services
    .cloud
    .team_servers()
    .await
    .map_err(|error| tracing::warn!(%error, "the account's team servers could not be read"))?;
  Ok(Listed(Arc::new(
    listed.into_iter().map(|listed| listed.server).collect(),
  )))
}

/// The invites the account `_account` accepted here whose join did not finish (logged when they cannot be read).
#[lurq::query(stale_time = "0s", gc_time = "24h")]
async fn cloud_accepted(session: Session, _account: CloudAccountId) -> Result<Kept, ()> {
  let Session(services) = session;
  services.hold.released().await;
  let kept = services
    .cloud
    .accepted_invites()
    .await
    .map_err(|error| tracing::warn!(%error, "the invites accepted here could not be read"))?;
  Ok(Kept(Arc::new(kept)))
}

/// Invalidates every read here, of every account: the servers' connections or the cloud account changed.
pub(crate) fn invalidate(client: &QueryClient) {
  client.invalidate(cloud_invites::all());
  client.invalidate(cloud_two_factor::all());
  client.invalidate(cloud_servers::all());
  client.invalidate(cloud_accepted::all());
}

/// Invalidates the two-factor authentication: the window gained or lost the focus.
pub(crate) fn invalidate_two_factor(client: &QueryClient) {
  client.invalidate(cloud_two_factor::all());
}

/// What the Orchester Cloud tab shows of the cloud beside its snapshot, once each part was read.
pub(super) struct Shown {
  pub invites: Invites,
  pub two_factor: TwoFactor,
  pub accepted: Kept,
  pub listed: Listed,
}

/// A screen's reads of the cloud, each read again behind its last value as the screen mounts.
pub(super) struct Reads {
  invites: Again,
  two_factor: Again,
  servers: Again,
  accepted: Again,
}

impl Reads {
  pub(super) fn new() -> Self {
    Self {
      invites: Again::live(),
      two_factor: Again::live(),
      servers: Again::live(),
      accepted: Again::live(),
    }
  }

  /// The invites waiting for `signed_in`; `None` until they were read once.
  pub(super) fn invites(&self, ctx: &mut Ctx, signed_in: Option<CloudAccountId>) -> Option<Invites> {
    let Some(account) = signed_in else {
      return Some(Invites::SignedOut);
    };
    let session = Session::of(ctx);
    let query = ctx.query(cloud_invites(session, account));
    self.invites.follow(0, &query);
    let data = query.data();
    data.map(|invites| Invites::clone(&invites))
  }

  /// The two-factor authentication of `signed_in`; `None` until it was read once, `Some(TwoFactor(None))` while nobody
  /// is signed in or when the cloud cannot say, so a page that waits on it tells *not read yet* from *cannot say*.
  pub(super) fn two_factor(&self, ctx: &mut Ctx, signed_in: Option<CloudAccountId>) -> Option<TwoFactor> {
    let Some(account) = signed_in else {
      return Some(TwoFactor(None));
    };
    let session = Session::of(ctx);
    let query = ctx.query(cloud_two_factor(session, account));
    self.two_factor.follow(0, &query);
    let data = query.data();
    data.map(|status| TwoFactor::clone(&status))
  }

  /// The team servers of `signed_in`; none while signed out or when the cloud could not say, `None` until read once.
  fn listed(&self, ctx: &mut Ctx, signed_in: Option<CloudAccountId>) -> Option<Listed> {
    let Some(account) = signed_in else {
      return Some(Listed::default());
    };
    let session = Session::of(ctx);
    let query = ctx.query(cloud_servers(session, account));
    self.servers.follow(0, &query);
    let (data, failed) = (query.data(), query.error().is_some());
    data
      .map(|listed| Listed::clone(&listed))
      .or_else(|| failed.then(Listed::default))
  }

  /// The invites `signed_in` accepted here; none while signed out or when they cannot be read, `None` until read once.
  fn accepted(&self, ctx: &mut Ctx, signed_in: Option<CloudAccountId>) -> Option<Kept> {
    let Some(account) = signed_in else {
      return Some(Kept::default());
    };
    let session = Session::of(ctx);
    let query = ctx.query(cloud_accepted(session, account));
    self.accepted.follow(0, &query);
    let (data, failed) = (query.data(), query.error().is_some());
    data
      .map(|kept| Kept::clone(&kept))
      .or_else(|| failed.then(Kept::default))
  }

  /// Everything the tab shows of the cloud for `signed_in`; `None` while a part was never read, when the tab shows its
  /// skeleton. Each part is asked for in every render.
  pub(super) fn shown(&self, ctx: &mut Ctx, signed_in: Option<CloudAccountId>) -> Option<Shown> {
    let invites = self.invites(ctx, signed_in);
    let two_factor = self.two_factor(ctx, signed_in);
    let accepted = self.accepted(ctx, signed_in);
    let listed = self.listed(ctx, signed_in);
    Some(Shown {
      invites: invites?,
      two_factor: two_factor?,
      accepted: accepted?,
      listed: listed?,
    })
  }
}
