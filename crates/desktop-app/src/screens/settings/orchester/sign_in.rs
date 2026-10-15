//! Signed out (`Cloud details` `alsg7` in `DUWM5`): *Sign in to Orchester Cloud* with the account's email and password
//! through the cloud's API; *Forgot password?* and *Create an account* open the cloud's web pages in the browser, where
//! the email is confirmed; the refusals and the browser's notice in place (`Sign-in · errors and the browser` in
//! `Aowlk`); *Connect a server with a code* for a server without the internet; the advanced cloud address.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_api::{Credential, cloud::CloudErrorCode};
use orchester_workspace::sharing::cloud::{CloudAddress, CloudError, WebPage};

use super::{
  Said, advanced,
  parts::{frame, header, hint, paragraph},
  words::{Look, cloud_error},
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  services::Services,
  theme::{AccountsSpacing, ExtraTypography, OrchesterSpacing, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    pressable::pressable,
  },
};

/// The form's width (`Ay4fx`).
const FORM_WIDTH: f32 = 420.0;

/// Why the cloud did not sign this computer in, as the form says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
  /// A wrong email or password: both fields are marked.
  Wrong,
  /// *Confirm your email first*, with *Open Orchester Cloud*.
  Unconfirmed,
  /// *Too many attempts*, for this many minutes.
  Paused(u64),
  /// *Orchester Cloud can't be reached*.
  Unreachable,
  Other(Said),
}

impl DevtoolsInspectable for Refusal {}

impl Refusal {
  fn of(error: &CloudError) -> Self {
    match error.cloud_code() {
      Some(CloudErrorCode::InvalidCredentials) => Self::Wrong,
      Some(CloudErrorCode::EmailUnconfirmed) => Self::Unconfirmed,
      Some(CloudErrorCode::RateLimited) => {
        Self::Paused(error.retry_after().map_or(15, |seconds| seconds.div_ceil(60).max(1)))
      }
      _ if error.unreachable() => Self::Unreachable,
      _ => Self::Other(cloud_error(error)),
    }
  }
}

/// A page of the cloud the form opened in the browser, or why it could not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Opened {
  None,
  Page(String),
  Failed(String),
}

impl DevtoolsInspectable for Opened {}

/// The form's state: what was typed, the sign-in under way, and the page it opened.
pub(super) struct Form {
  email: Signal<String>,
  /// Made outside the component's context, so DevTools never records it ([`FieldInput::masked`]).
  password: Signal<String>,
  sign_in: FutureAction<(String, String), (), Refusal>,
  opened: Signal<Opened>,
}

impl Form {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let cloud = Services::of(ctx).cloud;
    let password = Signal::new(String::new());
    let clear = password.clone();
    Self {
      email: ctx.signal(String::new()),
      password,
      sign_in: ctx.future_action(move |(email, password): (String, String)| {
        let (cloud, clear) = (cloud.clone(), clear.clone());
        async move {
          let signed_in = cloud.sign_in(email.trim().to_owned(), Credential::new(password)).await;
          clear.set(String::new());
          signed_in.map(drop).map_err(|error| {
            tracing::info!(%error, "Orchester Cloud did not sign this computer in");
            Refusal::of(&error)
          })
        }
      }),
      opened: ctx.signal(Opened::None),
    }
  }
}

/// The panel while nobody is signed in to the cloud.
pub(super) fn panel(ctx: &mut Ctx, address: &CloudAddress, form: &Form, advanced: &advanced::Advanced) -> Element {
  let look = Look {
    icon: Icon::CircleDashed,
    icon_color: PaletteColor::TextMuted,
    key: "cloud.state.notSignedIn",
    color: PaletteColor::TextMuted,
    name: "signed-out",
  };
  let facts = [address.address.authority()];
  let head = header(
    ctx,
    "orchester-cloud-header",
    (Icon::Cloud, &ctx.t("cloud.list.cloud"), &look),
    &facts,
  );
  let navigation = Navigation::of(ctx);
  let body = Column::new()
    .id("orchester-sign-in-body")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(OrchesterSpacing::FormTop.role())
        .right(AccountsSpacing::Panel.role())
        .bottom(OrchesterSpacing::FormGap.role())
        .left(OrchesterSpacing::FormSide.role()),
    )
    .spacing(AccountsSpacing::Panel.role())
    .child(sign_in_form(ctx, form, &address.address.authority()))
    .child(divider())
    .child(without_internet(ctx, &navigation))
    .child(Spacer::new().flex(1.0))
    .child(advanced::section(ctx, address, advanced));
  frame("orchester-cloud-details").child(head).child(body).into()
}

fn sign_in_form(ctx: &mut Ctx, form: &Form, host: &str) -> Element {
  let state = form.sign_in.state().get();
  let refusal = state.error.clone().filter(|_| !state.is_pending());
  let (email, password) = fields(ctx, form, refusal == Some(Refusal::Wrong));
  let submit = {
    let (action, email, password) = (form.sign_in.clone(), form.email.clone(), form.password.clone());
    let button = ActionButton::new("orchester-sign-in", ButtonKind::Primary, ctx.t("cloud.signIn.submit"))
      .icon(Icon::LogIn)
      .fill_width();
    if state.is_pending() {
      button.loading()
    } else {
      button.on_click(move || action.run((email.get(), password.get())))
    }
  };
  let opened = form.opened.get();
  let web_hint = ctx.t_args("cloud.signIn.webHint", [("host", host)]);
  Column::new()
    .id("orchester-sign-in-form")
    .width(Dimension::Px(FORM_WIDTH))
    .spacing(OrchesterSpacing::FormGap.role())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(Text::new(&ctx.t("cloud.signIn.title")).variant(OrchesterText::FormTitle.role()))
        .child(paragraph("orchester-sign-in-lead", &ctx.t("cloud.signIn.lead"))),
    )
    .child(email)
    .child(password)
    .with_children(refusal.as_ref().map(|refusal| refusal_part(ctx, form, refusal)))
    .child(submit)
    .with_children(opened_part(ctx, &opened))
    .child(divider())
    .child(create_row(ctx, form))
    .child(hint("orchester-sign-in-web-hint", &web_hint))
    .into()
}

/// The email and the password with *Forgot password?* over it, both marked after a wrong one.
fn fields(ctx: &mut Ctx, form: &Form, wrong: bool) -> (Element, Element) {
  let mut email = FieldInput::new(
    "orchester-sign-in-email",
    ctx.t("cloud.signIn.email"),
    form.email.clone(),
  )
  .placeholder(ctx.t("cloud.signIn.emailPlaceholder"));
  let mut password = FieldInput::new(
    "orchester-sign-in-password",
    ctx.t("cloud.signIn.password"),
    form.password.clone(),
  )
  .masked();
  if wrong {
    email = email.invalid();
    password = password.invalid();
  }
  let forgot = page_link(
    ctx,
    "orchester-forgot",
    &ctx.t("cloud.signIn.forgot"),
    (form, WebPage::Forgot),
  );
  let label = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("cloud.signIn.password")).variant(TypographyStyle::FieldLabel))
    .child(Spacer::new().flex(1.0))
    .child(forgot);
  let password = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .child(label)
    .child(password.render(ctx));
  let email = field(
    "orchester-sign-in-email-field",
    Some(&ctx.t("cloud.signIn.email")),
    email.render(ctx),
    None,
  );
  (email.into(), password.into())
}

/// *New to Orchester?* with *Create an account*.
fn create_row(ctx: &mut Ctx, form: &Form) -> Element {
  let create = page_button(
    ctx,
    "orchester-create-account",
    &ctx.t("cloud.signIn.create"),
    (form, WebPage::SignUp),
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("cloud.signIn.newHere")).variant(ExtraTypography::Lead.role()))
    .child(Spacer::new().flex(1.0))
    .child(create)
    .into()
}

/// The refusal in place: the fields' error, or a notice.
fn refusal_part(ctx: &mut Ctx, form: &Form, refusal: &Refusal) -> Element {
  match refusal {
    Refusal::Wrong => field_error("orchester-sign-in-wrong", &ctx.t("cloud.signIn.wrong")).into(),
    Refusal::Unconfirmed => {
      let email = form.email.get();
      Notice::new(
        "orchester-sign-in-unconfirmed",
        NoticeTone::Warning,
        ctx.t("cloud.signIn.unconfirmed.title"),
      )
      .icon(Icon::Mail)
      .body(ctx.t_args("cloud.signIn.unconfirmed.body", [("email", email.trim())]))
      .action(page_button(
        ctx,
        "orchester-sign-in-open-cloud",
        &ctx.t("cloud.signIn.unconfirmed.open"),
        (form, WebPage::SignIn),
      ))
      .into()
    }
    Refusal::Paused(minutes) => Notice::new(
      "orchester-sign-in-paused",
      NoticeTone::Danger,
      ctx.t("cloud.signIn.paused.title"),
    )
    .icon(Icon::CircleHelp)
    .body(ctx.t_args("cloud.signIn.paused.body", [("minutes", minutes.to_string())]))
    .into(),
    Refusal::Unreachable => Notice::new(
      "orchester-sign-in-unreachable",
      NoticeTone::Info,
      ctx.t("cloud.signIn.unreachable.title"),
    )
    .icon(Icon::WifiOff)
    .body(ctx.t("cloud.signIn.unreachable.body"))
    .into(),
    Refusal::Other(said) => field_error("orchester-sign-in-failed", &said.text(ctx)).into(),
  }
}

/// *Opened in your browser* with the page, or why it could not open.
fn opened_part(ctx: &Ctx, opened: &Opened) -> Option<Element> {
  match opened {
    Opened::None => None,
    Opened::Page(url) => Some(
      Notice::new("orchester-opened", NoticeTone::Info, ctx.t("cloud.signIn.opened.title"))
        .icon(Icon::ExternalLink)
        .body(ctx.t_args("cloud.signIn.opened.body", [("page", url.as_str())]))
        .into(),
    ),
    Opened::Failed(url) => Some(
      field_error(
        "orchester-open-failed",
        &ctx.t_args("cloud.signIn.openFailed", [("page", url.as_str())]),
      )
      .into(),
    ),
  }
}

/// Opens `page` of the cloud in the browser and says so in place.
fn opener(ctx: &mut Ctx, (form, page): (&Form, WebPage)) -> impl Fn() + Send + Sync + 'static {
  let services = Services::of(ctx);
  let (cloud, links, opened) = (services.cloud, services.links, form.opened.clone());
  move || {
    let url = match cloud.web_page(page) {
      Ok(url) => url,
      Err(error) => {
        tracing::warn!(%error, "the cloud's page has no address");
        return;
      }
    };
    match links.open(&url) {
      Ok(()) => opened.set(Opened::Page(url)),
      Err(error) => {
        tracing::warn!(%error, "the cloud's page could not be opened");
        opened.set(Opened::Failed(url));
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

/// A quiet link with the external glyph after it (`Link · Forgot password?` `Khtmv`).
fn page_link(ctx: &mut Ctx, id: &str, label: &str, target: (&Form, WebPage)) -> Element {
  pressable(opener(ctx, target))
    .id(id.to_owned())
    .spacing(crate::theme::ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", label.to_owned())
    .child(
      Text::new(label)
        .variant(ExtraTypography::NoteStrong.role())
        .color(PaletteColor::TextSecondary),
    )
    .child(icon(
      Icon::ExternalLink,
      crate::theme::IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .into()
}

fn divider() -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(0.0))
    .border_top(lurq::node::border::Border::inside(BorderSize::Sm, PaletteColor::Border))
    .into()
}

/// *No internet where your server runs?* (`MgxXb`).
fn without_internet(ctx: &Ctx, navigation: &Navigation) -> Element {
  let navigation = navigation.clone();
  Column::new()
    .id("orchester-without-internet")
    .width(Dimension::Px(FORM_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("cloud.signIn.offline.title")).variant(ExtraTypography::FormSection.role()))
    .child(paragraph(
      "orchester-without-internet-text",
      &ctx.t("cloud.signIn.offline.body"),
    ))
    .child(
      Row::new().child(
        ActionButton::new(
          "orchester-connect-with-code",
          ButtonKind::Secondary,
          ctx.t("cloud.signIn.offline.connect"),
        )
        .icon(Icon::Server)
        .on_click(move || navigation.go(Route::Orchester(OrchesterView::connect()))),
      ),
    )
    .into()
}
