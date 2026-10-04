//! What Add server found at an address (10.02 `kQ6BD`, the states of 10.03 `BWyKt`): the check
//! result card naming the server, its API and transport; the plain HTTP warning with its opt-in; the
//! notice that the server has no admin yet with the host command to copy; and the errors.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension},
};
use orchester_api::client::ServerAddress;

use crate::{
  messages::Message,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    command_row::{CommandPlace, Copied, command_row},
    icon::{Icon, IconSize, icon},
    notice::Notice,
    pressable::{Ring, keyboard_stop, surface},
  },
};

/// The first admin is created on the server's host (`docs/architecture/server.md`, "Bootstrap").
const BOOTSTRAP_COMMAND: &str = "docker compose exec server orchester-server user create --admin <username>";
/// The icon slot keeps the icon on the heading's first line (`T0w3JZ`).
const ICON_SLOT: f32 = 18.0;

/// What answered at an address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Answer {
  Server {
    name: String,
    api_version: u8,
    needs_bootstrap: bool,
  },
  Failed(Message),
}

/// The check of one address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Checked {
  pub address: ServerAddress,
  pub answer: Answer,
}

impl lurq::app::component::DevtoolsInspectable for Checked {}

/// *Orchester server · Kontur team · API 1* with, under it, what the transport means.
pub(super) fn result_card(ctx: &Ctx, name: &str, api_version: u8, detail: Option<Arc<str>>) -> Element {
  let api = api_version.to_string();
  let heading = ctx.t_args("servers.add.found", [("name", name), ("api", api.as_str())]);
  let copy = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(&heading)
        .id("add-server-found")
        .variant(ExtraTypography::ResultHeading.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(detail.map(|detail| {
      Text::new(&detail)
        .variant(ExtraTypography::Detail.role())
        .width(Dimension::Pct(100.0))
    }));
  Row::new()
    .id("add-server-result")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Stack.role())
    .padding_horizontal(ExtraSpacing::Group.role())
    .spacing(ExtraSpacing::Inline.role())
    .border_inside(BorderSize::Sm, ExtraColor::Hairline.role())
    .rounded(ExtraRadius::Notice.role())
    .child(
      Row::new()
        .height(Dimension::Px(ICON_SLOT))
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleCheck, IconSize::Dismiss, PaletteColor::TextPrimary)),
    )
    .child(copy)
    .into()
}

/// What the transport means for an answer: HTTPS trusted by the system, or plain HTTP that never
/// leaves this computer. Plain HTTP to another computer is said by [`plain_http_warning`].
pub(super) fn transport_detail(ctx: &Ctx, address: &ServerAddress, name: &str) -> Option<Arc<str>> {
  if address.is_https() {
    return Some(ctx.t_args(trusted_key(), [("name", name)]));
  }
  address
    .is_loopback()
    .then(|| ctx.t_args("servers.add.loopback", [("host", address.host())]))
}

#[cfg(windows)]
const fn trusted_key() -> &'static str {
  "servers.add.trusted.windows"
}

#[cfg(not(windows))]
const fn trusted_key() -> &'static str {
  "servers.add.trusted.macos"
}

/// 10.03 B: the password and token would cross the network unencrypted; the opt-in for this server.
pub(super) fn plain_http_warning(ctx: &Ctx, address: &ServerAddress, opt_in: &Signal<bool>) -> Element {
  let ticked = opt_in.get();
  let authority = address.authority();
  let label = ctx.t_args("servers.add.allowPlainHttp", [("host", authority.as_str())]);
  let flip = opt_in.clone();
  let check = keyboard_stop(surface(), Ring::Outer)
    .id("add-server-allow-http")
    .describe("checked", if ticked { "true" } else { "false" })
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_click(move |_| flip.update(|ticked| *ticked = !*ticked))
    .child(icon(
      if ticked { Icon::SquareCheck } else { Icon::Square },
      IconSize::Option,
      if ticked {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      },
    ))
    .child(
      Text::new(&label)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary),
    );
  Notice::new(
    Icon::TriangleAlert,
    ctx.t("servers.add.plainHttpHeading"),
    ctx.t_args("servers.add.plainHttpDetail", [("host", address.host())]),
  )
  .emphasized()
  .id("add-server-plain-http")
  .below(
    Row::new()
      .padding_top(ExtraSpacing::Tight.role())
      .overflow_visible()
      .child(check),
  )
  .into()
}

/// 10.03 F: nobody can sign in before the first admin is created on the server's host.
pub(super) fn bootstrap_notice(ctx: &Ctx, name: &str, copied: &Signal<Copied>) -> Element {
  Notice::new(
    Icon::UserCog,
    ctx.t_args("servers.add.noAdminHeading", [("name", name)]),
    ctx.t("servers.add.noAdminDetail"),
  )
  .emphasized()
  .id("add-server-no-admin")
  .below(command_row(
    ctx,
    "add-server-command",
    "add-server-copy-command",
    BOOTSTRAP_COMMAND,
    CommandPlace::Dialog,
    copied,
  ))
  .into()
}
