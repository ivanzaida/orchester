//! Settings › Orchester Cloud (`05 · Orchester Cloud` `vPV73` in the new `orchester.pen`, its states on `ZUuMT`): one
//! column with the cloud account, this computer, the invites, the team servers and the advanced cloud address
//! ([`cloud`]); a team server opens its page under the tab (`05c` `C6qnY`, [`server`]) and *Connect a server* its
//! steps (`05b` `MrVql`, `05b2` `IBnMu`, [`connect`]), each with its crumb back. Member actions only: everything
//! administrative is in a server's admin in the browser, which its page links to.
//!
//! What the tab shows is read again whenever a server's connection or the cloud account changes; the invites are read
//! with it while an account is signed in.

mod accepted;
mod account;
mod advanced;
mod answers;
mod cloud;
mod code;
pub mod connect;
mod ended;
mod invites;
pub mod landing;
mod list;
mod notice;
mod parts;
mod person;
mod pointed;
mod reading;
mod server;
mod server_facts;
mod server_parts;
mod server_projects;
mod two_factor;
mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_api::cloud::CloudInvite;
use orchester_core::id::ServerId;
use orchester_store::StoredCloudAccount;
use orchester_workspace::{
  background,
  sharing::{
    LinkedProject, ServerEntry, Servers,
    cloud::{Cloud, CloudError, CloudInUse},
  },
};

pub use self::{
  notice::InvitesNotice,
  person::PersonOnly,
  words::{Said, cloud_error as cloud_said, day as day_of, moment, runs, sharing_error as sharing_said, shown_address},
};

/// The cloud account as read, for a future's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account(pub Option<StoredCloudAccount>);

impl DevtoolsInspectable for Account {}
use super::frame;
use crate::{
  routes::{OrchesterAsk, OrchesterPanel, OrchesterView},
  services::Services,
  ui::state::load_error,
};

/// What the tab shows: the cloud account, the cloud's address, and the servers with their projects on this computer.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
  pub account: Option<StoredCloudAccount>,
  pub address: CloudInUse,
  pub servers: Vec<ServerEntry>,
  /// The local projects linked to a project on each server, in the servers' order.
  pub linked: Vec<Vec<LinkedProject>>,
}

impl DevtoolsInspectable for Snapshot {}

impl Snapshot {
  pub fn server(&self, id: ServerId) -> Option<(&ServerEntry, &[LinkedProject])> {
    let index = self.servers.iter().position(|entry| entry.stored.id == id)?;
    Some((&self.servers[index], &self.linked[index]))
  }
}

/// The invites waiting and the account's two-factor authentication, as the cloud answered: what the All projects notice
/// shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Waiting {
  pub invites: Invites,
  pub two_factor: Option<orchester_api::cloud::TwoFactorStatus>,
}

/// The invites waiting for the account, as the cloud answered.
#[derive(Clone, Debug)]
pub enum Invites {
  /// Nobody is signed in to the cloud.
  SignedOut,
  Listed(Arc<Vec<CloudInvite>>),
  Failed(Said),
}

impl PartialEq for Invites {
  /// Lists are the same when they hold the same invites, by id.
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::SignedOut, Self::SignedOut) => true,
      (Self::Listed(one), Self::Listed(two)) => {
        one.len() == two.len()
          && one
            .iter()
            .zip(two.iter())
            .all(|(one, two)| one.id == two.id && one.needs_two_factor == two.needs_two_factor)
      }
      (Self::Failed(one), Self::Failed(two)) => one == two,
      _ => false,
    }
  }
}

impl DevtoolsInspectable for Invites {}

impl Invites {
  pub fn waiting(&self) -> usize {
    match self {
      Self::Listed(invites) => invites.len(),
      _ => 0,
    }
  }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Props {
  pub view: OrchesterView,
}

impl DevtoolsInspectable for Props {}

/// Settings › Orchester Cloud: its column, a team server's page or *Connect a server*.
pub struct OrchesterTab {
  /// Counts *Try again* after the tab could not be read.
  retry: Signal<u64>,
}

impl Component for OrchesterTab {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self { retry: ctx.signal(0) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let view = ctx.props::<Props>().view;
    if view.panel == OrchesterPanel::Connect {
      let page = ctx.mount_keyed::<connect::ConnectPage>(
        "settings-orchester-connect",
        connect::Props {
          server: view.server,
          step: view.step,
        },
      );
      return frame::column("orchester-connect-scroll", page);
    }
    let services = Services::of(ctx);
    let revision = follow(ctx, (&services.servers, &services.cloud)) + self.retry.get();
    let loaded = snapshot(ctx, revision, (&services.servers, &services.cloud));
    let snapshot = match loaded {
      Ok(Some(snapshot)) => snapshot,
      Ok(None) => {
        let bones = reading::column(ctx);
        return frame::column("orchester-scroll", frame::pane("orchester-tab").child(bones));
      }
      Err(reason) => {
        let (text, retry) = (reason.text(ctx), self.retry.clone());
        let again = move || retry.update(|count| *count += 1);
        let failed = load_error(ctx, "orchester-load-error", (&ctx.t("cloud.loadFailed"), &text), again);
        return frame::column("orchester-scroll", frame::pane("orchester-tab").child(failed));
      }
    };
    let invites = invites(ctx, revision, &services.cloud, snapshot.account.as_ref());
    let signed_in = snapshot.account.as_ref().map(|account| account.account);
    let two_factor = two_factor::read(ctx, revision, &services.cloud, signed_in);
    let accepted = kept(ctx, revision, &services.cloud, snapshot.account.as_ref());
    match view.server.and_then(|id| snapshot.server(id)) {
      Some((entry, linked)) => {
        let page = ctx.mount_keyed::<server::ServerPanel>(
          &format!("orchester-server-{}", entry.stored.id),
          server::Props {
            entry: entry.clone(),
            linked: linked.to_vec(),
            revision,
            sign_out: view.ask == Some(OrchesterAsk::SignOut),
          },
        );
        frame::column("orchester-server-scroll", page)
      }
      None => {
        let page = ctx.mount_keyed::<cloud::CloudPanel>(
          "orchester-cloud",
          cloud::Props {
            snapshot: snapshot.clone(),
            invites,
            accepted,
            two_factor,
            ask: view.ask,
            at: view.at,
          },
        );
        frame::column("orchester-scroll", page)
      }
    }
  }
}

/// Counts every change of the servers' connections and of the cloud account, so what reads them reads again.
pub fn follow(ctx: &mut Ctx, (servers, cloud): (&Servers, &Cloud)) -> u64 {
  let stream = ctx.stream(
    (servers.clone(), cloud.clone()),
    |(servers, cloud), emitter: StreamEmitter<u64, ()>| async move {
      let (mut servers, mut cloud) = (servers.subscribe(), cloud.subscribe());
      let mut count = 0_u64;
      loop {
        if !emitter.emit(count) {
          return;
        }
        tokio::select! {
          changed = servers.changed() => if changed.is_err() { return },
          changed = cloud.changed() => if changed.is_err() { return },
        }
        count += 1;
      }
    },
  );
  stream.state().get().data.unwrap_or_default()
}

/// Counts changes of the shared projects' caches (a role, a removal, a sync), for a screen that shows how a project
/// stands with its server.
pub fn follow_shared(ctx: &mut Ctx, servers: &Servers) -> u64 {
  let stream = ctx.stream(servers.clone(), |servers, emitter: StreamEmitter<u64, ()>| async move {
    let mut shared = servers.subscribe_shared();
    let mut count = 0_u64;
    loop {
      if !emitter.emit(count) || shared.changed().await.is_err() {
        return;
      }
      count += 1;
    }
  });
  stream.state().get().data.unwrap_or_default()
}

/// What the tab shows, read off the UI thread at `revision`; `None` until the first read finished.
fn snapshot(ctx: &mut Ctx, revision: u64, (servers, cloud): (&Servers, &Cloud)) -> Result<Option<Snapshot>, Said> {
  let hold = Services::of(ctx).hold;
  let state = ctx
    .future(
      (revision, servers.clone(), cloud.clone()),
      move |(_, servers, cloud)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || read(&servers, &cloud))
            .await
            .map_err(|_| Said::new("cloud.errors.local"))?
        }
      },
    )
    .state()
    .get();
  match (state.data, state.error) {
    (Some(snapshot), _) => Ok(Some(snapshot)),
    (None, Some(error)) => Err(error),
    (None, None) => Ok(None),
  }
}

fn read(servers: &Servers, cloud: &Cloud) -> Result<Snapshot, Said> {
  let failed = |error: CloudError| words::cloud_error(&error);
  let account = cloud.account().map_err(failed)?;
  let address = cloud.address().map_err(failed)?;
  let entries = servers.entries().map_err(|error| words::sharing_error(&error))?;
  let linked = entries
    .iter()
    .map(|entry| servers.linked_projects(entry.stored.id))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| words::sharing_error(&error))?;
  Ok(Snapshot {
    account,
    address,
    servers: entries,
    linked,
  })
}

/// The invites waiting for `account`, read at `revision`.
pub fn invites(ctx: &mut Ctx, revision: u64, cloud: &Cloud, account: Option<&StoredCloudAccount>) -> Invites {
  let signed_in = account.map(|account| account.account);
  let state = ctx
    .future(
      (revision, signed_in, cloud.clone()),
      |(_, signed_in, cloud)| async move {
        if signed_in.is_none() {
          return Ok::<_, ()>(Invites::SignedOut);
        }
        Ok(match cloud.invites().await {
          Ok(invites) => Invites::Listed(Arc::new(invites)),
          Err(error) => {
            tracing::warn!(%error, "the cloud's invites could not be read");
            Invites::Failed(words::cloud_error(&error))
          }
        })
      },
    )
    .state()
    .get();
  state.data.unwrap_or(Invites::SignedOut)
}

/// The invites accepted here whose join did not finish (N1 security review 1, M2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept(pub Arc<Vec<orchester_store::AcceptedInvite>>);

impl DevtoolsInspectable for Kept {}

/// The invites accepted here by `account`, read at `revision`.
fn kept(ctx: &mut Ctx, revision: u64, cloud: &Cloud, account: Option<&StoredCloudAccount>) -> Kept {
  let signed_in = account.map(|account| account.account);
  ctx
    .future(
      (revision, signed_in, cloud.clone()),
      |(_, signed_in, cloud)| async move {
        if signed_in.is_none() {
          return Ok::<_, ()>(Kept::default());
        }
        let kept = cloud
          .accepted_invites()
          .await
          .map_err(|error| tracing::warn!(%error, "the invites accepted here could not be read"))?;
        Ok(Kept(Arc::new(kept)))
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}
