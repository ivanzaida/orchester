//! The sign-in gate (layer *Sign-in gate* `ASl21`; the person's decision, 2026-10-16): until this computer is signed
//! in to Orchester Cloud, the window shows only this, under a title bar with the mark alone and over a status bar with
//! the MCP server refusing (`crate::shell::status_bar::gate`). In the content panel: the gate's head (*Sign in to
//! start* on a first run `Lpznn`, *Sign in again* when the gate came back `ztigw`), why it came back ([`ended`]), the
//! sign-in card ([`form`]) and *Advanced · cloud address* (`S8aPFV`), centered; the footer under them says the
//! internet is needed once (a first run, or signed out to sign in with a code `Nq3Mf`) or that only a person signs in
//! (the gate came back), and names the cloud. For an account with two-factor authentication, the code step
//! ([`code`]) takes the card's place, under a head of its own and without *Advanced* (`Tmwkg`). The same layout at 1440
//! and 960; the footer's words are shorter in a narrow window.

mod code;
mod ended;
mod form;
mod step;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{BorderSize, Breakpoint, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_store::{CloudSignInEnd, EndedCloudSignIn};
use orchester_workspace::{
  background,
  sharing::cloud::{Cloud, CloudInUse, SignIn},
};

use self::{ended::Head, step::Step};
use crate::{
  screens::settings::orchester::{Advanced, Place, advanced_section},
  services::Services,
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, GateSpacing, IconSize},
  ui::{
    field::field_error,
    icon::{Icon, icon},
    skeleton::{self, Skeleton},
  },
};

/// The gate's column (`Gate column` `K4UfUq`).
const COLUMN_WIDTH: f32 = 400.0;
/// The footer under the gate (`Footer` `BFbUg`).
const FOOTER_HEIGHT: f32 = 56.0;

/// What the gate shows: why this computer is signed out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateProps {
  pub sign_in: SignIn,
}

impl DevtoolsInspectable for GateProps {}

/// The gate in the content panel.
pub struct SignInGate {
  form: form::Form,
  code: code::CodeForm,
  advanced: Advanced,
  /// Made outside the component's context, so DevTools never records the challenge it holds.
  step: Signal<Step>,
}

impl Component for SignInGate {
  type Props = GateProps;

  fn create(ctx: &mut Ctx) -> Self {
    let email = match &ctx.props::<GateProps>().sign_in {
      SignIn::SignedOut { ended: Some(ended), .. } => ended.email.clone(),
      SignIn::SignedOut { ended: None, .. } | SignIn::SignedIn { .. } => String::new(),
    };
    let step = Signal::new(Step::Password { timed_out: false });
    let form = form::Form::new(ctx, &email, step.clone());
    let code = code::CodeForm::new(ctx, step.clone(), form.to_password());
    Self {
      form,
      code,
      advanced: Advanced::new(ctx),
      step,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let GateProps { sign_in } = ctx.props::<GateProps>().clone();
    let ended = match &sign_in {
      SignIn::SignedOut { ended, .. } => ended.clone(),
      SignIn::SignedIn { .. } => None,
    };
    let cloud = Services::of(ctx).cloud;
    let address = match address(ctx, &cloud) {
      Ok(Some(address)) => address,
      Ok(None) => {
        // No loading frame draws the gate: its form's lines are bones until the cloud's address is read.
        let bones = skeleton::lines(ctx, &[220.0, 360.0, 300.0]);
        return panel(
          Skeleton::new("gate-reading", ctx.t("cloud.reading")).show(ctx, bones),
          None,
        );
      }
      Err(()) => {
        let failed = field_error("gate-address-failed", &ctx.t("cloud.errors.local"));
        return panel(failed.into(), None);
      }
    };
    let host = address.address.web().authority();
    let column = Column::new()
      .id("gate-column")
      .width(Dimension::Px(COLUMN_WIDTH))
      .spacing(GateSpacing::Column.role())
      // *Advanced*'s line ends the column; its ring shows past the column's edge (`Focus::Link`).
      .overflow_visible()
      .with_children(self.parts(ctx, (ended.as_ref(), &address), &host));
    let main = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(GateSpacing::Side.role(), SpacingSize::Section))
      .align_items(Alignment::Center)
      .child(column);
    // As high as the gate when it fits, centered in the panel; scrolled in a window too low for it.
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .justify(Justify::Center)
      .child(
        ScrollVertical::new(main)
          .id("gate-scroll")
          .width(Dimension::Pct(100.0))
          .max_height(Dimension::Pct(100.0)),
      )
      .into();
    let person_only = ended.is_some_and(|ended| ended.reason != CloudSignInEnd::TwoFactor);
    panel(content, Some(footer(ctx, person_only, &address)))
  }
}

impl SignInGate {
  /// The column's parts: the head, the notice, and the card with *Advanced* under it, or the code step's card.
  fn parts(
    &self,
    ctx: &mut Ctx,
    (ended, address): (Option<&EndedCloudSignIn>, &CloudInUse),
    host: &str,
  ) -> Vec<Element> {
    let step = self.step.get();
    if let Step::Code(entry) = &step {
      let head = ended::head(
        ctx,
        Head::Code {
          recovery: entry.recovery(),
        },
      );
      return vec![head, code::card(ctx, &self.code, entry, host)];
    }
    self.code.rest();
    let head = ended::head(ctx, if ended.is_some() { Head::Again } else { Head::First });
    let notice = if step == (Step::Password { timed_out: true }) {
      Some(ended::timed_out(ctx))
    } else {
      ended.map(|ended| ended::notice(ctx, ended))
    };
    let advanced = advanced_section(ctx, address, &self.advanced, Place::Gate);
    let advanced = if self.form.pending() {
      Row::new()
        .width(Dimension::Pct(100.0))
        .opacity(DISABLED_OPACITY)
        .overflow_visible()
        .child(advanced)
        .into()
    } else {
      advanced
    };
    let mut parts = vec![head];
    parts.extend(notice);
    parts.push(form::card(ctx, &self.form, host));
    parts.push(advanced);
    parts
  }
}

/// The content panel (`Content · Sign-in gate` `cvnjK`): `content` over `footer`.
fn panel(content: Element, footer: Option<Element>) -> Element {
  Column::new()
    .id("screen-sign-in-gate")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, ExtraColor::PanelBorder.role())
    .clip()
    .child(content)
    .with_children(footer)
    .into()
}

/// *Signing in needs the internet once…* on a first run, *Only you can sign in…* once the gate came back
/// (`person_only`); the cloud's host at the end, in `warning` with *test cloud* while another cloud than the build's is
/// set.
fn footer(ctx: &mut Ctx, person_only: bool, address: &CloudInUse) -> Element {
  let narrow = ctx.breakpoint() < Some(Breakpoint::Xl);
  let (glyph, key) = match (person_only, narrow) {
    (true, _) => (Icon::UserLock, "gate.footer.personOnly"),
    (false, false) => (Icon::Wifi, "gate.footer.offline"),
    (false, true) => (Icon::Wifi, "gate.footer.offlineShort"),
  };
  let host = address.address.web().authority();
  let (place, color) = if address.default {
    (host, PaletteColor::TextMuted)
  } else {
    (
      ctx
        .t_args("gate.footer.testCloud", [("host", host.as_str())])
        .to_string(),
      PaletteColor::Warning,
    )
  };
  Row::new()
    .id("gate-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Section))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Lg, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t(key))
        .id("gate-footer-note")
        .variant(ExtraTypography::Property.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&place)
        .id("gate-footer-cloud")
        .variant(TypographyStyle::Caption)
        .color(color)
        .nowrap(),
    )
    .into()
}

/// The cloud's address this computer uses, read off the UI thread and again whenever it changes; `None` until the
/// first read finished, an error (logged) when it could not be read.
fn address(ctx: &mut Ctx, cloud: &Cloud) -> Result<Option<CloudInUse>, ()> {
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
  let state = ctx
    .future((revision, cloud.clone()), |(_, cloud)| async move {
      background::run(move || cloud.address())
        .await
        .map_err(|message| tracing::error!(?message, "the gate's read of the cloud's address did not finish"))?
        .map_err(|error| tracing::error!(%error, "the gate cannot read the cloud's address"))
    })
    .state()
    .get();
  match (state.data, state.error) {
    (Some(address), _) => Ok(Some(address)),
    (None, Some(())) => Err(()),
    (None, None) => Ok(None),
  }
}
