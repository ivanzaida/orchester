//! The invite landing while accepting the invite needs a sign-in first (board *Signed out · redesign* `uBND2`), in
//! place of the checkouts and with the invite as this computer knows it: with nobody signed in, the sign-in itself
//! (`zoihT`); signed in without a code to an account with two-factor authentication on, *Sign in again to accept*
//! (`d1yW2I`), whose *Sign in again* signs out and leaves this page where it is, so the sign-in with the code opens
//! here.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  super::{answers::Answers, two_factor},
  rows,
};
use crate::{
  navigation::Navigation,
  routes::{JoinTarget, OrchesterView, Route},
  screens::{
    cloud_sign_in::{Place, SignInOffer},
    settings::frame,
  },
  theme::{ExtraSpacing, SettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    notice::NoticeTone,
    setting_notice::SettingNotice,
  },
};

/// The id of *Sign in again to accept*, and the start of its buttons' ids.
const AGAIN: &str = "orchester-landing-sign-in-again";

/// The landing while nobody is signed in (`zoihT`): the sign-in under the invite to `target`, on `server` when this
/// computer knows its name.
pub(super) fn signed_out(ctx: &mut Ctx, (target, server): (&JoinTarget, Option<&str>)) -> Element {
  let lead = ctx.t("sharing.landing.signInLead");
  let sign_in = ctx.mount_keyed::<SignInOffer>("orchester-landing-sign-in", Place::Card);
  page(ctx, (target, server), (&lead, sign_in))
}

/// The landing signed in as `email` without a code (`d1yW2I`): *Sign in again to accept*, with *Sign in again*, *Not
/// now*, which goes back to Settings › Orchester Cloud where the invite waits, and what signing in again leaves as it
/// is.
pub(super) fn again(
  ctx: &mut Ctx,
  (target, server): (&JoinTarget, Option<&str>),
  (email, answers): (&str, &Answers),
) -> Element {
  let lead = ctx.t_args("sharing.landing.signInAgainLead", [("email", email)]);
  let navigation = Navigation::of(ctx);
  let not_now = ActionButton::new(
    format!("{AGAIN}-not-now"),
    ButtonKind::Ghost,
    ctx.t("cloud.invites.twoFactor.notNow"),
  )
  .opens(navigation.to(Route::Orchester(OrchesterView::default())));
  let reasons: Vec<Element> = answers
    .person
    .reason(ctx, &format!("{AGAIN}-sign-in-again"))
    .into_iter()
    .chain(two_factor::failed(ctx, AGAIN, answers))
    .collect();
  let notice = SettingNotice::new(AGAIN, NoticeTone::Warning)
    .title(ctx.t("cloud.invites.twoFactor.refusedTitle"))
    .body(ctx.t("cloud.invites.twoFactor.refusedBody"))
    .action(two_factor::sign_in_again(ctx, AGAIN, target.slug(), answers))
    .action(not_now)
    .note(ctx.t("cloud.invites.twoFactor.signInNote"));
  let notice = reasons.into_iter().fold(notice, SettingNotice::part);
  page(ctx, (target, server), (&lead, notice.into()))
}

/// The crumb back to Orchester Cloud, then one card: *Join Kontur on Kontur team* (or *Join Kontur* while the server
/// is not in this computer's list), `lead`, the invite's facts, and `below`.
fn page(ctx: &mut Ctx, (target, server): (&JoinTarget, Option<&str>), (lead, below): (&str, Element)) -> Element {
  let navigation = Navigation::of(ctx);
  let title = match server {
    Some(server) => ctx.t_args(
      "sharing.landing.title",
      [("project", target.slug()), ("server", server)],
    ),
    None => ctx.t_args("sharing.landing.titlePending", [("project", target.slug())]),
  };
  let facts = rows::known_facts(ctx, target.slug(), server);
  let card = Column::new()
    .id("orchester-landing-signed-out")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Loose.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(
      Text::new(&title)
        .id("orchester-landing-signed-out-title")
        .variant(SettingsText::SubTitle.role()),
    )
    .child(
      Text::new(lead)
        .id("orchester-landing-signed-out-lead")
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
    .child(facts)
    .child(below);
  let pane = frame::pane("orchester-landing-page")
    .child(frame::crumb(
      "orchester-landing-crumb",
      &ctx.t("settings.tabs.orchester"),
      navigation.to(Route::Orchester(OrchesterView::default())),
    ))
    .child(card);
  frame::column("orchester-landing-scroll", pane)
}
