//! *Your sessions* (`B8KV9t` in 10.01): every computer signed in as the person, with when it signed
//! in and was last used. This computer's row says so; every other row has *Revoke*, which signs that
//! computer out at once (design decision 3: no confirmation).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::{
  account::{RevokeAction, SessionRow},
  labels::day_and_time,
  table::{Cell, Width, heading, row, table},
};
use crate::{
  sharing::{ServerEntry, device::Device},
  theme::ExtraTypography,
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
  },
};

/// Row and Revoke heights, and the column widths (`mpXZq`, `I2bcyn`, `pnte2`, `PUJBV`).
const ROW_HEIGHT: f32 = 52.0;
const REVOKE_HEIGHT: f32 = 28.0;
const TIME_WIDTH: f32 = 150.0;
const ACTION_WIDTH: f32 = 96.0;

/// Revoking a session of `entry`.
pub(super) struct Revoke {
  pub action: RevokeAction,
  pub entry: ServerEntry,
}

pub(super) fn sessions_table(ctx: &Ctx, rows: &[SessionRow], revoke: &Revoke) -> Element {
  let heading = heading([
    (Width::Fill, Some(ctx.t("servers.sessions.device"))),
    (Width::Px(TIME_WIDTH), Some(ctx.t("servers.sessions.signedIn"))),
    (Width::Px(TIME_WIDTH), Some(ctx.t("servers.sessions.lastUsed"))),
    (Width::Px(ACTION_WIDTH), None),
  ]);
  let pending = revoke.action.state().get().is_pending();
  table(
    "server-sessions",
    heading,
    rows.iter().map(|session| session_row(ctx, session, revoke, pending)),
  )
}

fn session_row(ctx: &Ctx, session: &SessionRow, revoke: &Revoke, pending: bool) -> Element {
  let last_used = if session.current {
    Text::new(&ctx.t("servers.sessions.now"))
      .variant(ExtraTypography::MonoValue.role())
      .nowrap()
  } else {
    time(ctx, session.last_used_at)
  };
  let action: Element = if session.current {
    Text::new(&ctx.t("servers.sessions.thisComputer"))
      .variant(TypographyStyle::Caption)
      .nowrap()
      .into()
  } else {
    let (action, entry, id) = (revoke.action.clone(), revoke.entry.clone(), session.id);
    let button = ActionButton::new(ctx.t("servers.sessions.revoke"))
      .icon(Icon::X)
      .id(Arc::<str>::from(format!("server-revoke-{id}")))
      .height(Dimension::Px(REVOKE_HEIGHT));
    let button = if pending {
      button.on_click(|| {}).enabled(false)
    } else {
      button.on_click(move || action.run((entry.clone(), id)))
    };
    button.render(ctx)
  };
  row(
    Dimension::Px(ROW_HEIGHT),
    [
      Cell::new(Width::Fill, device(ctx, session)),
      Cell::new(Width::Px(TIME_WIDTH), time(ctx, session.created_at)),
      Cell::new(Width::Px(TIME_WIDTH), last_used),
      Cell::new(Width::Px(ACTION_WIDTH), action).at_end(),
    ],
  )
}

/// The device's icon, *Orchester on* its computer, and its platform (`nVgUV`).
fn device(ctx: &Ctx, session: &SessionRow) -> Element {
  let (title, platform) = match &session.device {
    Device::Orchester { computer, platform } => (
      ctx.t_args("servers.sessions.orchesterOn", [("computer", computer.as_str())]),
      Some(platform.as_str()),
    ),
    Device::Other(text) => (Arc::from(text.as_str()), None),
  };
  let platform = match (platform, session.current) {
    (Some(platform), true) => Some(ctx.t_args("servers.sessions.platformHere", [("platform", platform)])),
    (Some(platform), false) => Some(Arc::from(platform)),
    (None, true) => Some(ctx.t("servers.sessions.thisComputerShort")),
    (None, false) => None,
  };
  let (glyph, color) = if session.current {
    (Icon::MonitorCheck, PaletteColor::TextPrimary)
  } else {
    (Icon::Monitor, PaletteColor::TextSecondary)
  };
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Dismiss, color))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(&title)
            .variant(ExtraTypography::FileName.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .with_children(
          platform.map(|platform| Text::new(&platform).variant(ExtraTypography::Footnote.role()).nowrap()),
        ),
    )
    .into()
}

fn time(ctx: &Ctx, millis: i64) -> Text {
  Text::new(&day_and_time(ctx, Timestamp::from_unix_millis(millis)))
    .variant(ExtraTypography::PullRequestContext.role())
    .nowrap()
}
