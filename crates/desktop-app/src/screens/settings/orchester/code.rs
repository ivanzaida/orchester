//! A code to read out: a team server's check code to compare (`Check code` `ofYYa` in `zxe2P`), and a pairing code
//! shown once (`Pairing code · Add a computer or Sign in to the admin` in `Aowlk`): Orchester does not keep it, and
//! while it shows, MCP clients can neither screenshot the window nor read its text, as with the MCP token.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::parts::{hint, paragraph};
use crate::{
  clipboard::copy_for_a_minute,
  mcp::input_gate::{InputGate, TokenShown},
  services::Services,
  theme::{ExtraRadius, ExtraTypography, OrchesterSpacing, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    icon::Icon,
  },
};

/// A code in its box, `3F7A-92C1-KD8M-QX4T` or `H8RW-2NCE`, centered in Geist Mono 24.
pub(super) fn code_box(id: &str, code: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(OrchesterSpacing::CodeSide.role(), SpacingSize::Md))
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(lurq::app::theme::RadiusSize::Md)
    .child(
      Text::new(code)
        .id(format!("{id}-text"))
        .variant(OrchesterText::Code.role())
        .nowrap(),
    )
}

/// What a pairing code is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
  /// *Add a computer*: the person's next computer joins as them.
  Computer,
  /// *Sign in to the admin*: a sign-in to the server admin in the browser.
  Admin,
}

/// A pairing code made for the server named `server` at `host`, for `username`.
#[derive(Clone, PartialEq, Eq)]
pub struct Shown {
  pub purpose: Purpose,
  pub server: String,
  pub host: String,
  pub username: String,
  /// The code as the server issued it; never logged, kept nowhere else.
  pub code: Secret,
}

/// A code shown once: its memory is wiped when the last copy goes, and `Debug` never shows it. Two are the same only
/// when they are the same issue.
#[derive(Clone)]
pub struct Secret(std::sync::Arc<zeroize::Zeroizing<String>>);

impl Secret {
  pub fn new(code: &orchester_api::Credential) -> Self {
    Self(std::sync::Arc::new(zeroize::Zeroizing::new(code.expose().to_owned())))
  }

  pub fn expose(&self) -> &str {
    self.0.as_str()
  }
}

impl PartialEq for Secret {
  fn eq(&self, other: &Self) -> bool {
    std::sync::Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for Secret {}

impl std::fmt::Debug for Shown {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Shown")
      .field("purpose", &self.purpose)
      .field("server", &self.server)
      .finish_non_exhaustive()
  }
}

impl DevtoolsInspectable for Shown {}

#[derive(Clone)]
pub(super) struct Props {
  pub shown: Shown,
  pub done: std::sync::Arc<dyn Fn() + Send + Sync>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.shown == other.shown
  }
}

impl DevtoolsInspectable for Props {}

/// The pairing code's card, which hides the window from MCP clients while it is mounted.
pub(super) struct ShownCode {
  _shown: TokenShown,
}

impl Component for ShownCode {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    Self {
      _shown: gate.show_token(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { shown, done } = ctx.props::<Props>().clone();
    let runtime = Services::of(ctx).runtime;
    let explanation = match shown.purpose {
      Purpose::Computer => ctx.t_args(
        "cloud.pairing.computer",
        [("host", shown.host.as_str()), ("username", shown.username.as_str())],
      ),
      Purpose::Admin => ctx.t_args(
        "cloud.pairing.admin",
        [("host", shown.host.as_str()), ("username", shown.username.as_str())],
      ),
    };
    let code = shown.code.expose().to_owned();
    let copy = ActionButton::new(
      "orchester-pairing-copy",
      ButtonKind::Secondary,
      ctx.t("cloud.pairing.copy"),
    )
    .icon(Icon::Copy)
    .on_click(move || {
      if !copy_for_a_minute(&runtime, code.clone()) {
        tracing::warn!("the pairing code could not be copied");
      }
    });
    let done = ActionButton::new("orchester-pairing-done", ButtonKind::Ghost, ctx.t("cloud.pairing.done"))
      .on_click(move || done());
    card("orchester-pairing", CardSurface::Outline)
      .rounded(ExtraRadius::Tile.role())
      .padding(SpacingSize::Lg)
      .spacing(SpacingSize::Md)
      .child(
        Text::new(&ctx.t_args("cloud.pairing.title", [("server", shown.server.as_str())]))
          .variant(ExtraTypography::Strong.role()),
      )
      .child(code_box("orchester-pairing-code", shown.code.expose()))
      .child(paragraph("orchester-pairing-explanation", &explanation))
      .child(Row::new().spacing(SpacingSize::Sm).child(copy).child(done))
      .child(hint("orchester-pairing-once", &ctx.t("cloud.pairing.once")))
  }
}

/// [`ShownCode`] mounted for `shown`.
pub(super) fn shown_code(ctx: &mut Ctx, shown: Shown, done: impl Fn() + Send + Sync + 'static) -> Element {
  ctx.mount_keyed::<ShownCode>(
    "orchester-pairing-card",
    Props {
      shown,
      done: std::sync::Arc::new(done),
    },
  )
}

/// A code's column: the box, then what to do with it.
pub(super) fn compared(id: &str, code: &str, parts: Vec<Element>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(code_box(&format!("{id}-code"), code))
    .with_children(parts)
}
