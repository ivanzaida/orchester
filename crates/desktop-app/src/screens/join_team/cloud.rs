//! The Orchester Cloud way (`With Orchester Cloud` in `B914L`). Signed out, its sign-in: the email, the password
//! with its lock, *Sign in*, and *No account yet? Create one ↗*, which opens the cloud's sign-up page in the browser.
//! It signs in as the sign-in gate does ([`crate::screens::gate::form`]), with the gate's words and refusals in
//! place; while it signs in the fields fade and the button says so (`BD519`). An account with two-factor
//! authentication goes on to the gate's code step in the column's place. Signed in, the account the step goes on
//! with, and *Your teams* back to its teams and invites ([`account_row`] is the 04 account row, `uNGRZ`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Form as LurqForm, FormProps, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::CloudAccountId;
use orchester_workspace::{
  background,
  sharing::cloud::{Cloud, WebPage},
};

use super::ways;
use crate::{
  navigation::Navigation,
  routes::{JoinWay, Route},
  screens::{
    gate::{
      code::{self as gate_code, CodeForm},
      form::{Form, Refusal, opener, refusal_part, submit_button, unopened},
      step::Step,
    },
    settings::orchester::Account,
  },
  services::Services,
  theme::{ControlHeight, DISABLED_OPACITY, ExtraSpacing, ExtraTypography, IconSize, StartupSpacing, StartupText},
  ui::{
    avatar::{initials, person_avatar},
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field},
    icon::{Icon, icon},
    pressable::{Focus, pressable},
  },
};

const ID: &str = "join-team-cloud";
/// *Sign in*, which only a person presses.
const SUBMIT: &str = "join-team-sign-in";

/// The sign-in's state: the gate's password form and its code step, on the cloud whose host is `host`.
pub(super) struct CloudSignIn {
  form: Form,
  code: CodeForm,
  /// Made outside the component's context, so DevTools never records the challenge it holds.
  step: Signal<Step>,
  host: String,
}

impl CloudSignIn {
  /// The sign-in, its email filled in with `email` (the account whose sign-in ended here).
  pub(super) fn new(ctx: &mut Ctx, email: &str) -> Self {
    let step = Signal::new(Step::Password { timed_out: false });
    let form = Form::new(ctx, email, step.clone(), SUBMIT);
    let code = CodeForm::new(ctx, step.clone(), form.to_password());
    Self {
      form,
      code,
      step,
      host: host(&Services::of(ctx).cloud),
    }
  }

  /// Whether a sign-in is under way: the code column waits, faded, meanwhile.
  pub(super) fn pending(&self) -> bool {
    self.form.pending()
  }
}

/// The cloud's host, as the progress line and the refusals name it.
fn host(cloud: &Cloud) -> String {
  match cloud.address() {
    Ok(address) => address.address.web().authority(),
    Err(error) => {
      // Only the words while it signs in and a refusal's note name the host; signing in reads the address itself.
      tracing::warn!(%error, "the cloud's address could not be read for Join a team");
      String::new()
    }
  }
}

/// The column signed out: the sign-in, or the gate's code step in its place.
pub(super) fn signed_out(ctx: &mut Ctx, sign_in: &CloudSignIn) -> Element {
  if let Step::Code(entry) = sign_in.step.get() {
    return gate_code::card(ctx, &sign_in.code, &entry, &sign_in.host);
  }
  sign_in.code.rest();
  let form = &sign_in.form;
  form.take_focus(ctx);
  let (pending, refusal) = (form.pending(), form.refusal());
  // Always inside the same row, so a field keeps its input, and the focus, while the sign-in fades it.
  let faded = |part: Element| -> Element {
    let row = Row::new().width(Dimension::Pct(100.0)).child(part);
    if pending { row.opacity(DISABLED_OPACITY) } else { row }.into()
  };
  let (email, password) = fields(ctx, form, refusal == Some(Refusal::Wrong));
  let head = ways::head(
    ID,
    Icon::Cloud,
    (&ctx.t("startup.join.cloud.title"), &ctx.t("startup.join.cloud.sub")),
  );
  let mut parts = vec![faded(email), faded(password)];
  parts.extend(
    refusal
      .as_ref()
      .map(|refusal| refusal_part(ctx, form, (refusal, &sign_in.host))),
  );
  parts.push(submit_button(ctx, form).large().into());
  parts.extend(form.person.reason(ctx, form.submit));
  parts.extend(unopened(ctx, form));
  parts.push(faded(create(ctx, form)));
  let submit = form.submitter();
  LurqForm::element(
    FormProps::default().on_submit_data(move |_| submit()),
    ways::way(ID, head, parts),
  )
}

/// The email and the password, 36 high, the password behind its lock; both marked after a wrong one.
fn fields(ctx: &mut Ctx, form: &Form, wrong: bool) -> (Element, Element) {
  let (email_label, password_label) = (ctx.t("gate.email"), ctx.t("gate.password"));
  let mut email = FieldInput::new(format!("{ID}-email"), email_label.clone(), form.email.clone())
    .placeholder(ctx.t("gate.emailPlaceholder"))
    .text_style(TypographyStyle::Body)
    .box_height(ControlHeight::Lg.value())
    .element(form.email_input.clone());
  let mut password = FieldInput::new(format!("{ID}-password"), password_label.clone(), form.password.clone())
    .masked()
    .no_trailing()
    .leading(Icon::Lock)
    .placeholder(ctx.t("startup.join.cloud.passwordPlaceholder"))
    .text_style(TypographyStyle::Body)
    .box_height(ControlHeight::Lg.value())
    .element(form.password_input.clone());
  if wrong {
    email = email.invalid();
    password = password.invalid();
  }
  let email = field(
    &format!("{ID}-email-field"),
    Some(&email_label),
    email.render(ctx),
    None,
  );
  let password = field(
    &format!("{ID}-password-field"),
    Some(&password_label),
    password.render(ctx),
    None,
  );
  (email.into(), password.into())
}

/// *No account yet?* and *Create one ↗*, which opens the cloud's sign-up page in the browser.
fn create(ctx: &mut Ctx, form: &Form) -> Element {
  let label = ctx.t("startup.join.cloud.create");
  let link = Focus::Link
    .apply(pressable(opener(ctx, (form, WebPage::SignUp))))
    .id(format!("{ID}-create"))
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", label.to_string())
    .child(Text::new(&label).variant(StartupText::Link.role()))
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextPrimary));
  Row::new()
    .id(format!("{ID}-no-account"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("startup.join.cloud.noAccount")).variant(StartupText::Aside.role()))
    .child(link)
    .into()
}

/// The column signed in to the account of `email` (`account`): the account, and *Your teams*.
pub(super) fn signed_in(ctx: &mut Ctx, email: &str, account: CloudAccountId) -> Element {
  let head = ways::head(
    ID,
    Icon::Cloud,
    (&ctx.t("startup.join.cloud.title"), &ctx.t("startup.join.cloud.sub")),
  );
  let row = account_row(ctx, (email, account), None);
  let teams = ActionButton::new(
    format!("{ID}-your-teams"),
    ButtonKind::Secondary,
    ctx.t("startup.join.cloud.yourTeams"),
  )
  .icon(Icon::Users)
  .opens(Navigation::of(ctx).to(Route::JoinTeam(JoinWay::Cloud)));
  ways::way(ID, head, vec![row, Row::new().child(teams).into()])
}

/// The account signed in (`uNGRZ`): its initials, its email, *· signed in with Orchester Cloud*, and `end` at the
/// row's end.
pub(super) fn account_row(ctx: &mut Ctx, (email, account): (&str, CloudAccountId), end: Option<Element>) -> Element {
  let name = display_name(ctx, account).unwrap_or_else(|| email.to_owned());
  Row::new()
    .id("join-team-account")
    .width(Dimension::Pct(100.0))
    .spacing(StartupSpacing::Row.role())
    .align_items(Alignment::Center)
    .child(person_avatar("join-team-account-avatar", &initials(&name)))
    .child(
      Text::new(email)
        .id("join-team-account-email")
        .variant(ExtraTypography::Strong.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("startup.join.cloud.signedIn"))
        .variant(StartupText::Aside.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(end)
    .into()
}

/// The display name of `account`, read off the UI thread; `None` until it is read, or when it cannot be (the initials
/// are the email's then).
fn display_name(ctx: &mut Ctx, account: CloudAccountId) -> Option<String> {
  let cloud = Services::of(ctx).cloud;
  let read = ctx
    .future((account, cloud), |(_, cloud)| async move {
      background::run(move || cloud.account())
        .await
        .map_err(|message| tracing::warn!(?message, "the cloud account's read did not finish"))?
        .map(Account)
        .map_err(|error| tracing::warn!(%error, "the cloud account could not be read for Join a team"))
    })
    .state()
    .get();
  read
    .data
    .and_then(|Account(account)| account)
    .map(|account| account.display_name)
}
