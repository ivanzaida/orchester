//! The Orchester Cloud sign-in, in place where it is asked (board *Signed out · redesign* `uBND2`; the person's
//! decision, 2026-10-28: a sign-in is needed only for team features, and nothing locks without one). [`SignInOffer`]
//! holds it: in a card with *Not signed in*, or *Sign in again* and why the last one ended (`j7SbPk`, `CBmxe`), in
//! Settings › Orchester Cloud and the invite landing (`zoihT`); or the fields alone under a row (`i5lddT`). For an
//! account with two-factor authentication, the code step ([`code`]) takes the fields' place in the same card (`eWsUb`).
//! What asks follows the sign-in (`crate::root::follow_sign_in`) and shows what it unlocks once it is made, with no
//! navigation.

pub(crate) mod code;
mod ended;
pub(crate) mod form;
pub(crate) mod step;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_store::EndedCloudSignIn;
use orchester_workspace::{
  background,
  sharing::cloud::{Cloud, SignIn},
};

use self::step::Step;
use crate::{
  services::Services,
  theme::{ExtraSpacing, SignInText},
};

/// The sign-in's fields, and the code's boxes (`j7SbPk`, `eWsUb`).
pub(crate) const WIDTH: f32 = 400.0;

/// How the sign-in is shown where it is asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  /// In a card of its own, with its title and what signing in is for: Settings › Orchester Cloud, the invite landing.
  Card,
  /// The fields alone, under what asked for them: a row of Sharing.
  Under,
}

impl DevtoolsInspectable for Place {}

/// The sign-in where it is asked. Its id is that of its card.
pub struct SignInOffer {
  form: form::Form,
  code: code::CodeForm,
  /// Made outside the component's context, so DevTools never records the challenge it holds.
  step: Signal<Step>,
}

impl Component for SignInOffer {
  type Props = Place;

  fn create(ctx: &mut Ctx) -> Self {
    let cloud = Services::of(ctx).cloud;
    let email = ended(&cloud).map_or_else(String::new, |ended| ended.email);
    let step = Signal::new(Step::Password { timed_out: false });
    let form = form::Form::new(ctx, &email, step.clone(), form::SUBMIT);
    let code = code::CodeForm::new(ctx, step.clone(), form.to_password());
    Self { form, code, step }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let place = *ctx.props::<Place>();
    let cloud = Services::of(ctx).cloud;
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let shown_host = shown_host(ctx, &cloud);
    let ended = ended(&cloud);
    let in_card = place == Place::Card;
    let mut column = Column::new()
      .id("sign-in-card")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role());
    if in_card {
      column = column
        .padding(SpacingSize::Md)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Lg);
    }
    let step = self.step.get();
    if let Step::Code(entry) = &step {
      return column.child(code::step(ctx, &self.code, entry, &shown_host));
    }
    self.code.rest();
    if in_card {
      column = column.with_children(head(ctx, ended.as_ref()));
    }
    let notice = if step == (Step::Password { timed_out: true }) {
      Some(ended::timed_out(ctx))
    } else {
      ended
        .as_ref()
        .filter(|_| in_card)
        .map(|ended| ended::notice(ctx, ended))
    };
    column
      .with_children(notice)
      .child(form::fields(ctx, &self.form, (&shown_host, in_card && ended.is_none())))
  }
}

/// The host of the cloud in use, for a future's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Host(String);

impl DevtoolsInspectable for Host {}

/// The cloud's host, read off the UI thread and again whenever it changes in *Advanced*; empty until it is read.
fn shown_host(ctx: &mut Ctx, cloud: &Cloud) -> String {
  let revision = ctx
    .stream(cloud.clone(), |cloud, emitter: StreamEmitter<u64, ()>| async move {
      let mut changes = cloud.subscribe();
      loop {
        let count = *changes.borrow_and_update();
        if !emitter.emit(count) || changes.changed().await.is_err() {
          return;
        }
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default();
  ctx
    .future((revision, cloud.clone()), |(_, cloud)| async move {
      background::run(move || Host(host(&cloud))).await.map_err(drop)
    })
    .state()
    .get()
    .data
    .map_or_else(String::new, |Host(host)| host)
}

/// *Not signed in* with what signing in is for; *Sign in again* once a sign-in ended here, whose notice says why.
fn head(ctx: &Ctx, ended: Option<&EndedCloudSignIn>) -> Vec<Element> {
  if ended.is_some() {
    return vec![title(ctx, "sign-in-title", "sign_in.again")];
  }
  vec![
    title(ctx, "sign-in-title", "sign_in.notSignedIn"),
    Text::new(&ctx.t("sign_in.notSignedInLead"))
      .id("sign-in-lead")
      .variant(TypographyStyle::Description)
      .width(Dimension::Pct(100.0))
      .into(),
  ]
}

/// A title over the sign-in, by its key.
pub(crate) fn title(ctx: &Ctx, id: &str, key: &str) -> Element {
  Text::new(&ctx.t(key))
    .id(id.to_owned())
    .variant(SignInText::Title.role())
    .into()
}

/// Why the last sign-in from here ended, while signed out.
fn ended(cloud: &Cloud) -> Option<EndedCloudSignIn> {
  match cloud.sign_in_state() {
    SignIn::SignedOut { ended, .. } => ended,
    SignIn::SignedIn { .. } => None,
  }
}

/// The cloud's host, as the progress line and the refusals name it.
pub(crate) fn host(cloud: &Cloud) -> String {
  match cloud.address() {
    Ok(address) => address.address.web().authority(),
    Err(error) => {
      // Only the words while it signs in and a refusal's note name the host; signing in reads the address itself.
      tracing::warn!(%error, "the cloud's address could not be read for the sign-in");
      String::new()
    }
  }
}
