//! The tab's column (`05 · Orchester Cloud` `vPV73`): the account card with its two-factor authentication
//! ([`super::two_factor`]) and *Sign out* asked in place ([`super::account`]); few recovery codes left, after a sign-in
//! with one; *This computer*; the invites ([`super::invites`]); the team servers ([`super::list`]); *Advanced* with the
//! cloud's address ([`super::advanced`]); and where the cloud token is kept. Signed out, the app shows only the gate
//! (`crate::screens::gate`), so the column always has an account.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, Interval},
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_api::cloud::TwoFactorStatus;

use super::{
  Invites, Said, Snapshot, account,
  advanced::{self, Place},
  invites, list,
  words::cloud_error,
};
use crate::{
  screens::settings::frame,
  services::Services,
  ui::{field::field_error, icon::Icon, setting_notice::status_line},
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub snapshot: Snapshot,
  pub invites: Invites,
  /// The invites accepted here whose join did not finish.
  pub accepted: super::Kept,
  /// The account's two-factor authentication, once the cloud said.
  pub two_factor: Option<TwoFactorStatus>,
}

impl DevtoolsInspectable for Props {}

pub(super) struct CloudPanel {
  advanced: advanced::Advanced,
  invites: invites::Answers,
  pub(super) sign_out: FutureAction<(), (), Said>,
  /// *Sign out* asks before it signs out and locks.
  pub(super) asking: Signal<bool>,
  /// Signing this computer out of the cloud is a person's (review 1, M1).
  pub(super) person: super::PersonOnly,
  /// Counts the checks of joins that wait for approval ([`super::accepted::check`]).
  tick: Signal<u64>,
  checks: Interval,
}

impl Component for CloudPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let cloud = Services::of(ctx).cloud;
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let checks = ctx.create_interval(super::accepted::CHECK_EVERY, move || next.update(|tick| *tick += 1));
    Self {
      advanced: advanced::Advanced::new(ctx),
      invites: invites::Answers::new(ctx),
      sign_out: ctx.future_action(move |(): ()| {
        let cloud = cloud.clone();
        async move { cloud.sign_out().await.map_err(|error| cloud_error(&error)) }
      }),
      asking: ctx.signal(false),
      person: super::PersonOnly::new(ctx),
      tick,
      checks,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.checks.start();
    let pane = frame::pane("orchester-cloud-details");
    let snapshot = &props.snapshot;
    // Only while the window turns to the sign-in gate after a sign-out.
    let Some(account) = &snapshot.account else {
      return pane.child(status_line(
        "orchester-cloud-reading",
        Icon::LoaderCircle,
        &ctx.t("cloud.reading"),
      ));
    };
    let cloud = Services::of(ctx).cloud;
    super::accepted::check(ctx, self.tick.get(), (&cloud, &self.invites));
    let after = super::two_factor::header(ctx, props.two_factor.as_ref(), &self.invites);
    let card = account::card(ctx, self, (account, &snapshot.address), after);
    let question = self.asking.get().then(|| account::sign_out_question(ctx, self));
    let failed = self
      .sign_out
      .state()
      .get()
      .error
      .map(|reason| field_error("orchester-cloud-sign-out-error", &reason.text(ctx)));
    let recovery = super::two_factor::recovery_warning(
      ctx,
      "orchester-cloud-recovery",
      (props.two_factor.as_ref(), &self.invites),
    );
    let turn_on = self.invites.person.reason(ctx, super::two_factor::TURN_ON_HEADER);
    let computer = account::this_computer(ctx, account);
    let invites = invites::waiting(
      ctx,
      (&props.invites, &props.accepted, props.two_factor.as_ref()),
      &self.invites,
    );
    let servers = list::group(ctx, snapshot);
    let advanced = advanced::section(ctx, &snapshot.address, &self.advanced, Place::Settings);
    let top = Column::new()
      .width(lurq::node::dimension::Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Sm)
      .child(card)
      .with_children(question)
      .with_children(failed)
      .with_children(recovery)
      .with_children(turn_on);
    pane
      .child(top)
      .child(computer)
      .child(invites)
      .child(servers)
      .child(advanced)
      .child(account::token_note(ctx))
  }
}

/// Where this platform keeps the cloud token, as the note under the account says it.
#[cfg(target_os = "macos")]
pub(super) const fn token_note() -> &'static str {
  "cloud.account.tokenNote.macos"
}

#[cfg(not(target_os = "macos"))]
pub(super) const fn token_note() -> &'static str {
  "cloud.account.tokenNote.windows"
}
