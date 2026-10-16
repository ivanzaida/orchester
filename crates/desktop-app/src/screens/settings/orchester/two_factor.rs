//! Two-factor authentication in Settings › Orchester and the All projects notice (layer `uyO5v`: `hC11x`, `rPG8h` and
//! board `qwDH9`). It is turned on and off on the cloud's security page, never here: the account header says whether it
//! is on, with *Turn on…* ↗ while it is off. Accepting an invite needs it on the account and a sign-in with a code on
//! this computer: an invite the cloud says needs it, or whose accept it refused, says which one is missing and offers
//! the one step that fixes it, *Turn on two-factor authentication* ↗ or *Sign in again*, which signs out and locks so
//! the person signs in at the gate with the code (the designer's decision, `Nq3Mf`). After a sign-in with a recovery
//! code that leaves few, a warning says to make new ones; no frame draws it, so it is the UI kit's warning callout.

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
use orchester_store::StoredCloudAccount;
use orchester_workspace::sharing::cloud::{Cloud, WebPage};

use super::{
  invites::{Answers, Asked},
  list::state_line,
  words::{Look, counted},
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::{Icon, icon},
    pressable::{Focus, pressable},
    unavailable::reason_line,
  },
};

/// An invite card's avatar (`T8devG`): what is under its top line starts past it.
const AVATAR: f32 = 22.0;

/// The account's two-factor authentication as the cloud reported it, for a future's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoFactor(pub Option<TwoFactorStatus>);

impl DevtoolsInspectable for TwoFactor {}

/// The two-factor authentication of `account`, read at `revision` (`GET /me`); `None` until it is read, or when the
/// cloud cannot say (logged).
pub fn read(
  ctx: &mut Ctx,
  revision: u64,
  cloud: &Cloud,
  account: Option<&StoredCloudAccount>,
) -> Option<TwoFactorStatus> {
  let signed_in = account.map(|account| account.account);
  ctx
    .future(
      (revision, signed_in, cloud.clone()),
      |(_, signed_in, cloud)| async move {
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

/// What `invite` needs, by the cloud's word on it, the account's two-factor authentication `status`, and the person's
/// last accept.
pub(super) fn need(invite: &CloudInvite, status: Option<&TwoFactorStatus>, answers: &Answers) -> Need {
  let put_off = answers.not_now.get().0.contains(&invite.id);
  match answers.two_factor_refusal(invite.id) {
    Some(true) => Need::TurnOn,
    Some(false) if put_off => Need::SignInAgain,
    Some(false) => Need::Refused,
    None if put_off => Need::SignInAgain,
    None if invite.needs_two_factor => match status {
      Some(status) if !status.enabled => Need::TurnOn,
      Some(_) => Need::SignInAgain,
      // Unknown: *Accept*, whose refusal says which.
      None => Need::Accept,
    },
    None => Need::Accept,
  }
}

/// *Two-factor: on*, or *Two-factor: off* with *Turn on…* ↗, after the account header's state (`WH6nX`, `vWLuG`).
pub(super) fn header(ctx: &mut Ctx, status: Option<&TwoFactorStatus>, answers: &Answers) -> Vec<Element> {
  let Some(status) = status else {
    return Vec::new();
  };
  let look = if status.enabled {
    Look {
      icon: Icon::ShieldCheck,
      icon_color: PaletteColor::Success,
      key: "cloud.account.twoFactor.on",
      color: PaletteColor::TextSecondary,
      name: "two-factor-on",
    }
  } else {
    Look {
      icon: Icon::ShieldOff,
      icon_color: PaletteColor::TextMuted,
      key: "cloud.account.twoFactor.off",
      color: PaletteColor::TextSecondary,
      name: "two-factor-off",
    }
  };
  let mut parts: Vec<Element> = vec![state_line(ctx, "orchester-cloud-two-factor", &look).into()];
  if !status.enabled {
    let label = ctx.t("cloud.account.twoFactor.turnOn");
    let link = Focus::Link
      .apply(pressable(opener(ctx, answers)))
      .id("orchester-cloud-two-factor-turn-on")
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .describe("a11y_name", label.to_string())
      .child(
        Text::new(&label)
          .variant(ExtraTypography::NoteStrong.role())
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
      .child(icon(Icon::ExternalLink, IconSize::Sm, PaletteColor::TextMuted));
    parts.push(link.into());
  }
  parts
}

/// Opens the cloud's security page in the browser; says so in place when it cannot.
fn opener(ctx: &mut Ctx, answers: &Answers) -> impl Fn() + Send + Sync + 'static {
  let services = Services::of(ctx);
  let (cloud, links, unopened) = (services.cloud, services.links, answers.unopened.clone());
  move || {
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

/// *Turn on two-factor authentication* ↗.
pub(super) fn turn_on(ctx: &mut Ctx, id: &str, answers: &Answers) -> ActionButton {
  ActionButton::new(
    format!("{id}-turn-on"),
    ButtonKind::Primary,
    ctx.t("cloud.invites.twoFactor.turnOn"),
  )
  .icon(Icon::ExternalLink)
  .on_click(opener(ctx, answers))
}

/// *Sign in again*: signs out and locks for a sign-in with a code, a person's.
pub(super) fn sign_in_again(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> ActionButton {
  let button = ActionButton::new(
    format!("{id}-sign-in-again"),
    ButtonKind::Primary,
    ctx.t("cloud.invites.twoFactor.signInAgain"),
  )
  .icon(Icon::LogIn);
  if answers.sign_in_again.state().get().is_pending() {
    return button.loading();
  }
  let (answers, invite, pressed) = (answers.clone(), invite.clone(), format!("{id}-sign-in-again"));
  button.on_click(move || answers.sign_in_again(&pressed, &invite))
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

/// What a settings invite card shows under its top line while `need` keeps it from being accepted (`ZhlIn`, `ns0C8`,
/// `VLERC`): past the avatar, why or *Sign in again to accept*, the step that fixes it with *Decline* or *Not now*, and
/// what that step does.
pub(super) fn below(
  ctx: &mut Ctx,
  id: &str,
  invite: &CloudInvite,
  (need, answers): (Need, &Answers),
) -> Option<Element> {
  let (first, buttons, note) = match need {
    Need::Accept => return None,
    Need::TurnOn => (
      reason(ctx, id, need)?,
      (turn_on(ctx, id, answers), decline(ctx, id, invite, answers)),
      Some("cloud.invites.twoFactor.turnOnNote"),
    ),
    Need::SignInAgain => (
      reason(ctx, id, need)?,
      (
        sign_in_again(ctx, id, invite, answers),
        decline(ctx, id, invite, answers),
      ),
      None,
    ),
    Need::Refused => (
      refused(ctx, id),
      (
        sign_in_again(ctx, id, invite, answers),
        not_now(ctx, id, invite, answers),
      ),
      Some("cloud.invites.twoFactor.signInNote"),
    ),
  };
  let lines = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Md)
    .child(first)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(buttons.0)
        .child(buttons.1),
    )
    .with_children(note.map(|key| {
      Text::new(&ctx.t(key))
        .id(format!("{id}-two-factor-note"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0))
    }))
    .with_children(answers.person.reason(ctx, &format!("{id}-sign-in-again")));
  Some(
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(Row::new().width(Dimension::Px(AVATAR)).flex_shrink(0.0))
      .child(lines)
      .into(),
  )
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

/// *2 recovery codes left*, after this session's sign-in with a recovery code left few, with the security page.
pub(super) fn recovery_warning(ctx: &mut Ctx, id: &str, answers: &Answers) -> Option<Element> {
  let left = Services::of(ctx).cloud.few_recovery_codes()?;
  let title = counted(
    ctx,
    ("cloud.recovery.left.one", "cloud.recovery.left.other"),
    usize::try_from(left).unwrap_or(usize::MAX),
  );
  let open = ActionButton::new(
    format!("{id}-security"),
    ButtonKind::Secondary,
    ctx.t("cloud.recovery.open"),
  )
  .icon(Icon::ExternalLink)
  .on_click(opener(ctx, answers));
  Some(
    Callout::new(id.to_owned(), CalloutTone::Warning, title)
      .icon(Icon::KeyRound)
      .body(ctx.t("cloud.recovery.body"))
      .action(open)
      .into(),
  )
}
