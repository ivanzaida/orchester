//! Two-factor authentication in Settings › Orchester Cloud and the All projects notice (`Account card · two-factor
//! off` `AstDs` and `Accept needs two-factor` `g0GfK` in `ZUuMT`). It is turned on and off on the cloud's security
//! page, never here: the account card says whether it is on, with *Turn on…* ↗ while it is off. Accepting an invite
//! needs it on the account and a sign-in with a code on this computer: an invite the cloud says needs it, or whose
//! accept it refused, says which one is missing and offers the one step that fixes it, *Turn on two-factor
//! authentication* ↗ or *Sign in again*, which signs out and locks so the person signs in at the gate with the code
//! (the designer's decision, `Nq3Mf`). After a sign-in with a recovery code that leaves few, a warning says to make new
//! ones; no frame draws it, so it is the UI kit's warning callout.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::cloud::{CloudInvite, TwoFactorStatus};
use orchester_core::id::InviteId;
use orchester_store::StoredCloudAccount;
use orchester_workspace::sharing::cloud::{Cloud, FEW_RECOVERY_CODES, WebPage};

use super::{
  invites::{Answers, Asked},
  words::counted,
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::{Icon, icon},
    notice::NoticeTone,
    setting_notice::SettingNotice,
    setting_rows::status as status_dot,
    unavailable::reason_line,
  },
};

/// The account's two-factor authentication as the cloud reported it, for a future's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoFactor(pub Option<TwoFactorStatus>);

impl DevtoolsInspectable for TwoFactor {}

/// The two-factor authentication of `account`, read at `revision` (`GET /me`) and again whenever the window gains or
/// loses the focus, so turning it on in the browser shows on coming back; `None` until it is read, or when the cloud
/// cannot say (logged).
pub fn read(
  ctx: &mut Ctx,
  revision: u64,
  cloud: &Cloud,
  account: Option<&StoredCloudAccount>,
) -> Option<TwoFactorStatus> {
  let signed_in = account.map(|account| account.account);
  let focused = ctx.window().info().is_focused;
  ctx
    .future(
      (revision, focused, signed_in, cloud.clone()),
      |(_, _, signed_in, cloud)| async move {
        if signed_in.is_none() {
          return Ok::<_, ()>(TwoFactor(None));
        }
        Ok(TwoFactor(cloud.two_factor().await.map_or_else(
          |error| {
            tracing::warn!(%error, "the account's two-factor authentication could not be read");
            None
          },
          Some,
        )))
      },
    )
    .state()
    .get()
    .data
    .and_then(|read| read.0)
}

/// What an invite needs before it can be accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Need {
  Accept,
  /// The account has two-factor authentication off: *Turn on two-factor authentication* (`hC11x`).
  TurnOn,
  /// This computer signed in without a code: *Sign in again* in *Accept*'s place (`qwDH9` cell 3).
  SignInAgain,
  /// The cloud refused the accept for that: *Sign in again to accept* with *Not now* (`rPG8h`).
  Refused,
}

/// What the invite `invite` needs, by the account's two-factor authentication `status` as last read, the person's last
/// accept, and the cloud's word on it (`listed`: `needs_two_factor`). The status wins over an earlier refusal, so
/// turning it on in the browser, or signing in with a code, shows as soon as it is read again.
pub(super) fn need((invite, listed): (InviteId, bool), status: Option<&TwoFactorStatus>, answers: &Answers) -> Need {
  let put_off = answers.not_now.get().0.contains(&invite);
  let refusal = answers.two_factor_refusal(invite);
  match status {
    Some(status) if !status.enabled => Need::TurnOn,
    Some(status) if status.this_session => Need::Accept,
    Some(_) if refusal == Some(false) && !put_off => Need::Refused,
    Some(_) => Need::SignInAgain,
    None => match refusal {
      Some(true) => Need::TurnOn,
      Some(false) if put_off => Need::SignInAgain,
      Some(false) => Need::Refused,
      None if put_off || listed => Need::SignInAgain,
      // Unknown: *Accept*, whose refusal says which.
      None => Need::Accept,
    },
  }
}

/// After the account card's name: *Two-factor: on* in `success` with its shield (`2FA` `dkUDr`), or *Two-factor: off*
/// in `warning` with *Turn on…* ↗ (`Account card · two-factor off` `AstDs` in `ZUuMT`).
pub(super) fn header(ctx: &mut Ctx, status: Option<&TwoFactorStatus>, answers: &Answers) -> Vec<Element> {
  let Some(status) = status else {
    return Vec::new();
  };
  if status.enabled {
    let on = Row::new()
      .id("orchester-cloud-two-factor")
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .describe("state", "two-factor-on")
      .child(icon(Icon::ShieldCheck, IconSize::Xs, PaletteColor::Success))
      .child(
        Text::new(&ctx.t("cloud.account.twoFactor.on"))
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::Success)
          .nowrap(),
      );
    return vec![on.into()];
  }
  let off = status_dot(
    "orchester-cloud-two-factor",
    (PaletteColor::Warning, PaletteColor::Warning),
    &ctx.t("cloud.account.twoFactor.off"),
  )
  .describe("state", "two-factor-off");
  let turn_on = ActionButton::new(
    TURN_ON_HEADER,
    ButtonKind::Ghost,
    ctx.t("cloud.account.twoFactor.turnOn"),
  )
  .icon(Icon::ExternalLink)
  .small()
  .on_click(opener(ctx, answers, TURN_ON_HEADER));
  vec![off.into(), turn_on.into()]
}

/// Opens the cloud's security page in the browser when a person pressed `id`; says so in place when it cannot.
/// Synthetic input is refused, and the panel says so under `id`.
fn opener(ctx: &mut Ctx, answers: &Answers, id: &str) -> impl Fn() + Send + Sync + 'static {
  let services = Services::of(ctx);
  let (cloud, links, unopened) = (services.cloud, services.links, answers.unopened.clone());
  let (person, id) = (answers.person.clone(), id.to_owned());
  move || {
    if !person.allows(&id) {
      return;
    }
    let url = match cloud.web_page(WebPage::Security) {
      Ok(url) => url,
      Err(error) => return tracing::warn!(%error, "the cloud's security page has no address"),
    };
    match links.open(&url) {
      Ok(()) => unopened.set(None),
      Err(error) => {
        tracing::warn!(%error, "the cloud's security page could not be opened");
        unopened.set(Some(url.into()));
      }
    }
  }
}

/// What `invite` is to, as the gate names it after *Sign in again*: its project, or its server.
pub(super) fn invite_to(invite: &CloudInvite) -> &str {
  invite
    .project
    .as_ref()
    .map_or(&invite.server.name, |project| &project.name)
}

/// *Turn on…* ↗ in the account header.
pub(super) const TURN_ON_HEADER: &str = "orchester-cloud-two-factor-turn-on";

/// *Turn on two-factor authentication* ↗, the button `<id>-turn-on`.
pub(super) fn turn_on(ctx: &mut Ctx, id: &str, answers: &Answers) -> ActionButton {
  let button = format!("{id}-turn-on");
  let open = opener(ctx, answers, &button);
  ActionButton::new(button, ButtonKind::Primary, ctx.t("cloud.invites.twoFactor.turnOn"))
    .icon(Icon::ExternalLink)
    .on_click(open)
}

/// *Sign in again*: signs out and locks for a sign-in with a code, a person's; `to`: what the invite is to.
pub(super) fn sign_in_again(ctx: &Ctx, id: &str, to: &str, answers: &Answers) -> ActionButton {
  let button = ActionButton::new(
    format!("{id}-sign-in-again"),
    ButtonKind::Primary,
    ctx.t("cloud.invites.twoFactor.signInAgain"),
  )
  .icon(Icon::LogIn);
  if answers.sign_in_again.state().get().is_pending() {
    return button.loading();
  }
  let (answers, to, pressed) = (answers.clone(), to.to_owned(), format!("{id}-sign-in-again"));
  button.on_click(move || answers.sign_in_again(&pressed, &to))
}

/// *Not now*: *Sign in again* takes *Accept*'s place.
pub(super) fn not_now(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> ActionButton {
  let (answers, invite) = (answers.clone(), invite.id);
  ActionButton::new(
    format!("{id}-not-now"),
    ButtonKind::Ghost,
    ctx.t("cloud.invites.twoFactor.notNow"),
  )
  .on_click(move || answers.not_now(invite))
}

/// *Decline*, asked in place.
pub(super) fn decline(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> ActionButton {
  let (declining, invite) = (answers.declining.clone(), invite.id);
  ActionButton::new(
    format!("{id}-decline"),
    ButtonKind::Ghost,
    ctx.t("cloud.invites.decline"),
  )
  .on_click(move || declining.set(Asked(Some(invite))))
}

/// *Sign in again to accept* (`v5lXw`).
pub(super) fn refused(ctx: &Ctx, id: &str) -> Element {
  Callout::new(
    format!("{id}-sign-in-needed"),
    CalloutTone::Warning,
    ctx.t("cloud.invites.twoFactor.refusedTitle"),
  )
  .icon(Icon::ShieldAlert)
  .body(ctx.t("cloud.invites.twoFactor.refusedBody"))
  .into()
}

/// Why `need` keeps the invite from being accepted, on a line of its own.
pub(super) fn reason(ctx: &Ctx, id: &str, need: Need) -> Option<Element> {
  let key = match need {
    Need::TurnOn => "cloud.invites.twoFactor.off",
    Need::SignInAgain => "cloud.invites.twoFactor.session",
    Need::Accept | Need::Refused => return None,
  };
  Some(reason_line(&format!("{id}-two-factor"), &ctx.t(key)).into())
}

/// What an invite's row shows under itself in place of its actions while `need` keeps it from being accepted (`Accept
/// needs two-factor` `g0GfK` in `ZUuMT`): a warning that says why or *Sign in again to accept*, the step that fixes it
/// with *Decline* or *Not now*, and what that step does.
pub(super) fn below(
  ctx: &mut Ctx,
  id: &str,
  invite: &CloudInvite,
  (need, answers): (Need, &Answers),
) -> Option<Element> {
  let notice = SettingNotice::new(format!("{id}-two-factor"), NoticeTone::Warning);
  let (notice, buttons, note) = match need {
    Need::Accept => return None,
    Need::TurnOn => (
      notice.title(ctx.t("cloud.invites.twoFactor.off")),
      (turn_on(ctx, id, answers), decline(ctx, id, invite, answers)),
      Some("cloud.invites.twoFactor.turnOnNote"),
    ),
    Need::SignInAgain => (
      notice.title(ctx.t("cloud.invites.twoFactor.session")),
      (
        sign_in_again(ctx, id, invite_to(invite), answers),
        decline(ctx, id, invite, answers),
      ),
      None,
    ),
    Need::Refused => (
      SettingNotice::new(format!("{id}-sign-in-needed"), NoticeTone::Warning)
        .icon(Icon::ShieldAlert)
        .title(ctx.t("cloud.invites.twoFactor.refusedTitle"))
        .body(ctx.t("cloud.invites.twoFactor.refusedBody")),
      (
        sign_in_again(ctx, id, invite_to(invite), answers),
        not_now(ctx, id, invite, answers),
      ),
      Some("cloud.invites.twoFactor.signInNote"),
    ),
  };
  let notice = notice.action(buttons.0).action(buttons.1);
  let notice = match note {
    Some(key) => notice.note(ctx.t(key)),
    None => notice,
  };
  let reasons = answers
    .person
    .reason(ctx, &format!("{id}-sign-in-again"))
    .into_iter()
    .chain(answers.person.reason(ctx, &format!("{id}-turn-on")));
  Some(reasons.fold(notice, |notice, reason| notice.part(reason)).into())
}

/// Why *Sign in again* or the security page did not go through, the parts' ids starting with `id`.
pub(super) fn failed(ctx: &Ctx, id: &str, answers: &Answers) -> Vec<Element> {
  let mut parts = Vec::new();
  let state = answers.sign_in_again.state().get();
  if let Some(reason) = state.error.clone().filter(|_| !state.is_pending()) {
    parts.push(field_error(&format!("{id}-sign-in-again-failed"), &reason.text(ctx)).into());
  }
  if let Some(page) = answers.unopened.get() {
    let text = ctx.t_args("cloud.invites.twoFactor.openFailed", [("page", &*page)]);
    parts.push(field_error(&format!("{id}-open-failed"), &text).into());
  }
  parts
}

/// *2 recovery codes left*, after this session's sign-in with a recovery code left few, with the security page; gone
/// once `status`, read again, says new codes were made.
pub(super) fn recovery_warning(
  ctx: &mut Ctx,
  id: &str,
  (status, answers): (Option<&TwoFactorStatus>, &Answers),
) -> Option<Element> {
  let left = Services::of(ctx).cloud.few_recovery_codes()?;
  let left = status.map_or(left, |status| status.recovery_codes_left);
  if left > FEW_RECOVERY_CODES {
    return None;
  }
  let title = counted(
    ctx,
    ("cloud.recovery.left.one", "cloud.recovery.left.other"),
    usize::try_from(left).unwrap_or(usize::MAX),
  );
  let button = format!("{id}-security");
  let open = opener(ctx, answers, &button);
  let open = ActionButton::new(button.clone(), ButtonKind::Secondary, ctx.t("cloud.recovery.open"))
    .icon(Icon::ExternalLink)
    .on_click(open);
  let warning: Element = Callout::new(id.to_owned(), CalloutTone::Warning, title)
    .icon(Icon::KeyRound)
    .body(ctx.t("cloud.recovery.body"))
    .action(open)
    .into();
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(warning)
      .with_children(answers.person.reason(ctx, &button))
      .into(),
  )
}
