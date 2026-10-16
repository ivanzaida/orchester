//! The gate's sign-in card (`Sign-in card` `Bg4k0` in `Lpznn`), N1's cloud sign-in through the cloud's API: the email,
//! the password with *Forgot password?* over it, *Sign in*, *New to Orchester?* with *Create an account*, and what the
//! two pages in the browser are for. The refusals in place: a wrong email or password (`Atym1`), no internet
//! (`i8HD8`), an email to confirm first (`vovth`), too many attempts (`t2C4a`); while it signs in, the fields fade and
//! the button says so (`U0v7kx`). Signing in is a person's: synthetic input (an MCP client's) is refused, and says so.

use std::time::Duration;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, Interval},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_api::{Credential, cloud::CloudErrorCode};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  display,
  sharing::cloud::{CloudError, WebPage},
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
    pressable::pressable,
  },
};

/// A divider's thickness (`Divider` `I2ZKts`).
const DIVIDER: f32 = 1.0;
/// The email's label line (`Label` `lSC1y` in `Field · Email` `JPVVZ`): the field block is 48 high.
const LABEL_HEIGHT: f32 = 14.0;
/// The password's label line with *Forgot password?* (`Password label` `N1TYBK`): the field block is 49 high.
const PASSWORD_LABEL_HEIGHT: f32 = 15.0;
/// How often a paused sign-in looks whether its time has come.
const PAUSE_LOOK: Duration = Duration::from_secs(15);
/// How long a pause lasts when the cloud does not say: its sign-in limit's 15 minutes (`docs/cloud-api.md`).
const PAUSE_DEFAULT: u64 = 15 * 60;

const SUBMIT: &str = "gate-sign-in";

/// Why the cloud did not sign this computer in, as the card says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
  /// A wrong email or password: both fields are marked.
  Wrong,
  /// *Confirm your email first*, for this email, with *Open Orchester Cloud*.
  Unconfirmed(String),
  /// *Too many attempts*: this email's sign-in is paused until then.
  Paused {
    email: String,
    until: Timestamp,
  },
  /// *Signing in needs the internet once*: the cloud did not answer then.
  Unreachable(Timestamp),
  /// The account has two-factor authentication on, and the app cannot ask for its code yet.
  TwoFactor,
  Other(Said),
}

impl DevtoolsInspectable for Refusal {}

impl Refusal {
  fn of(error: &CloudError, email: &str) -> Self {
    let now = Timestamp::now();
    match error.cloud_code() {
      Some(CloudErrorCode::InvalidCredentials) => Self::Wrong,
      Some(CloudErrorCode::EmailUnconfirmed) => Self::Unconfirmed(email.to_owned()),
      Some(CloudErrorCode::RateLimited) => {
        let seconds = error.retry_after().unwrap_or(PAUSE_DEFAULT);
        let millis = i64::try_from(seconds.saturating_mul(1000)).unwrap_or(i64::MAX);
        Self::Paused {
          email: email.to_owned(),
          until: Timestamp::from_unix_millis(now.unix_millis().saturating_add(millis)),
        }
      }
      _ if error.two_factor_required() => Self::TwoFactor,
      _ if error.unanswered() => Self::Unreachable(now),
      _ => Self::Other(cloud_said(error)),
    }
  }
}

/// The card's state: what was typed, the sign-in under way, and a page that did not open.
pub(super) struct Form {
  email: Signal<String>,
  /// Made outside the component's context, so DevTools never records it ([`FieldInput::masked`]).
  password: Signal<String>,
  sign_in: FutureAction<(String, String), (), Refusal>,
  /// The cloud's page that could not open in the browser.
  unopened: Signal<Option<String>>,
  person: PersonOnly,
  /// Looks while a sign-in is paused, so *Sign in* comes back when its time has come.
  pause_look: Interval,
  tick: Signal<u64>,
}

impl Form {
  /// The card, its email filled in with `email` (the account whose sign-in ended here).
  pub(super) fn new(ctx: &mut Ctx, email: &str) -> Self {
    let cloud = Services::of(ctx).cloud;
    let password = Signal::new(String::new());
    let clear = password.clone();
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    Self {
      email: ctx.signal(email.to_owned()),
      password,
      sign_in: ctx.future_action(move |(email, password): (String, String)| {
        let (cloud, clear) = (cloud.clone(), clear.clone());
        async move {
          let email = email.trim().to_owned();
          let signed_in = cloud.sign_in(email.clone(), Credential::new(password)).await;
          let signed_in = signed_in.map(drop).map_err(|error| {
            tracing::info!(%error, "Orchester Cloud did not sign this computer in");
            Refusal::of(&error, &email)
          });
          // A wrong email or password leaves the email to correct and the password to type again (`Atym1`); any
          // other refusal keeps the password, masked, for the next try.
          if matches!(signed_in, Ok(()) | Err(Refusal::Wrong)) {
            clear.set(String::new());
          }
          signed_in
        }
      }),
      unopened: ctx.signal(None),
      person: PersonOnly::new(ctx),
      pause_look: ctx.create_interval(PAUSE_LOOK, move || next.update(|tick| *tick += 1)),
      tick,
    }
  }

  /// Whether a sign-in is under way: the gate's other parts fade with the card's.
  pub(super) fn pending(&self) -> bool {
    self.sign_in.state().get().is_pending()
  }

  /// The refusal shown, once the sign-in that got it ended.
  fn refusal(&self) -> Option<Refusal> {
    let state = self.sign_in.state().get();
    let pending = state.is_pending();
    state.error.filter(|_| !pending)
  }

  /// Whether this email's sign-in is paused now.
  fn paused(&self) -> bool {
    self.tick.get();
    match self.refusal() {
      Some(Refusal::Paused { email, until }) => {
        let paused = email == self.email.get().trim() && Timestamp::now() < until;
        if paused {
          self.pause_look.start();
        } else {
          self.pause_look.stop();
        }
        paused
      }
      _ => false,
    }
  }
}

/// The card, on the cloud whose host is `host`.
pub(super) fn card(ctx: &mut Ctx, form: &Form, host: &str) -> Element {
  let pending = form.pending();
  let refusal = form.refusal();
  let faded = |part: Element| {
    if pending {
      Row::new()
        .width(Dimension::Pct(100.0))
        .opacity(DISABLED_OPACITY)
        .child(part)
        .into()
    } else {
      part
    }
  };
  let (email, password) = fields(ctx, form, refusal == Some(Refusal::Wrong));
  let checking = pending.then(|| {
    Text::new(&ctx.t_args("gate.checking", [("host", host)]))
      .id("gate-checking")
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
  });
  Column::new()
    .id("gate-card")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Xl)
    .spacing(OrchesterSpacing::FormGap.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(faded(email))
    .child(faded(password))
    .with_children(refusal.as_ref().map(|refusal| refusal_part(ctx, form, (refusal, host))))
    .child(submit(ctx, form))
    .with_children(checking)
    .with_children(form.person.reason(ctx, SUBMIT))
    .with_children(unopened(ctx, form))
    .child(divider())
    .child(faded(create_row(ctx, form)))
    .child(
      Text::new(&ctx.t("gate.browserNote"))
        .id("gate-browser-note")
        .variant(GateText::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// *Sign in*, *Signing in…* while it does, faded while this email's sign-in is paused.
fn submit(ctx: &mut Ctx, form: &Form) -> Element {
  let button = ActionButton::new(SUBMIT, ButtonKind::Primary, ctx.t("gate.submit"))
    .icon(Icon::LogIn)
    .fill_width();
  if form.pending() {
    return ActionButton::new(SUBMIT, ButtonKind::Primary, ctx.t("gate.signingIn"))
      .fill_width()
      .loading()
      .into();
  }
  if form.paused() {
    return button.disabled().into();
  }
  let (action, email, password, person) = (
    form.sign_in.clone(),
    form.email.clone(),
    form.password.clone(),
    form.person.clone(),
  );
  button
    .on_click(move || {
      if person.allows(SUBMIT) {
        action.run((email.get(), password.get()));
      }
    })
    .into()
}

/// The email, and the password with *Forgot password?* over it; both marked after a wrong one.
fn fields(ctx: &mut Ctx, form: &Form, wrong: bool) -> (Element, Element) {
  let mut email =
    FieldInput::new("gate-email", ctx.t("gate.email"), form.email.clone()).placeholder(ctx.t("gate.emailPlaceholder"));
  let mut password = FieldInput::new("gate-password", ctx.t("gate.password"), form.password.clone())
    .masked()
    .no_trailing();
  if wrong {
    email = email.invalid();
    password = password.invalid();
  }
  let forgot = page_link(ctx, "gate-forgot", &ctx.t("gate.forgot"), (form, WebPage::Forgot));
  let label = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PASSWORD_LABEL_HEIGHT))
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("gate.password")).variant(TypographyStyle::FieldLabel))
    .child(Spacer::new().flex(1.0))
    .child(forgot);
  let password = Column::new()
    .id("gate-password-field")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(label)
    .child(password.render(ctx));
  let email = Column::new()
    .id("gate-email-field")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&ctx.t("gate.email"))
        .id("gate-email-field-label")
        .variant(TypographyStyle::FieldLabel)
        .height(Dimension::Px(LABEL_HEIGHT)),
    )
    .child(email.render(ctx));
  (email.into(), password.into())
}

/// The refusal in place: the fields' error, or a callout.
fn refusal_part(ctx: &mut Ctx, form: &Form, (refusal, host): (&Refusal, &str)) -> Element {
  match refusal {
    Refusal::Wrong => field_error("gate-wrong", &ctx.t("gate.wrong")).into(),
    Refusal::Unconfirmed(email) => Callout::new(
      "gate-unconfirmed",
      CalloutTone::Warning,
      ctx.t("gate.unconfirmed.title"),
    )
    .icon(Icon::MailWarning)
    .body(ctx.t_args("gate.unconfirmed.body", [("email", email.as_str())]))
    .action(page_button(
      ctx,
      "gate-open-cloud",
      &ctx.t("gate.unconfirmed.open"),
      (form, WebPage::SignIn),
    ))
    .into(),
    Refusal::Paused { until, .. } => Callout::new("gate-paused", CalloutTone::Danger, ctx.t("gate.paused.title"))
      .icon(Icon::Clock)
      .body(ctx.t_args("gate.paused.body", [("time", display::clock(*until))]))
      .into(),
    Refusal::Unreachable(at) => Callout::new(
      "gate-unreachable",
      CalloutTone::Neutral,
      ctx.t("gate.unreachable.title"),
    )
    .icon(Icon::WifiOff)
    .body(ctx.t("gate.unreachable.body"))
    .note(ctx.t_args(
      "gate.unreachable.when",
      [("host", host.to_owned()), ("time", display::clock(*at))],
    ))
    .into(),
    Refusal::TwoFactor => Callout::new("gate-two-factor", CalloutTone::Warning, ctx.t("gate.twoFactor.title"))
      .icon(Icon::ShieldAlert)
      .body(ctx.t("gate.twoFactor.body"))
      .into(),
    Refusal::Other(said) => field_error("gate-failed", &said.text(ctx)).into(),
  }
}

/// Why the cloud's page did not open in the browser.
fn unopened(ctx: &Ctx, form: &Form) -> Option<Element> {
  let page = form.unopened.get()?;
  Some(
    field_error(
      "gate-open-failed",
      &ctx.t_args("gate.openFailed", [("page", page.as_str())]),
    )
    .into(),
  )
}

/// *New to Orchester?* with *Create an account*.
fn create_row(ctx: &mut Ctx, form: &Form) -> Element {
  let create = page_button(
    ctx,
    "gate-create-account",
    &ctx.t("gate.create"),
    (form, WebPage::SignUp),
  );
  Row::new()
    .id("gate-create")
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("gate.newHere")).variant(ExtraTypography::Lead.role()))
    .child(Spacer::new().flex(1.0))
    .child(create)
    .into()
}

/// Opens `page` of the cloud in the browser; says so in place when it cannot.
fn opener(ctx: &mut Ctx, (form, page): (&Form, WebPage)) -> impl Fn() + Send + Sync + 'static {
  let services = Services::of(ctx);
  let (cloud, links, unopened) = (services.cloud, services.links, form.unopened.clone());
  move || {
    let url = match cloud.web_page(page) {
      Ok(url) => url,
      Err(error) => {
        tracing::warn!(%error, "the cloud's page has no address");
        return;
      }
    };
    match links.open(&url) {
      Ok(()) => unopened.set(None),
      Err(error) => {
        tracing::warn!(%error, "the cloud's page could not be opened");
        unopened.set(Some(url));
      }
    }
  }
}

fn page_button(ctx: &mut Ctx, id: &str, label: &str, target: (&Form, WebPage)) -> Element {
  ActionButton::new(id.to_owned(), ButtonKind::Secondary, label.to_owned())
    .icon(Icon::ExternalLink)
    .on_click(opener(ctx, target))
    .into()
}

/// A quiet link with the external glyph after it (`Link · Forgot password?` `KjUBG`).
fn page_link(ctx: &mut Ctx, id: &str, label: &str, target: (&Form, WebPage)) -> Element {
  pressable(opener(ctx, target))
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", label.to_owned())
    .child(
      Text::new(label)
        .variant(ExtraTypography::NoteStrong.role())
        .color(PaletteColor::TextSecondary),
    )
    .child(icon(Icon::ExternalLink, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

/// The hairline over *New to Orchester?*.
fn divider() -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(DIVIDER))
    .flex_shrink(0.0)
    .background(PaletteColor::Border)
    .into()
}
