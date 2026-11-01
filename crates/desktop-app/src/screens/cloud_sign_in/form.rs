//! The sign-in's fields (`j7SbPk`, `wpD5E`): the email, the password with *Forgot password?* over it, *Sign in* and, in
//! a card, *New to Orchester?* with *Create an account*. The refusals in place: a wrong email or password, no internet,
//! an email to confirm first, too many attempts; while it signs in, the fields fade and the button says so. Signing in
//! is a person's: synthetic input (an MCP client's) is refused, and says so. A password the cloud answers with a
//! challenge goes on to the code step ([`super::code`]), which comes back here with *Back* or when the challenge ends.
//!
//! The fields are a lurq form: Tab goes through the email, the password, *Forgot password?*, *Sign in* and *Create an
//! account* in the order they show, and Enter in either field signs in as *Sign in* does. The email takes the focus
//! when it opens empty, the password when the email is filled in (a sign-in ended here).

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, Interval},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Form as LurqForm, FormProps, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::{Credential, cloud::CloudErrorCode};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  display,
  sharing::cloud::{CloudError, SignInStep, WebPage},
};

use super::{WIDTH, step::Step};
use crate::{
  screens::settings::orchester::{PersonOnly, Said, cloud_said},
  services::Services,
  theme::{ControlHeight, DISABLED_OPACITY, ExtraSpacing, ExtraTypography, IconSize, OrchesterSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    pressable::{Focus, pressable},
  },
};

/// A field's label line: the field block is 48 high.
pub(super) const LABEL_HEIGHT: f32 = 14.0;
/// The password's label line with *Forgot password?*: the field block is 49 high.
const PASSWORD_LABEL_HEIGHT: f32 = 15.0;
/// How often a paused sign-in looks whether its time has come.
const PAUSE_LOOK: Duration = Duration::from_secs(15);
/// How long a pause lasts when the cloud does not say: its sign-in limit's 15 minutes (`docs/cloud-api.md`).
const PAUSE_DEFAULT: u64 = 15 * 60;

/// *Sign in*, which only a person presses.
pub(super) const SUBMIT: &str = "sign-in-submit";
/// The email's and the password's places at the head of the window's Tab order: *Forgot password?* is drawn over the
/// password, and Tab reaches it after the password, as a person reads the card.
const EMAIL_TAB: i32 = 1;
const PASSWORD_TAB: i32 = 2;

/// Takes the sign-in back to the password step, the password empty and focused; `true` when a code step timed out.
pub(crate) type ToPassword = Arc<dyn Fn(bool) + Send + Sync>;

/// A field that takes the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
  Email,
  Password,
}

impl DevtoolsInspectable for Field {}

/// Why the cloud did not sign this computer in, as the sign-in says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
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
      _ if error.unanswered() => Self::Unreachable(now),
      _ => Self::Other(cloud_said(error)),
    }
  }
}

/// The sign-in's state: what was typed, the sign-in under way, and a page that did not open. [`super::SignInOffer`] and
/// the Cloud column of *Join a team* (`B914L`) share it, each with its own *Sign in*, which lays the fields out itself.
pub(crate) struct Form {
  pub(crate) email: Signal<String>,
  /// Made outside the component's context, so DevTools never records it ([`FieldInput::masked`]).
  pub(crate) password: Signal<String>,
  sign_in: FutureAction<(String, String), (), Refusal>,
  /// The cloud's page that could not open in the browser.
  unopened: Signal<Option<String>>,
  pub(crate) person: PersonOnly,
  /// Looks while a sign-in is paused, so *Sign in* comes back when its time has come.
  pause_look: Interval,
  tick: Signal<u64>,
  /// The field to focus at the next render.
  focus: Signal<Option<Field>>,
  pub(crate) email_input: ElementRef,
  pub(crate) password_input: ElementRef,
  step: Signal<Step>,
  /// The id of *Sign in*, which only a person presses.
  pub(crate) submit: &'static str,
}

impl Form {
  /// The sign-in, its email filled in with `email` (the account whose sign-in ended here), going on to the code step in
  /// `step`; `submit` is the id of its *Sign in*.
  pub(crate) fn new(ctx: &mut Ctx, email: &str, step: Signal<Step>, submit: &'static str) -> Self {
    let cloud = Services::of(ctx).cloud;
    let password = Signal::new(String::new());
    let (clear, coded) = (password.clone(), step.clone());
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    Self {
      email: ctx.signal(email.to_owned()),
      password,
      sign_in: ctx.future_action(move |(email, password): (String, String)| {
        let (cloud, clear, coded) = (cloud.clone(), clear.clone(), coded.clone());
        async move {
          let email = email.trim().to_owned();
          let signed_in = cloud.sign_in(email.clone(), Credential::new(password)).await;
          let signed_in = signed_in
            .map(|step| {
              if let SignInStep::Code(challenge) = step {
                coded.set(Step::code(challenge));
              }
            })
            .map_err(|error| {
              tracing::info!(%error, "Orchester Cloud did not sign this computer in");
              Refusal::of(&error, &email)
            });
          // A wrong email or password leaves the email to correct and the password to type again; any other refusal
          // keeps the password, masked, for the next try.
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
      focus: ctx.signal(Some(if email.is_empty() {
        Field::Email
      } else {
        Field::Password
      })),
      email_input: ElementRef::new(),
      password_input: ElementRef::new(),
      step,
      submit,
    }
  }

  /// Back to the fields from the code step: the email kept, the password to type again.
  pub(crate) fn to_password(&self) -> ToPassword {
    let (step, password, focus) = (self.step.clone(), self.password.clone(), self.focus.clone());
    Arc::new(move |timed_out| {
      password.set(String::new());
      focus.set(Some(Field::Password));
      step.set(Step::Password { timed_out });
    })
  }

  /// Whether a sign-in is under way.
  pub(crate) fn pending(&self) -> bool {
    self.sign_in.state().get().is_pending()
  }

  /// The refusal shown, once the sign-in that got it ended.
  pub(crate) fn refusal(&self) -> Option<Refusal> {
    let state = self.sign_in.state().get();
    let pending = state.is_pending();
    state.error.filter(|_| !pending)
  }

  /// Whether this email's sign-in is paused now.
  fn paused(&self) -> bool {
    self.tick.get();
    let refusal = self.refusal();
    if !matches!(refusal, Some(Refusal::Paused { .. })) {
      return false;
    }
    let paused = paused_now(refusal.as_ref(), &self.email.get());
    if paused {
      self.pause_look.start();
    } else {
      self.pause_look.stop();
    }
    paused
  }

  /// Signs in with what was typed, for *Sign in* and for Enter in either field alike. Nothing happens while a sign-in
  /// is under way or this email's sign-in is paused; an empty field takes the focus instead of a sign-in; synthetic
  /// input is refused, and the sign-in says so under *Sign in*.
  pub(crate) fn submitter(&self) -> impl Fn() + Send + Sync + 'static {
    let (action, email, password, person, focus) = (
      self.sign_in.clone(),
      self.email.clone(),
      self.password.clone(),
      self.person.clone(),
      self.focus.clone(),
    );
    let (step, submit) = (self.step.clone(), self.submit);
    move || {
      let state = action.state().get_untracked();
      let (typed, secret) = (email.get_untracked(), password.get_untracked());
      if state.is_pending() || paused_now(state.error.as_ref(), &typed) {
        return;
      }
      let empty = if typed.trim().is_empty() {
        Some(Field::Email)
      } else {
        secret.is_empty().then_some(Field::Password)
      };
      if let Some(field) = empty {
        focus.set(Some(field));
        return;
      }
      if person.allows(submit) {
        step.set(Step::Password { timed_out: false });
        action.run_if_idle((typed, secret));
      }
    }
  }

  /// Moves the focus to the field asked for, once.
  pub(crate) fn take_focus(&self, ctx: &Ctx) {
    let Some(field) = self.focus.get() else {
      return;
    };
    self.focus.set(None);
    ctx.focus(match field {
      Field::Email => &self.email_input,
      Field::Password => &self.password_input,
    });
  }
}

/// Whether `refusal` paused the sign-in of `email` until a time still to come.
fn paused_now(refusal: Option<&Refusal>, email: &str) -> bool {
  matches!(refusal, Some(Refusal::Paused { email: paused, until }) if paused == email.trim() && Timestamp::now() < *until)
}

/// The fields on the cloud whose host is `host`, 400 wide, and *Create an account* under them when `create` asks for
/// it.
pub(super) fn fields(ctx: &mut Ctx, form: &Form, (host, create): (&str, bool)) -> Element {
  form.take_focus(ctx);
  let pending = form.pending();
  let refusal = form.refusal();
  // Always inside the same row, so a field keeps its input, and the focus, while the sign-in fades it. The row lets
  // *Forgot password?*'s ring show past its edge (`Focus::Link`).
  let faded = |part: Element| -> Element {
    let row = Row::new().width(Dimension::Pct(100.0)).overflow_visible().child(part);
    if pending { row.opacity(DISABLED_OPACITY) } else { row }.into()
  };
  let (email, password) = inputs(ctx, form, refusal == Some(Refusal::Wrong));
  let checking = pending.then(|| {
    Text::new(&ctx.t_args("sign_in.checking", [("host", host)]))
      .id("sign-in-checking")
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
  });
  let created = create.then(|| faded(create_row(ctx, form)));
  let column = Column::new()
    .id("sign-in-form")
    .width(Dimension::Px(WIDTH))
    .max_width(Dimension::Pct(100.0))
    .spacing(OrchesterSpacing::FormGap.role())
    .child(faded(email))
    .child(faded(password))
    .with_children(refusal.as_ref().map(|refusal| refusal_part(ctx, form, (refusal, host))))
    .child(submit_button(ctx, form).after(Icon::ArrowRight).large())
    .with_children(checking)
    .with_children(form.person.reason(ctx, form.submit))
    .with_children(unopened(ctx, form))
    .with_children(created);
  let submit = form.submitter();
  LurqForm::element(FormProps::default().on_submit_data(move |_| submit()), column)
}

/// *Sign in*, as wide as the fields, *Signing in…* while it does, faded while this email's sign-in is paused.
pub(crate) fn submit_button(ctx: &mut Ctx, form: &Form) -> ActionButton {
  if form.pending() {
    return ActionButton::new(form.submit, ButtonKind::Primary, ctx.t("sign_in.signingIn"))
      .fill_width()
      .loading();
  }
  let button = ActionButton::new(form.submit, ButtonKind::Primary, ctx.t("sign_in.submit")).fill_width();
  if form.paused() {
    return button.disabled();
  }
  button.in_form().on_click(form.submitter())
}

/// The email, and the password with *Forgot password?* over it; both marked after a wrong one.
fn inputs(ctx: &mut Ctx, form: &Form, wrong: bool) -> (Element, Element) {
  let mut email = FieldInput::new("sign-in-email", ctx.t("sign_in.email"), form.email.clone())
    .placeholder(ctx.t("sign_in.emailPlaceholder"))
    .leading(Icon::Mail)
    .box_height(ControlHeight::Lg.value())
    .element(form.email_input.clone())
    .tab_first(EMAIL_TAB);
  let mut password = FieldInput::new("sign-in-password", ctx.t("sign_in.password"), form.password.clone())
    .placeholder(ctx.t("sign_in.passwordPlaceholder"))
    .masked()
    .no_trailing()
    .leading(Icon::Lock)
    .box_height(ControlHeight::Lg.value())
    .element(form.password_input.clone())
    .tab_first(PASSWORD_TAB);
  if wrong {
    email = email.invalid();
    password = password.invalid();
  }
  let forgot = forgot_link(ctx, form);
  let label = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PASSWORD_LABEL_HEIGHT))
    // *Forgot password?* ends where the row does; its ring shows past the row's edge (`Focus::Link`).
    .overflow_visible()
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("sign_in.password")).variant(TypographyStyle::FieldLabel))
    .child(Spacer::new().flex(1.0))
    .child(forgot);
  let password = Column::new()
    .id("sign-in-password-field")
    .width(Dimension::Pct(100.0))
    .overflow_visible()
    .spacing(ExtraSpacing::Snug.role())
    .child(label)
    .child(password.render(ctx));
  let email = Column::new()
    .id("sign-in-email-field")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&ctx.t("sign_in.email"))
        .id("sign-in-email-field-label")
        .variant(TypographyStyle::FieldLabel)
        .height(Dimension::Px(LABEL_HEIGHT)),
    )
    .child(email.render(ctx));
  (email.into(), password.into())
}

/// The refusal in place: the fields' error, or a callout.
pub(crate) fn refusal_part(ctx: &mut Ctx, form: &Form, (refusal, host): (&Refusal, &str)) -> Element {
  match refusal {
    Refusal::Wrong => field_error("sign-in-wrong", &ctx.t("sign_in.wrong")).into(),
    Refusal::Unconfirmed(email) => Callout::new(
      "sign-in-unconfirmed",
      CalloutTone::Warning,
      ctx.t("sign_in.unconfirmed.title"),
    )
    .icon(Icon::MailWarning)
    .body(ctx.t_args("sign_in.unconfirmed.body", [("email", email.as_str())]))
    .action(page_button(
      ctx,
      "sign-in-open-cloud",
      &ctx.t("sign_in.unconfirmed.open"),
      (form, WebPage::SignIn),
    ))
    .into(),
    Refusal::Paused { until, .. } => Callout::new("sign-in-paused", CalloutTone::Danger, ctx.t("sign_in.paused.title"))
      .icon(Icon::Clock)
      .body(ctx.t_args("sign_in.paused.body", [("time", display::clock(*until))]))
      .into(),
    Refusal::Unreachable(at) => Callout::new(
      "sign-in-unreachable",
      CalloutTone::Neutral,
      ctx.t("sign_in.unreachable.title"),
    )
    .icon(Icon::WifiOff)
    .body(ctx.t("sign_in.unreachable.body"))
    .note(ctx.t_args(
      "sign_in.unreachable.when",
      [("host", host.to_owned()), ("time", display::clock(*at))],
    ))
    .into(),
    Refusal::Other(said) => field_error("sign-in-failed", &said.text(ctx)).into(),
  }
}

/// Why the cloud's page did not open in the browser.
pub(crate) fn unopened(ctx: &Ctx, form: &Form) -> Option<Element> {
  let page = form.unopened.get()?;
  Some(
    field_error(
      "sign-in-open-failed",
      &ctx.t_args("sign_in.openFailed", [("page", page.as_str())]),
    )
    .into(),
  )
}

/// *New to Orchester?* with *Create an account* ↗.
fn create_row(ctx: &mut Ctx, form: &Form) -> Element {
  let label = ctx.t("sign_in.create");
  let link = Focus::Link
    .apply(pressable(opener(ctx, (form, WebPage::SignUp))))
    .id("sign-in-create-account")
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", label.to_string())
    .child(
      Text::new(&label)
        .variant(ExtraTypography::NoteStrong.role())
        .color(PaletteColor::TextPrimary),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextPrimary));
  Row::new()
    .id("sign-in-create")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("sign_in.newHere")).variant(ExtraTypography::Lead.role()))
    .child(link)
    .into()
}

/// Opens `page` of the cloud in the browser; says so in place when it cannot.
pub(crate) fn opener(ctx: &mut Ctx, (form, page): (&Form, WebPage)) -> impl Fn() + Send + Sync + 'static {
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
    .in_form()
    .on_click(opener(ctx, target))
    .into()
}

/// *Forgot password?*, a quiet link and a stop of the form's Tab order.
fn forgot_link(ctx: &mut Ctx, form: &Form) -> Element {
  let label = ctx.t("sign_in.forgot");
  Focus::Link
    .apply(pressable(opener(ctx, (form, WebPage::Forgot))))
    .id("sign-in-forgot")
    .align_items(Alignment::Center)
    .describe("a11y_name", label.to_string())
    .child(
      Text::new(&label)
        .variant(ExtraTypography::NoteStrong.role())
        .color(PaletteColor::TextSecondary),
    )
    .into()
}
