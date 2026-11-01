//! Why a sign-in ended, over the sign-in (`CBmxe`): the notice names the reason the cloud gave, or that the person
//! signed out here. The frame draws the revoked reason; the others are worded like it in the same notice, and a reason
//! this app does not know shows the fallback. Signed out to sign in again with a code, the notice says so and names the
//! invite that needs it; a code step that ended says so over the password.

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_store::{CloudSignInEnd, EndedCloudSignIn};
use orchester_workspace::display;

use crate::ui::{
  callout::{Callout, CalloutTone},
  icon::Icon,
};

/// *Sign-in timed out*: a code step ended before a code finished it, and the password is asked again.
pub(super) fn timed_out(ctx: &Ctx) -> Element {
  Callout::new(
    "sign-in-timed-out",
    CalloutTone::Warning,
    ctx.t("sign_in.code.timedOut.title"),
  )
  .icon(Icon::TimerOff)
  .body(ctx.t("sign_in.code.timedOut.body"))
  .into()
}

/// *Sign in with your code to accept invites*, naming the invite when it is known.
fn for_code(ctx: &Ctx, ended: &EndedCloudSignIn) -> Element {
  let body = match &ended.invite {
    Some(invite) => ctx.t_args("sign_in.ended.twoFactor.body", [("invite", invite.as_str())]),
    None => ctx.t("sign_in.ended.twoFactor.bodyNoInvite"),
  };
  let notice: Element = Callout::new(
    "sign-in-notice",
    CalloutTone::Info,
    ctx.t("sign_in.ended.twoFactor.title"),
  )
  .icon(Icon::ShieldCheck)
  .body(body)
  .into();
  shown(notice, "two-factor")
}

/// Why the last sign-in from here ended.
pub(super) fn notice(ctx: &Ctx, ended: &EndedCloudSignIn) -> Element {
  let (glyph, name, (title, body)) = match ended.reason {
    CloudSignInEnd::Revoked => (
      Icon::LogOut,
      "revoked",
      ("sign_in.ended.revoked.title", "sign_in.ended.revoked.body"),
    ),
    CloudSignInEnd::PasswordReset => (
      Icon::KeyRound,
      "password-reset",
      ("sign_in.ended.passwordReset.title", "sign_in.ended.passwordReset.body"),
    ),
    CloudSignInEnd::AccountDeleted => (
      Icon::UserX,
      "account-deleted",
      (
        "sign_in.ended.accountDeleted.title",
        "sign_in.ended.accountDeleted.body",
      ),
    ),
    CloudSignInEnd::SignedOut => (
      Icon::LogOut,
      "signed-out",
      ("sign_in.ended.signedOut.title", "sign_in.ended.signedOut.body"),
    ),
    CloudSignInEnd::TwoFactor => return for_code(ctx, ended),
    CloudSignInEnd::Other => (
      Icon::CircleAlert,
      "other",
      ("sign_in.ended.other.title", "sign_in.ended.other.body"),
    ),
  };
  let time = display::clock(ended.ended_at);
  let body = ctx.t_args(body, [("time", time.as_str()), ("email", ended.email.as_str())]);
  let notice: Element = Callout::new("sign-in-notice", CalloutTone::Neutral, ctx.t(title))
    .icon(glyph)
    .body(body)
    .into();
  shown(notice, name)
}

fn shown(notice: Element, reason: &'static str) -> Element {
  Column::new()
    .id("sign-in-ended")
    .width(Dimension::Pct(100.0))
    .describe("reason", reason)
    .child(notice)
    .into()
}
