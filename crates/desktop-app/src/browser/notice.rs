//! What the page area shows instead of a page, as a card in its middle (`State` in `05` `GjcAk`, `06` `eCCuU`, `07`
//! `YEv2c`): no tab yet (*No pages yet*), no browser found (the reason, *Open Settings › Browser*, *Look again*), and
//! the browser restarted in a window of its own (*Back to Orchester*).

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_ipc::browser::BrowserCommand;

use super::{link::BrowserLink, state::Showing};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  state::EmptyState,
};

/// The card's width (`State` `vBDaj`).
const CARD_WIDTH: f32 = 480.0;

/// *No pages yet* (`06 · Empty` `eCCuU`).
pub fn empty(ctx: &mut Ctx) -> Element {
  centered(EmptyState::new(
    "browser-empty",
    Icon::Globe,
    ctx.t("browser.empty.title"),
    ctx.t("browser.empty.body"),
  ))
}

/// *No Chromium-based browser found* (`07 · Browser not found` `YEv2c`), in this platform's words.
pub fn not_found(ctx: &mut Ctx, showing: &Showing) -> Element {
  let body = if cfg!(target_os = "windows") {
    "browser.notFound.bodyWindows"
  } else if cfg!(target_os = "macos") {
    "browser.notFound.bodyMacos"
  } else {
    "browser.notFound.bodyUnsupported"
  };
  let link = BrowserLink::of(ctx);
  let settings = ActionButton::new(
    "browser-open-settings",
    ButtonKind::Primary,
    ctx.t("browser.notFound.openSettings"),
  )
  .icon(Icon::Settings)
  .on_click(move || {
    if let Some(link) = &link {
      link.open_settings();
    }
  });
  let hooks = showing.person.clone();
  let again = ActionButton::new(
    "browser-look-again",
    ButtonKind::Secondary,
    ctx.t("browser.notFound.lookAgain"),
  )
  .icon(Icon::RotateCw)
  .on_click(move || hooks.look_again());
  centered(
    EmptyState::new(
      "browser-not-found",
      Icon::TriangleAlert,
      ctx.t("browser.notFound.title"),
      ctx.t(body),
    )
    .warning()
    .action(settings)
    .action(again),
  )
}

/// *Kontur's browser is open in a window of its own* (`05 · Opened in a browser window` `GjcAk`); *Back to Orchester*
/// turns its loader and cannot be pressed again until the browser restarted, which takes seconds.
pub fn headed(ctx: &mut Ctx, showing: &Showing) -> Element {
  let commands = showing.commands.clone();
  let back = ActionButton::new(
    "browser-back-to-orchester",
    ButtonKind::Primary,
    ctx.t("browser.headed.back"),
  )
  .icon(Icon::Undo2)
  .on_click(move || commands.send(BrowserCommand::BackToOrchester));
  let back = if showing.restarting { back.loading() } else { back };
  centered(
    EmptyState::new(
      "browser-headed",
      Icon::AppWindow,
      ctx.t_args("browser.headed.title", [("project", showing.name.as_ref())]),
      ctx.t("browser.headed.body"),
    )
    .action(back),
  )
}

/// `state` as a card in the middle of the page area.
fn centered(state: EmptyState) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(Row::new().width(Dimension::Px(CARD_WIDTH)).child(state))
    .into()
}
