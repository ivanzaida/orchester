//! The invites accepted here whose join did not finish (N1 security review 1, M2), first in *Waiting invites*. One that
//! waits for approval (`Waiting for approval · on the joining computer` `Bl08B` in `jUMAF`) says who approves it, that
//! nothing was uploaded and that it finishes on its own, with *Cancel joining*; while Settings › Orchester shows, it is
//! asked again on the workspace's backoff ([`check`]; 2, 4 and 8 minutes, not after its approval expires). One whose
//! join failed, or that stopped being asked on its own, says so as `Join stopped` (`DheFE`, the nearest frame) does,
//! with *Try again*, which only a person presses, and *Later*; one asked for as often as Orchester does offers
//! *Cancel joining* instead of *Try again* (N1 security re-review 2).

use std::time::Duration;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_api::cloud::TwoFactorStatus;
use orchester_core::time::Timestamp;
use orchester_store::AcceptedInvite;
use orchester_workspace::sharing::cloud::{Cloud, MAX_ASKS, may_ask, waits};

use super::{
  Kept,
  answers::{Answers, Outcome},
  two_factor::{self, Need},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::Icon,
  },
};

/// How often the invites accepted here are looked at for an ask that is due ([`Cloud::check_accepted`]); only a due
/// ask takes an identity token.
pub(super) const CHECK_EVERY: Duration = Duration::from_secs(30);

/// The kept invites the person did not put off with *Later*, for an account whose two-factor authentication is
/// `status`.
pub(super) fn kept(
  ctx: &mut Ctx,
  (kept, status): (&Kept, Option<&TwoFactorStatus>),
  answers: &Answers,
) -> Vec<Element> {
  let (later, now) = (answers.later.get(), Timestamp::now());
  kept
    .0
    .iter()
    .filter(|invite| !later.0.contains(&invite.invite))
    .map(|invite| {
      if waits(invite, now) {
        return waiting(ctx, invite, answers);
      }
      match two_factor::need((invite.invite, false), status, answers) {
        Need::Accept => stopped(ctx, invite, answers),
        need => needs_two_factor(ctx, invite, (need, answers)),
      }
    })
    .collect()
}

/// A kept invite whose join the cloud refuses for want of two-factor authentication (its earlier ask took a token, so
/// the cloud lists it no more): why, with *Turn on two-factor authentication* or *Sign in again*, and *Later*. After a
/// sign-in with a code it offers *Try again*.
fn needs_two_factor(ctx: &mut Ctx, invite: &AcceptedInvite, (need, answers): (Need, &Answers)) -> Element {
  let id = format!("orchester-accepted-{}", invite.invite);
  let server = invite.server_name.as_str();
  let to = invite.project_name.as_deref().unwrap_or(server);
  let (later, kept) = (answers.later.clone(), invite.invite);
  let put_off = ActionButton::new(
    format!("{id}-later"),
    ButtonKind::Ghost,
    ctx.t("cloud.invites.joinStopped.later"),
  )
  .on_click(move || later.update(|later| later.0.push(kept)));
  let (body, note, first) = if need == Need::TurnOn {
    (
      "cloud.invites.twoFactor.off",
      "cloud.invites.twoFactor.keptTurnOnNote",
      two_factor::turn_on(ctx, &id, answers),
    )
  } else {
    (
      "cloud.invites.twoFactor.session",
      "cloud.invites.twoFactor.signInNote",
      two_factor::sign_in_again(ctx, &id, to, answers),
    )
  };
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Section.role())
    .child(
      Callout::new(
        format!("{id}-stopped"),
        CalloutTone::Warning,
        ctx.t_args("cloud.invites.joinStopped.title", [("server", server)]),
      )
      .icon(Icon::ShieldAlert)
      .body(ctx.t(body))
      .detail(actions(first, Some(put_off.into())))
      .detail(
        Text::new(&ctx.t(note))
          .id(format!("{id}-two-factor-note"))
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      ),
    )
    .with_children(answers.person.reason(ctx, &format!("{id}-sign-in-again")))
    .with_children(answers.person.reason(ctx, &format!("{id}-turn-on")))
    .into()
}

/// *Waiting for approval*, an information notice (`Notice · info` `ktYJP` in `at7XI`): who approves, *Cancel joining*,
/// and under it that nothing was uploaded.
fn waiting(ctx: &Ctx, invite: &AcceptedInvite, answers: &Answers) -> Element {
  let id = format!("orchester-accepted-{}", invite.invite);
  let server = invite.server_name.as_str();
  let (set_aside, kept) = (answers.set_aside.clone(), invite.invite);
  let cancel = ActionButton::new(
    format!("{id}-cancel"),
    ButtonKind::Ghost,
    ctx.t("cloud.invites.approval.cancel"),
  )
  .on_click(move || set_aside.run(kept));
  Callout::new(id.clone(), CalloutTone::Info, ctx.t("cloud.invites.approval.title"))
    .body(ctx.t_args("cloud.invites.approval.ask", [("server", server)]))
    .detail(actions(cancel, None))
    .detail(
      Text::new(&ctx.t("cloud.invites.approval.later"))
        .id(format!("{id}-later-note"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// A notice's buttons as parts of it, so that a note can follow them (`Actions` `ppr9A` then `Note` `uxLaj`); a
/// callout's own actions come last.
fn actions(first: impl Into<Element>, second: Option<Element>) -> Row {
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .padding(Padding::new().top(ExtraSpacing::Snug.role()))
    .child(first)
    .with_children(second)
}

/// *Joining Kontur team didn't finish*, why, and *Try again* (or *Cancel joining* once asked for as often as
/// Orchester does) and *Later*, inside the notice (`Notice · warning` `p6biQ` in `at7XI`).
fn stopped(ctx: &Ctx, invite: &AcceptedInvite, answers: &Answers) -> Element {
  let id = format!("orchester-accepted-{}", invite.invite);
  let server = invite.server_name.as_str();
  let again = format!("{id}-try-again");
  let (later, kept) = (answers.later.clone(), invite.invite);
  let put_off = ActionButton::new(
    format!("{id}-later"),
    ButtonKind::Ghost,
    ctx.t("cloud.invites.joinStopped.later"),
  )
  .on_click(move || later.update(|later| later.0.push(kept)));
  let (glyph, tone) = if invite.needs_code {
    (Icon::ShieldCheck, CalloutTone::Neutral)
  } else {
    (Icon::WifiOff, CalloutTone::Warning)
  };
  let body = if !may_ask(invite) {
    ctx.t_args(
      "cloud.invites.joinStopped.tooMany",
      [("server", server.to_owned()), ("count", MAX_ASKS.to_string())],
    )
  } else if invite.needs_code {
    ctx.t_args("cloud.invites.joinStopped.signedInWithCode", [("server", server)])
  } else if invite.approval.is_some() {
    ctx.t_args("cloud.invites.joinStopped.unanswered", [("server", server)])
  } else {
    ctx.t_args("cloud.invites.joinStopped.body", [("server", server)])
  };
  let first = if may_ask(invite) {
    try_again(ctx, &again, invite, answers)
  } else {
    let set_aside = answers.set_aside.clone();
    ActionButton::new(
      format!("{id}-cancel"),
      ButtonKind::Secondary,
      ctx.t("cloud.invites.approval.cancel"),
    )
    .on_click(move || set_aside.run(kept))
  };
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Section.role())
    .child(
      Callout::new(
        format!("{id}-stopped"),
        tone,
        ctx.t_args("cloud.invites.joinStopped.title", [("server", server)]),
      )
      .icon(glyph)
      .body(body)
      .action(first)
      .action(put_off),
    )
    .with_children(answers.person.reason(ctx, &again))
    .into()
}

/// *Try again*, the button `id`, which only a person presses.
fn try_again(ctx: &Ctx, id: &str, invite: &AcceptedInvite, answers: &Answers) -> ActionButton {
  let button = ActionButton::new(
    id.to_owned(),
    ButtonKind::Primary,
    ctx.t("cloud.invites.joinStopped.tryAgain"),
  )
  .icon(Icon::RefreshCw);
  if answers.finish.state().get().is_pending() {
    button.loading()
  } else {
    let (answers, kept, id) = (answers.clone(), invite.clone(), id.to_owned());
    button.on_click(move || answers.try_again(&id, &kept))
  }
}

/// What [`check`] came to; shown through [`Answers::ended`], and the kept invites are read again when one changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Checked;

impl DevtoolsInspectable for Checked {}

/// Asks again, at `tick`, for each invite accepted here whose ask on its own is due: approved, it joins, and the panel
/// says so; refused, it says that.
pub(super) fn check(ctx: &mut Ctx, tick: u64, (cloud, answers): (&Cloud, &Answers)) {
  let ended = answers.ended.clone();
  ctx.future((tick, cloud.clone()), move |(_, cloud)| {
    let ended = ended.clone();
    async move {
      let checked = cloud
        .check_accepted(Timestamp::now())
        .await
        .map_err(|error| tracing::warn!(%error, "the invites accepted here could not be checked"))?;
      for one in checked {
        match Outcome::asked(one.outcome, (one.invite, &one.server_name)) {
          // A refusal for want of two-factor authentication leaves the invite listed, which says what is needed.
          Ok(Outcome::Waiting { .. } | Outcome::TwoFactorOff { .. } | Outcome::NeedsCode { .. }) => {}
          Ok(outcome) => ended.update(|ended| ended.0.push((one.invite, outcome))),
          Err(reason) => tracing::info!(reason = reason.key, "a join that waits for approval did not finish yet"),
        }
      }
      Ok::<_, ()>(Checked)
    }
  });
}
