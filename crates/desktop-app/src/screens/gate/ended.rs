//! The gate's head, and why it came back (`Sign-in gate · why the gate came back` `obc19`): above the form, a notice
//! names the reason the cloud gave, or that the person signed out here, and always says what happens to the work on
//! this computer meanwhile (`While signed out` `Qkpc2`). A reason with no frame (the account was deleted) is worded
//! like the others; any other refusal shows the fallback.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_store::{CloudSignInEnd, EndedCloudSignIn};
use orchester_workspace::display;

use crate::{
  brand::mark,
  theme::{ExtraSpacing, ExtraTypography, GateText, IconSize},
  ui::{
    callout::{Callout, CalloutTone},
    icon::{Icon, icon},
  },
};

/// The mark's tile on a first run (`Mark` `v3ywiH`).
const MARK_TILE: f32 = 40.0;
/// The extra room under the tile, beside the head's 10 (`Gap` `jqwWb`).
const UNDER_MARK: f32 = 2.0;
/// The slot of a *While signed out* row's icon, as high as its first line (`Icon slot` `x7C91I`).
const ICON_SLOT: f32 = 19.0;

/// *Sign in to start* with the mark and what the gate is for on a first run; *Sign in again* once it came back.
pub(super) fn head(ctx: &Ctx, came_back: bool) -> Element {
  let head = Column::new()
    .id("gate-head")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role());
  if came_back {
    return head
      .child(eyebrow(&ctx.t("gate.again.eyebrow")))
      .child(title(&ctx.t("gate.again.title")))
      .into();
  }
  let tile = Row::new()
    .width(Dimension::Px(MARK_TILE))
    .height(Dimension::Px(MARK_TILE))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(mark());
  head
    .child(tile)
    .child(Row::new().height(Dimension::Px(UNDER_MARK)))
    .child(eyebrow(&ctx.t("gate.first.eyebrow")))
    .child(title(&ctx.t("gate.first.title")))
    .child(
      Text::new(&ctx.t("gate.first.lead"))
        .id("gate-lead")
        .variant(GateText::Lead.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

fn eyebrow(text: &str) -> Text {
  Text::new(text).id("gate-eyebrow").variant(GateText::Eyebrow.role())
}

fn title(text: &str) -> Text {
  Text::new(text).id("gate-title").variant(GateText::Title.role())
}

/// Why the last sign-in from here ended, and what that means while signed out.
pub(super) fn notice(ctx: &Ctx, ended: &EndedCloudSignIn) -> Element {
  let (tone, glyph, name, (title, body)) = match ended.reason {
    CloudSignInEnd::Revoked => (
      CalloutTone::Warning,
      Icon::LogOut,
      "revoked",
      ("gate.ended.revoked.title", "gate.ended.revoked.body"),
    ),
    CloudSignInEnd::PasswordReset => (
      CalloutTone::Warning,
      Icon::KeyRound,
      "password-reset",
      ("gate.ended.passwordReset.title", "gate.ended.passwordReset.body"),
    ),
    CloudSignInEnd::AccountDeleted => (
      CalloutTone::Warning,
      Icon::UserX,
      "account-deleted",
      ("gate.ended.accountDeleted.title", "gate.ended.accountDeleted.body"),
    ),
    CloudSignInEnd::SignedOut => (
      CalloutTone::Neutral,
      Icon::LogOut,
      "signed-out",
      ("gate.ended.signedOut.title", "gate.ended.signedOut.body"),
    ),
    CloudSignInEnd::Other => (
      CalloutTone::Warning,
      Icon::CircleAlert,
      "other",
      ("gate.ended.other.title", "gate.ended.other.body"),
    ),
  };
  let time = display::clock(ended.ended_at);
  let body = ctx.t_args(body, [("time", time.as_str()), ("email", ended.email.as_str())]);
  let notice: Element = Callout::new("gate-notice", tone, ctx.t(title))
    .icon(glyph)
    .body(body)
    .detail(while_signed_out(ctx))
    .into();
  Column::new()
    .id("gate-ended")
    .width(Dimension::Pct(100.0))
    .describe("reason", name)
    .child(notice)
    .into()
}

/// What happens to the work on this computer while it is signed out.
fn while_signed_out(ctx: &Ctx) -> Element {
  [
    (Icon::FolderCheck, "gate.whileSignedOut.projects"),
    (Icon::Play, "gate.whileSignedOut.runs"),
    (Icon::Lock, "gate.whileSignedOut.mcp"),
  ]
  .into_iter()
  .fold(
    Column::new()
      .id("gate-while-signed-out")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(ExtraSpacing::Snug.role()))
      .spacing(ExtraSpacing::Snug.role()),
    |rows, (glyph, key)| {
      rows.child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Start)
          .child(
            Row::new()
              .height(Dimension::Px(ICON_SLOT))
              .align_items(Alignment::Center)
              .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted)),
          )
          .child(
            Text::new(&ctx.t(key))
              .variant(ExtraTypography::Lead.role())
              .flex(1.0)
              .min_width(Dimension::Px(0.0)),
          ),
      )
    },
  )
  .into()
}
