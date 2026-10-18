//! The gate's code step (`Tmwkg`, `Dix2w`), in the sign-in card's place once the password of an account with
//! two-factor authentication was right: the account and *Password accepted*, one field for the six digits of the app
//! (mono, so a pasted code works), *Back* to the password, *Verify*, and *Use a recovery code instead* (`hJ2yS`), and
//! back. While it verifies, the field, *Back* and the link fade and the button says so (`iflk6`); a wrong code clears
//! the field and says so under it (`DtxLq`); too many codes pause it until the cloud's time (`qZyaZ`); a challenge that
//! ended takes the gate back to the password with *Sign-in timed out* (`R2wOmb`), on its own once its five minutes
//! passed.
//!
//! The card is a lurq form like the password's: the field takes the focus and the first Tab stop, Tab goes on through
//! *Back*, *Verify* and the link, and Enter in the field verifies as *Verify* does. Verifying is a person's: synthetic
//! input is refused and says so. A recovery code is typed masked: it works until it is used, so it never shows. The
//! code is held outside the component's context, so DevTools never records it, and cleared after every answer.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, Interval},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Form as LurqForm, FormProps, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_api::{Credential, cloud::CloudErrorCode};
use orchester_core::time::Timestamp;
use orchester_workspace::{display, sharing::cloud::CloudError};

use super::{
  form::{LABEL_HEIGHT, ToPassword, divider},
  step::{Entry, Step},
};
use crate::{
  screens::settings::orchester::{PersonOnly, Said, cloud_said},
  services::Services,
  theme::{DISABLED_OPACITY, ExtraSpacing, ExtraTypography, GateText, IconSize, OrchesterSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    pressable::{Focus, pressable},
  },
};

const VERIFY: &str = "gate-verify";
/// The code field's place at the head of the window's Tab order.
const CODE_TAB: i32 = 1;
/// How often the step looks whether a pause is over or its challenge ended.
const LOOK: Duration = Duration::from_secs(15);
/// How long a pause lasts when the cloud does not say: its code limit's 15 minutes (`docs/cloud-api.md`).
const PAUSE_DEFAULT: u64 = 15 * 60;
/// The digits of a code from the app.
const DIGITS: usize = 6;
/// The account line over the divider (`Account` `Nm7zk`): as high as the email.
const ACCOUNT_LINE: f32 = 16.0;
/// *Use a recovery code instead* (`Link · Use a recovery code instead` `uIJ9l`).
const SWITCH_LINE: f32 = 15.0;

/// Why the cloud did not take a code, for the entry it was typed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
  /// Not the code: the field is marked and says so.
  Wrong,
  /// *Too many attempts*: codes for the account are paused until then.
  Paused(Timestamp),
  /// The challenge ended: the gate went back to the password.
  Ended,
  Other(Said),
}

impl Refusal {
  fn of(error: &CloudError) -> Self {
    match error.cloud_code() {
      Some(CloudErrorCode::InvalidCode) => Self::Wrong,
      Some(CloudErrorCode::ChallengeExpired) => Self::Ended,
      Some(CloudErrorCode::RateLimited) => {
        let seconds = error.retry_after().unwrap_or(PAUSE_DEFAULT);
        let millis = i64::try_from(seconds.saturating_mul(1000)).unwrap_or(i64::MAX);
        Self::Paused(Timestamp::from_unix_millis(
          Timestamp::now().unix_millis().saturating_add(millis),
        ))
      }
      _ => Self::Other(cloud_said(error)),
    }
  }
}

/// The account whose codes the cloud paused, by its email, and until when: for every code step of this gate for that
/// account, not only the one it said it in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PausedUntil(Option<(String, Timestamp)>);

impl DevtoolsInspectable for PausedUntil {}

impl PausedUntil {
  /// The time, for the account of `email`, while it is still to come.
  fn now(&self, email: &str) -> Option<Timestamp> {
    self
      .0
      .as_ref()
      .filter(|(paused, until)| paused == email && Timestamp::now() < *until)
      .map(|(_, until)| *until)
  }
}

/// A refusal and the entry it is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Refused(u64, Refusal);

impl DevtoolsInspectable for Refused {}

/// The code step's state: what was typed, the check under way and what it came to.
pub(super) struct CodeForm {
  /// Made outside the component's context, so DevTools never records it.
  code: Signal<String>,
  verify: FutureAction<(Entry, String), (), Refused>,
  person: PersonOnly,
  look: Interval,
  tick: Signal<u64>,
  /// The entry whose code was not sent because it is not six digits.
  unsent: Signal<Option<u64>>,
  pause: Signal<PausedUntil>,
  /// The entry whose field took the focus.
  focused: Signal<Option<u64>>,
  input: ElementRef,
  step: Signal<Step>,
  to_password: ToPassword,
}

impl CodeForm {
  pub(super) fn new(ctx: &mut Ctx, step: Signal<Step>, to_password: ToPassword) -> Self {
    let cloud = Services::of(ctx).cloud;
    let code = Signal::new(String::new());
    let (unsent, pause) = (ctx.signal(None), ctx.signal(PausedUntil(None)));
    // Back to the password, the code typed so far gone: the next code step may be one that shows what is typed.
    let to_password: ToPassword = {
      let (to_password, code, unsent) = (to_password, code.clone(), unsent.clone());
      Arc::new(move |timed_out| {
        code.set(String::new());
        unsent.set(None);
        to_password(timed_out);
      })
    };
    let (clear, back, paused) = (code.clone(), to_password.clone(), pause.clone());
    let tick = ctx.signal(0_u64);
    let (next, watched, ended) = (tick.clone(), step.clone(), to_password.clone());
    Self {
      code,
      verify: ctx.future_action(move |(entry, typed): (Entry, String)| {
        let (cloud, clear, back, paused) = (cloud.clone(), clear.clone(), back.clone(), paused.clone());
        async move {
          let finished = cloud
            .finish_sign_in(&entry.challenge, (Credential::new(typed), entry.kind))
            .await;
          clear.set(String::new());
          finished.map(drop).map_err(|error| {
            tracing::info!(%error, "Orchester Cloud did not take the two-factor code");
            let refusal = Refusal::of(&error);
            match refusal {
              Refusal::Ended => back(true),
              Refusal::Paused(until) => paused.set(PausedUntil(Some((entry.challenge.email().to_owned(), until)))),
              Refusal::Wrong | Refusal::Other(_) => {}
            }
            Refused(entry.id, refusal)
          })
        }
      }),
      person: PersonOnly::new(ctx),
      look: ctx.create_interval(LOOK, move || {
        if let Step::Code(entry) = watched.get_untracked()
          && entry.expired()
        {
          ended(true);
        }
        next.update(|tick| *tick += 1);
      }),
      tick,
      unsent,
      pause,
      focused: ctx.signal(None),
      input: ElementRef::new(),
      step,
      to_password,
    }
  }

  /// Stops looking at the clock while the gate is on the password.
  pub(super) fn rest(&self) {
    self.look.stop();
  }

  fn verifying(&self) -> bool {
    self.verify.state().get().is_pending()
  }

  /// What the last check of `entry` came to, once it ended.
  fn refusal(&self, entry: &Entry) -> Option<Refusal> {
    let state = self.verify.state().get();
    let pending = state.is_pending();
    state
      .error
      .filter(|Refused(id, _)| !pending && *id == entry.id)
      .map(|Refused(_, refusal)| refusal)
  }

  /// Until when codes are paused for the account of `entry`, while that time is still to come: after *Back* or a
  /// timeout too.
  fn paused(&self, entry: &Entry) -> Option<Timestamp> {
    self.tick.get();
    self.pause.get().now(entry.challenge.email())
  }
}

impl CodeForm {
  /// Verifies what was typed for `entry`, for *Verify* and Enter alike. Nothing happens while a check is under way or
  /// codes are paused; an empty field takes the focus; a code of the app that is not six digits is not sent; synthetic
  /// input is refused, and the card says so under *Verify*.
  fn submitter(&self, entry: &Entry) -> impl Fn() + Send + Sync + 'static {
    let (action, code, person, unsent, focused) = (
      self.verify.clone(),
      self.code.clone(),
      self.person.clone(),
      self.unsent.clone(),
      self.focused.clone(),
    );
    let (entry, pause) = (entry.clone(), self.pause.clone());
    move || {
      let paused = pause.get_untracked().now(entry.challenge.email()).is_some();
      if action.state().get_untracked().is_pending() || paused {
        return;
      }
      let typed: String = code.get_untracked().chars().filter(|c| !c.is_whitespace()).collect();
      if typed.is_empty() {
        focused.set(None);
        return;
      }
      let digits = typed.len() == DIGITS && typed.chars().all(|c| c.is_ascii_digit());
      if !entry.recovery() && !digits {
        unsent.set(Some(entry.id));
        return;
      }
      if person.allows(VERIFY) {
        unsent.set(None);
        action.run_if_idle((entry.clone(), typed));
      }
    }
  }

  /// Moves the focus to the field once for each entry.
  fn take_focus(&self, ctx: &Ctx, entry: &Entry) {
    if self.focused.get() == Some(entry.id) {
      return;
    }
    self.focused.set(Some(entry.id));
    ctx.focus(&self.input);
  }
}

/// The code step's card for `entry`, on the cloud whose host is `host`.
pub(super) fn card(ctx: &mut Ctx, form: &CodeForm, entry: &Entry, host: &str) -> Element {
  form.look.start();
  form.take_focus(ctx, entry);
  let verifying = form.verifying();
  let paused = form.paused(entry);
  let refusal = form.refusal(entry);
  let held = verifying || paused.is_some();
  let faded = |part: Element| -> Element {
    let row = Row::new().width(Dimension::Pct(100.0)).overflow_visible().child(part);
    if held { row.opacity(DISABLED_OPACITY) } else { row }.into()
  };
  let said = said(form, entry, refusal.as_ref());
  let field = faded(field(ctx, form, entry, (said.is_some(), paused.is_some())));
  let said = said.map(|key| Element::from(field_error("gate-code-note", &ctx.t(key))));
  let checking = verifying.then(|| {
    Text::new(&ctx.t_args("gate.checking", [("host", host)]))
      .id("gate-code-checking")
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
  });
  let card = Column::new()
    .id("gate-code-card")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Xl)
    .spacing(OrchesterSpacing::FormGap.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(account(ctx, entry))
    .child(divider())
    .child(field)
    .with_children(said)
    .with_children(refused(ctx, refusal.as_ref(), paused))
    .child(buttons(ctx, form, entry, paused.is_some()))
    .with_children(checking)
    .with_children(form.person.reason(ctx, VERIFY))
    .child(faded(switch(ctx, form, entry, held)));
  let submit = form.submitter(entry);
  LurqForm::element(FormProps::default().on_submit_data(move |_| submit()), card)
}

/// The account whose password was right, and *Password accepted* at the line's end.
fn account(ctx: &Ctx, entry: &Entry) -> Element {
  Row::new()
    .id("gate-code-account")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACCOUNT_LINE))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleUserRound, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(entry.challenge.email())
        .id("gate-code-email")
        .variant(ExtraTypography::Lead.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .id("gate-code-accepted")
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Check, IconSize::Sm, PaletteColor::Success))
        .child(Text::new(&ctx.t("gate.code.passwordAccepted")).variant(ExtraTypography::Note.role())),
    )
    .into()
}

/// The field: six digits in mono, or a recovery code, masked; marked after a wrong one, which the card says under it
/// ([`said`]). After a refusal (`wrong`, or codes `paused`) it is blank, without its placeholder (`DtxLq`, `qZyaZ`).
fn field(ctx: &mut Ctx, form: &CodeForm, entry: &Entry, (wrong, paused): (bool, bool)) -> Element {
  let (label, placeholder) = if entry.recovery() {
    ("gate.code.recoveryLabel", "gate.code.recoveryPlaceholder")
  } else {
    ("gate.code.label", "gate.code.placeholder")
  };
  let mut input = FieldInput::new("gate-code", ctx.t(label), form.code.clone())
    .text_style(GateText::Code.role())
    .element(form.input.clone())
    .tab_first(CODE_TAB);
  if !(wrong || paused) {
    input = input.placeholder(ctx.t(placeholder));
  }
  if entry.recovery() {
    input = input.masked().no_trailing();
  }
  if wrong {
    input = input.invalid();
  }
  Column::new()
    .id("gate-code-field")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&ctx.t(label))
        .id("gate-code-label")
        .variant(TypographyStyle::FieldLabel)
        .height(Dimension::Px(LABEL_HEIGHT)),
    )
    .child(input.render(ctx))
    .into()
}

/// What the field error under the field says: a code not sent, else the cloud's word on the one before (`DtxLq`).
fn said(form: &CodeForm, entry: &Entry, refusal: Option<&Refusal>) -> Option<&'static str> {
  if form.unsent.get() == Some(entry.id) {
    return Some("gate.code.sixDigits");
  }
  match refusal {
    Some(Refusal::Wrong) if entry.recovery() => Some("gate.code.wrongRecovery"),
    Some(Refusal::Wrong) => Some("gate.code.wrong"),
    _ => None,
  }
}

/// *Too many attempts* until the cloud's time, or another refusal in the field error's place.
fn refused(ctx: &Ctx, refusal: Option<&Refusal>, paused: Option<Timestamp>) -> Option<Element> {
  if let Some(until) = paused {
    return Some(
      Callout::new("gate-code-paused", CalloutTone::Danger, ctx.t("gate.code.paused.title"))
        .icon(Icon::Clock)
        .body(ctx.t_args("gate.code.paused.body", [("time", display::clock(until))]))
        .into(),
    );
  }
  match refusal? {
    Refusal::Other(said) => Some(field_error("gate-code-failed", &said.text(ctx)).into()),
    Refusal::Wrong | Refusal::Paused(_) | Refusal::Ended => None,
  }
}

/// *Back* and *Verify*, *Verifying…* while it does; *Verify* faded while codes are paused.
fn buttons(ctx: &Ctx, form: &CodeForm, entry: &Entry, paused: bool) -> Element {
  let back = ActionButton::new("gate-code-back", ButtonKind::Secondary, ctx.t("gate.code.back"))
    .icon(Icon::ArrowLeft)
    .in_form();
  let back = if form.verifying() {
    back.disabled()
  } else {
    let to_password = form.to_password.clone();
    back.on_click(move || to_password(false))
  };
  let verify = if form.verifying() {
    ActionButton::new(VERIFY, ButtonKind::Primary, ctx.t("gate.code.verifying")).loading()
  } else {
    let verify = ActionButton::new(VERIFY, ButtonKind::Primary, ctx.t("gate.code.verify")).icon(Icon::ShieldCheck);
    if paused {
      verify.disabled()
    } else {
      verify.in_form().on_click(form.submitter(entry))
    }
  };
  Row::new()
    .id("gate-code-buttons")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(back)
    .child(Row::new().flex(1.0).child(verify.fill_width()))
    .into()
}

/// *Use a recovery code instead*, or *Use your authenticator app instead* while a recovery code is typed: the same
/// sign-in, the field empty. It does nothing while `held`: a code is checked, or codes are paused.
fn switch(ctx: &Ctx, form: &CodeForm, entry: &Entry, held: bool) -> Element {
  let (glyph, key) = if entry.recovery() {
    (Icon::Smartphone, "gate.code.useApp")
  } else {
    (Icon::KeyRound, "gate.code.useRecovery")
  };
  let label = ctx.t(key);
  let (step, code, next) = (form.step.clone(), form.code.clone(), entry.switched());
  Focus::Link
    .apply(pressable(move || {
      if !held {
        code.set(String::new());
        step.set(next.clone());
      }
    }))
    .id("gate-code-switch")
    .height(Dimension::Px(SWITCH_LINE))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", label.to_string())
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&label)
        .variant(ExtraTypography::NoteStrong.role())
        .color(PaletteColor::TextSecondary),
    )
    .into()
}
