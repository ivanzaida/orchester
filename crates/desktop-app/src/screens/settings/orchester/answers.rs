//! The person's answers to invites (Settings › Orchester › Invites and the All projects notice): accepting one, which
//! joins its server; declining one, asked in place first; and, for an invite accepted here whose join did not finish,
//! asking again (*Try again*, and the check while it waits for approval) or setting it aside. Accepting, declining and
//! trying again are a person's ([`PersonOnly`], N1 security review 1, M1). A server that refuses this computer is an
//! outcome to show (`S5mMY`), not a failure, and so is the cloud's refusal for want of two-factor authentication
//! (`hC11x`, `rPG8h`): the invite then offers *Turn on two-factor authentication* or *Sign in again*, which signs out
//! and locks so the person signs in with a code (also a person's).

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_api::cloud::CloudInvite;
use orchester_core::id::{InviteId, ServerId};
use orchester_store::AcceptedInvite;
use orchester_workspace::sharing::{
  CloudJoined, ProjectAddress,
  cloud::{Accepted, CloudError},
};

use super::{PersonOnly, Said, words::cloud_error};
use crate::{
  navigation::Navigation,
  routes::{JoinTarget, Route},
  services::Services,
};

/// What accepting an invite, or asking again for one, came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
  /// Joined the server, in the list as `id`; the project the invite was to, if any, and where to link its checkout
  /// when this computer knows its address.
  Joined {
    id: ServerId,
    server: String,
    project: Option<String>,
    landing: Option<JoinTarget>,
  },
  /// The server holds the join until another computer of the person's, or an admin, approves it.
  Waiting { server: String },
  /// The server refused this computer: its approval was refused, or the account there is disabled.
  Refused { server: String },
  /// The cloud refused to accept `invite` because the account has two-factor authentication off.
  TwoFactorOff { invite: InviteId },
  /// The cloud refused to accept `invite` because this computer signed in without a code.
  NeedsCode { invite: InviteId },
}

impl DevtoolsInspectable for Outcome {}

impl Outcome {
  /// What asking for `invite`, to `server`, came to.
  pub(super) fn asked(asked: Result<Accepted, CloudError>, (invite, server): (InviteId, &str)) -> Result<Self, Said> {
    Self::asked_for(asked, (invite, server), None)
  }

  /// [`Self::asked`] for an invite to the project at `address`, whose checkout is linked next.
  fn asked_for(
    asked: Result<Accepted, CloudError>,
    (invite, server): (InviteId, &str),
    address: Option<&str>,
  ) -> Result<Self, Said> {
    match asked {
      Ok(accepted) => Ok(Self::of(accepted, address)),
      Err(CloudError::JoinRefused) => Ok(Self::Refused {
        server: server.to_owned(),
      }),
      Err(CloudError::TwoFactorOff) => Ok(Self::TwoFactorOff { invite }),
      Err(CloudError::TwoFactorSession) => Ok(Self::NeedsCode { invite }),
      Err(error) => Err(cloud_error(&error)),
    }
  }

  fn of(accepted: Accepted, address: Option<&str>) -> Self {
    let server = accepted.server.name;
    match accepted.joined {
      CloudJoined::Joined(id) => Self::Joined {
        id,
        server,
        project: accepted.project,
        landing: address
          .and_then(|address| ProjectAddress::parse(address).ok())
          .and_then(|address| JoinTarget::new(id, address.slug())),
      },
      CloudJoined::Waiting(_) => Self::Waiting { server },
    }
  }
}

/// The invite a question or an accept is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Asked(pub Option<InviteId>);

impl DevtoolsInspectable for Asked {}

/// How the joins the check asked for came out, by invite, until the person moves on ([`super::ended`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Ended(pub Vec<(InviteId, Outcome)>);

impl DevtoolsInspectable for Ended {}

/// The invites accepted here that the person put off with *Later*, while this panel is open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Later(pub Vec<InviteId>);

impl DevtoolsInspectable for Later {}

/// The answers of one panel. Clones share them.
#[derive(Clone)]
pub(super) struct Answers {
  pub(super) declining: Signal<Asked>,
  pub(super) accept: FutureAction<CloudInvite, Outcome, Said>,
  pub(super) decline: FutureAction<InviteId, (), Said>,
  /// Asks again for an invite accepted here, by its id and its server's name.
  pub(super) finish: FutureAction<(InviteId, String), Outcome, Said>,
  pub(super) set_aside: FutureAction<InviteId, (), Said>,
  /// The invite the last accept was for.
  pub(super) accepting: Signal<Asked>,
  /// The person pressed *Cancel* while the last accept was connecting.
  pub(super) stopped: Signal<bool>,
  pub(super) later: Signal<Later>,
  pub(super) ended: Signal<Ended>,
  pub(super) person: PersonOnly,
  /// Signs out and locks so the person signs in again with a code, naming what the invite is to.
  pub(super) sign_in_again: FutureAction<Option<String>, (), Said>,
  /// The invites whose *Sign in again to accept* the person put off with *Not now*.
  pub(super) not_now: Signal<Later>,
  /// The cloud's page that could not open in the browser.
  pub(super) unopened: Signal<Option<Arc<str>>>,
}

impl Answers {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let cloud = Services::of(ctx).cloud;
    let answers = Self::made(ctx, cloud);
    // A project invite accepted goes on to link the project's checkout (`xeFRB`).
    let navigation = Navigation::of(ctx);
    ctx.watch(&answers.accept.state(), move |state| {
      if let Some(Outcome::Joined {
        landing: Some(target), ..
      }) = state.data.as_ref().filter(|_| state.is_fulfilled())
      {
        navigation.go(Route::Invite(target.clone()));
      }
    });
    answers
  }

  fn made(ctx: &mut Ctx, cloud: orchester_workspace::sharing::cloud::Cloud) -> Self {
    let (declining, finishing, aside, again) = (cloud.clone(), cloud.clone(), cloud.clone(), cloud.clone());
    Self {
      declining: ctx.signal(Asked(None)),
      accept: ctx.future_action(move |invite: CloudInvite| {
        let cloud = cloud.clone();
        async move {
          let address = invite.project.as_ref().map(|project| project.address.as_str());
          Outcome::asked_for(cloud.accept(&invite).await, (invite.id, &invite.server.name), address)
        }
      }),
      decline: ctx.future_action(move |invite: InviteId| {
        let cloud = declining.clone();
        async move { cloud.decline(invite).await.map_err(|error| cloud_error(&error)) }
      }),
      finish: ctx.future_action(move |(invite, server): (InviteId, String)| {
        let cloud = finishing.clone();
        async move { Outcome::asked(cloud.finish_accept(invite).await, (invite, &server)) }
      }),
      set_aside: ctx.future_action(move |invite: InviteId| {
        let cloud = aside.clone();
        async move { cloud.set_aside(invite).await.map_err(|error| cloud_error(&error)) }
      }),
      accepting: ctx.signal(Asked(None)),
      stopped: ctx.signal(false),
      later: ctx.signal(Later::default()),
      ended: ctx.signal(Ended::default()),
      person: PersonOnly::new(ctx),
      sign_in_again: ctx.future_action(move |invite: Option<String>| {
        let cloud = again.clone();
        async move {
          cloud
            .sign_out_for_code(invite)
            .await
            .map_err(|error| cloud_error(&error))
        }
      }),
      not_now: ctx.signal(Later::default()),
      unopened: ctx.signal(None),
    }
  }

  /// Signs out to sign in again with a code for an invite to `to`, which the gate names, when a person pressed the
  /// button `id`.
  pub(super) fn sign_in_again(&self, id: &str, to: &str) {
    if self.person.allows(id) {
      self.sign_in_again.run(Some(to.to_owned()));
    }
  }

  /// Puts off *Sign in again to accept* for `invite`: *Sign in again* takes *Accept*'s place.
  pub(super) fn not_now(&self, invite: InviteId) {
    self.not_now.update(|later| {
      if !later.0.contains(&invite) {
        later.0.push(invite);
      }
    });
  }

  /// Whether the cloud refused to accept `invite` for want of two-factor authentication, by the last accept or ask
  /// again: `Some(true)` when the account has it off, `Some(false)` when this computer signed in without a code.
  pub(super) fn two_factor_refusal(&self, invite: InviteId) -> Option<bool> {
    [self.accept.state().get(), self.finish.state().get()]
      .into_iter()
      .filter(lurq::app::ctx::FutureState::is_fulfilled)
      .filter_map(|state| state.data)
      .find_map(|outcome| match outcome {
        Outcome::TwoFactorOff { invite: refused } if refused == invite => Some(true),
        Outcome::NeedsCode { invite: refused } if refused == invite => Some(false),
        _ => None,
      })
  }

  /// Whether an accept is connecting and nobody stopped it.
  pub(super) fn connecting(&self) -> bool {
    self.accept.state().get().is_pending() && !self.stopped.get()
  }

  /// Accepts `invite` when a person pressed the button `id`, connecting to its server.
  pub(super) fn accept(&self, id: &str, invite: &CloudInvite) {
    if !self.person.allows(id) {
      return;
    }
    self.accepting.set(Asked(Some(invite.id)));
    self.stopped.set(false);
    self.accept.run(invite.clone());
  }

  /// Declines `invite` when a person pressed the button `id`.
  pub(super) fn decline(&self, id: &str, invite: InviteId) {
    if !self.person.allows(id) {
      return;
    }
    self.declining.set(Asked(None));
    self.decline.run(invite);
  }

  /// Asks again for `invite`, accepted here, when a person pressed the button `id`.
  pub(super) fn try_again(&self, id: &str, invite: &AcceptedInvite) {
    if self.person.allows(id) {
      self.finish.run((invite.invite, invite.server_name.clone()));
    }
  }

  /// Stops waiting for the accept under way: its connection is dropped where it stands. The invite stays accepted
  /// here, to try again.
  pub(super) fn stop(&self) {
    self.accept.cancel();
    self.stopped.set(true);
  }
}
