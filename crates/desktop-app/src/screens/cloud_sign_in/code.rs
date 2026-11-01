//! The sign-in's code step (`eWsUb`), in the fields' place once the password of an account with two-factor
//! authentication was right: *Password accepted* and *Code*, what to type, six boxes for the app's digits (one input
//! over them, so a pasted code works), *Verify*, *Use a recovery code instead* and *Back*. While it verifies, the boxes
//! and the links fade and the button says so; a wrong code clears the boxes and says so under them; too many codes
//! pause it until the cloud's time; a challenge that ended takes the sign-in back to the password with *Sign-in timed
//! out*, on its own once its five minutes passed.
//!
//! The step is a lurq form like the password's: the code takes the focus and the first Tab stop, Tab goes on through
//! *Verify* and the links, and Enter in the code verifies as *Verify* does. Verifying is a person's: synthetic input is
//! refused and says so. A recovery code is typed masked in a field of its own: it works until it is used, so it never
//! shows. The code is held outside the component's context, so DevTools never records it, and cleared after every
//! answer.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, Interval},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Form as LurqForm, FormProps, Row, Stack, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_api::{Credential, cloud::CloudErrorCode};
use orchester_core::time::Timestamp;
use orchester_workspace::{display, sharing::cloud::CloudError};

use super::{
  WIDTH,
  form::{LABEL_HEIGHT, ToPassword},
  step::{Entry, Step},
  title,
};
use crate::{
  screens::settings::orchester::{PersonOnly, Said, cloud_said},
  services::Services,
  theme::{
    ControlHeight, DISABLED_OPACITY, ExtraSpacing, ExtraTypography, IconSize, OrchesterSpacing, SignInSpacing,
    SignInText,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    pressable::{Focus, pressable},
  },
};

const VERIFY: &str = "sign-in-verify";
/// The code's place at the head of the window's Tab order.
const CODE_TAB: i32 = 1;
/// How often the step looks whether a pause is over or its challenge ended.
const LOOK: Duration = Duration::from_secs(15);
/// How long a pause lasts when the cloud does not say: its code limit's 15 minutes (`docs/cloud-api.md`).
const PAUSE_DEFAULT: u64 = 15 * 60;
/// The digits of a code from the app.
const DIGITS: usize = 6;
/// A digit's box (`eWsUb`).
const BOX_HEIGHT: f32 = 52.0;
/// The dash between the third box and the fourth (`eWsUb`).
const DASH_WIDTH: f32 = 12.0;
const DASH_HEIGHT: f32 = 1.0;
/// The caret in the box that takes the next digit (`eWsUb`).
const CARET_WIDTH: f32 = 1.5;
const CARET_HEIGHT: f32 = 22.0;
/// The line between *Password accepted* and *Code* (`eWsUb`).
const STEP_LINE: f32 = 24.0;

/// Why the cloud did not take a code, for the entry it was typed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
  /// Not the code: the boxes are marked and say so.
  Wrong,
  /// *Too many attempts*: codes for the account are paused until then.
  Paused(Timestamp),
  /// The challenge ended: the sign-in went back to the password.
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

/// The account whose codes the cloud paused, by its email, and until when: for every code step of this sign-in for that
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
pub(crate) struct CodeForm {
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
  pub(crate) fn new(ctx: &mut Ctx, step: Signal<Step>, to_password: ToPassword) -> Self {
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

  /// Stops looking at the clock while the sign-in is on the password.
  pub(crate) fn rest(&self) {
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

  /// Verifies what was typed for `entry`, for *Verify* and Enter alike. Nothing happens while a check is under way or
  /// codes are paused; an empty code takes the focus; a code of the app that is not six digits is not sent; synthetic
  /// input is refused, and the step says so under *Verify*.
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

  /// Moves the focus to the code once for each entry.
  fn take_focus(&self, ctx: &Ctx, entry: &Entry) {
    if self.focused.get() == Some(entry.id) {
      return;
    }
    self.focused.set(Some(entry.id));
    ctx.focus(&self.input);
  }
}

/// The code step for `entry`, on the cloud whose host is `host`.
pub(crate) fn step(ctx: &mut Ctx, form: &CodeForm, entry: &Entry, host: &str) -> Element {
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
  let code = faded(code_part(ctx, form, entry, said.is_some()));
  let said = said.map(|key| Element::from(field_error("sign-in-code-note", &ctx.t(key))));
  let checking = verifying.then(|| {
    Text::new(&ctx.t_args("sign_in.checking", [("host", host)]))
      .id("sign-in-code-checking")
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
  });
  let (heading, lead) = if entry.recovery() {
    ("sign_in.code.recoveryTitle", "sign_in.code.recoveryLead")
  } else {
    ("sign_in.code.title", "sign_in.code.lead")
  };
  let form_part = Column::new()
    .id("sign-in-code-form")
    .width(Dimension::Px(WIDTH))
    .max_width(Dimension::Pct(100.0))
    .spacing(OrchesterSpacing::FormGap.role())
    .child(code)
    .with_children(said)
    .with_children(refused(ctx, refusal.as_ref(), paused))
    .child(verify_button(ctx, form, entry, paused.is_some()))
    .with_children(checking)
    .with_children(form.person.reason(ctx, VERIFY))
    .child(faded(links(ctx, form, entry, held)));
  let submit = form.submitter(entry);
  Column::new()
    .id("sign-in-code-card")
    .width(Dimension::Pct(100.0))
    .spacing(OrchesterSpacing::FormGap.role())
    .child(accepted(ctx))
    .child(title(ctx, "sign-in-title", heading))
    .child(
      Text::new(&ctx.t(lead))
        .id("sign-in-lead")
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
    .child(LurqForm::element(
      FormProps::default().on_submit_data(move |_| submit()),
      form_part,
    ))
    .into()
}

/// *Password accepted*, in `success`, a line, and *Code*, the step this is.
fn accepted(ctx: &Ctx) -> Element {
  Row::new()
    .id("sign-in-code-accepted")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleCheck, IconSize::Md, PaletteColor::Success))
        .child(
          Text::new(&ctx.t("sign_in.code.passwordAccepted"))
            .variant(ExtraTypography::NoteStrong.role())
            .color(PaletteColor::Success),
        ),
    )
    .child(
      Row::new()
        .width(Dimension::Px(STEP_LINE))
        .height(Dimension::Px(DASH_HEIGHT))
        .background(PaletteColor::Border),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleDot, IconSize::Md, PaletteColor::TextPrimary))
        .child(
          Text::new(&ctx.t("sign_in.code.step"))
            .variant(ExtraTypography::NoteStrong.role())
            .color(PaletteColor::TextPrimary),
        ),
    )
    .into()
}

/// The six boxes and their label; a recovery code in a masked field instead. One input takes what is typed or pasted,
/// over the boxes, which show it.
fn code_part(ctx: &mut Ctx, form: &CodeForm, entry: &Entry, wrong: bool) -> Element {
  let label = if entry.recovery() {
    "sign_in.code.recoveryLabel"
  } else {
    "sign_in.code.label"
  };
  let mut input = FieldInput::new("sign-in-code", ctx.t(label), form.code.clone())
    .text_style(SignInText::Code.role())
    .element(form.input.clone())
    .tab_first(CODE_TAB);
  if wrong {
    input = input.invalid();
  }
  let shown: Element = if entry.recovery() {
    input
      .placeholder(ctx.t("sign_in.code.recoveryPlaceholder"))
      .box_height(ControlHeight::Lg.value())
      .masked()
      .no_trailing()
      .render(ctx)
  } else {
    let typed = form.code.get();
    let input = input.box_height(BOX_HEIGHT).render(ctx);
    Stack::new()
      .id("sign-in-code-boxes")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(BOX_HEIGHT))
      .child(digit_boxes(&typed, wrong))
      .child(Row::new().width(Dimension::Pct(100.0)).opacity(0.0).child(input))
      .into()
  };
  Column::new()
    .id("sign-in-code-field")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&ctx.t(label))
        .id("sign-in-code-label")
        .variant(TypographyStyle::FieldLabel)
        .height(Dimension::Px(LABEL_HEIGHT)),
    )
    .child(shown)
    .into()
}

/// A box per digit of `typed`, a dash between the third and the fourth, and the caret in the box that takes the next
/// one.
fn digit_boxes(typed: &str, wrong: bool) -> Element {
  let digits: Vec<char> = typed.chars().filter(|c| !c.is_whitespace()).collect();
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(BOX_HEIGHT))
    .spacing(SignInSpacing::Digits.role())
    .align_items(Alignment::Center);
  for place in 0..DIGITS {
    if place == DIGITS / 2 {
      row = row.child(
        Row::new()
          .width(Dimension::Px(DASH_WIDTH))
          .height(Dimension::Px(DASH_HEIGHT))
          .flex_shrink(0.0)
          .background(PaletteColor::Border),
      );
    }
    row = row.child(digit_box(
      place,
      digits.get(place).copied(),
      digits.len() == place,
      wrong,
    ));
  }
  row.into()
}

fn digit_box(place: usize, digit: Option<char>, next: bool, wrong: bool) -> Element {
  let stroke = if wrong {
    Border::inside(BorderSize::Sm, PaletteColor::Danger)
  } else if next {
    Border::inside(BorderSize::Md, PaletteColor::BorderFocus)
  } else if digit.is_some() {
    Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)
  } else {
    Border::inside(BorderSize::Sm, PaletteColor::Border)
  };
  let shown = digit.map(|digit| Text::new(&digit.to_string()).variant(SignInText::Digit.role()));
  let caret = (next && !wrong).then(|| {
    Row::new()
      .width(Dimension::Px(CARET_WIDTH))
      .height(Dimension::Px(CARET_HEIGHT))
      .background(PaletteColor::TextPrimary)
  });
  Row::new()
    .id(format!("sign-in-code-box-{place}"))
    .flex(1.0)
    .height(Dimension::Px(BOX_HEIGHT))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceInput)
    .border(stroke)
    .rounded(RadiusSize::Md)
    .with_children(shown)
    .with_children(caret)
    .into()
}

/// What the field error under the code says: a code not sent, else the cloud's word on the one before.
fn said(form: &CodeForm, entry: &Entry, refusal: Option<&Refusal>) -> Option<&'static str> {
  if form.unsent.get() == Some(entry.id) {
    return Some("sign_in.code.sixDigits");
  }
  match refusal {
    Some(Refusal::Wrong) if entry.recovery() => Some("sign_in.code.wrongRecovery"),
    Some(Refusal::Wrong) => Some("sign_in.code.wrong"),
    _ => None,
  }
}

/// *Too many attempts* until the cloud's time, or another refusal in the field error's place.
fn refused(ctx: &Ctx, refusal: Option<&Refusal>, paused: Option<Timestamp>) -> Option<Element> {
  if let Some(until) = paused {
    return Some(
      Callout::new(
        "sign-in-code-paused",
        CalloutTone::Danger,
        ctx.t("sign_in.code.paused.title"),
      )
      .icon(Icon::Clock)
      .body(ctx.t_args("sign_in.code.paused.body", [("time", display::clock(until))]))
      .into(),
    );
  }
  match refusal? {
    Refusal::Other(said) => Some(field_error("sign-in-code-failed", &said.text(ctx)).into()),
    Refusal::Wrong | Refusal::Paused(_) | Refusal::Ended => None,
  }
}

/// *Verify*, as wide as the fields, *Verifying…* while it does; faded while codes are paused.
fn verify_button(ctx: &Ctx, form: &CodeForm, entry: &Entry, paused: bool) -> ActionButton {
  if form.verifying() {
    return ActionButton::new(VERIFY, ButtonKind::Primary, ctx.t("sign_in.code.verifying"))
      .fill_width()
      .large()
      .loading();
  }
  let verify = ActionButton::new(VERIFY, ButtonKind::Primary, ctx.t("sign_in.code.verify"))
    .after(Icon::ArrowRight)
    .fill_width()
    .large();
  if paused {
    verify.disabled()
  } else {
    verify.in_form().on_click(form.submitter(entry))
  }
}

/// *Use a recovery code instead*, or *Use your authenticator app instead* while a recovery code is typed: the same
/// sign-in, the code empty, doing nothing while `held`: a code is checked, or codes are paused. And *Back* to the
/// password, which only a check under way holds.
fn links(ctx: &Ctx, form: &CodeForm, entry: &Entry, held: bool) -> Element {
  let verifying = form.verifying();
  let (glyph, key) = if entry.recovery() {
    (Icon::Smartphone, "sign_in.code.useApp")
  } else {
    (Icon::KeyRound, "sign_in.code.useRecovery")
  };
  let (step, code, next) = (form.step.clone(), form.code.clone(), entry.switched());
  let switch = link(ctx, ("sign-in-code-switch", key, glyph), move || {
    if !held {
      code.set(String::new());
      step.set(next.clone());
    }
  });
  let to_password = form.to_password.clone();
  let back = link(
    ctx,
    ("sign-in-code-back", "sign_in.code.back", Icon::ArrowLeft),
    move || {
      if !verifying {
        to_password(false);
      }
    },
  );
  Row::new()
    .id("sign-in-code-links")
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .child(switch)
    .child(back)
    .into()
}

/// A quiet link, its glyph before its words, a stop of the form's Tab order.
fn link(ctx: &Ctx, (id, key, glyph): (&str, &str, Icon), press: impl Fn() + Send + Sync + 'static) -> Element {
  let label = ctx.t(key);
  Focus::Link
    .apply(pressable(press))
    .id(id.to_owned())
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
