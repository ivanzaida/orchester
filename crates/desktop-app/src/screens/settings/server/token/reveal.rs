//! The token's value, *Show* and *Hide*, and reading the token while the server has not read it yet.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{background, messages::Message};
use parking_lot::Mutex;

use super::{Props, STORE_KEY, TokenRow, refused};
use crate::{
  mcp::{
    AppMcp, BearerToken, ServerState,
    input_gate::{InputGate, TokenShown},
  },
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// How long the token shows before it hides again.
const SHOWN_FOR: u32 = 30;
/// The masked value's height (`DShSq`).
const MASK_HEIGHT: f32 = 32.0;

/// The token while it shows, with the mark that keeps clients from reading the window; once it is hidden, that mark
/// until a render no longer draws the token, so clients never read a window that still shows it.
#[derive(Clone, Default)]
pub(super) struct Reveal {
  shown: Arc<Mutex<Option<(BearerToken, TokenShown)>>>,
  fading: Arc<Mutex<Option<TokenShown>>>,
}

impl Reveal {
  pub(super) fn token(&self) -> Option<BearerToken> {
    self.shown.lock().as_ref().map(|(token, _)| token.clone())
  }

  /// Hides the token; clients stay kept from the window until [`Reveal::done_hiding`].
  pub(super) fn hide(&self) {
    let shown = self.shown.lock().take();
    if let Some((_, mark)) = shown {
      *self.fading.lock() = Some(mark);
    }
  }

  /// A render that does not draw the token ran: clients may read the window once it is shown.
  pub(super) fn done_hiding(&self) {
    let mark = self.fading.lock().take();
    drop(mark);
  }
}

/// Whether the token can be shown and copied now, and why not.
pub(super) struct Read {
  /// The server holds the token.
  pub readable: bool,
  /// Why the secret store did not give it.
  pub failure: Option<Message>,
  pub pending: bool,
}

/// Reads the token off the UI thread when the server has not read it yet (it is off, or has not started), so *Show*
/// and *Copy* never wait on the secret store; `again` reads it once more. While the server's state says the token
/// cannot be read, that state says so instead.
pub(super) fn ready(ctx: &mut Ctx, props: &Props, again: u32) -> Read {
  let server = props.server.clone();
  let read = ctx.future((props.state.clone(), again), move |(state, _)| {
    let server = server.clone();
    async move {
      let Some(server) = server.filter(|server| server.token().is_none()) else {
        return Ok(());
      };
      if matches!(state, ServerState::TokenUnreadable { .. }) {
        return Ok(());
      }
      background::run(move || server.read_token().map(drop))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "the app's MCP token cannot be read for the window");
          error.message()
        })
    }
  });
  let state = read.state().get();
  Read {
    readable: props.server.as_ref().is_some_and(|server| server.token().is_some()),
    failure: state.error.clone(),
    pending: state.is_pending(),
  }
}

/// The token masked, or shown as sensitive text while a person reads it.
pub(super) fn value(ctx: &Ctx, shown: Option<&BearerToken>) -> Element {
  let (text, glyph) = match shown {
    Some(token) => (
      Text::new(token.expose())
        .sensitive()
        .variant(ExtraTypography::PropertyMono.role()),
      Icon::Eye,
    ),
    None => (
      Text::new(&ctx.t("settings.automation.server.token.mask")).variant(ExtraTypography::MonoKey.role()),
      Icon::LockKeyhole,
    ),
  };
  // Both grow from no width at all, so the field takes the room the buttons leave whatever it shows; a token longer
  // than the field ends in an ellipsis.
  Row::new()
    .id("settings-mcp-token-value")
    .width(Dimension::Px(0.0))
    .flex(1.0)
    .height(Dimension::Px(MASK_HEIGHT))
    .clip()
    .align_items(Alignment::Center)
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      text
        .id("settings-mcp-token-text")
        .width(Dimension::Px(0.0))
        .flex(1.0)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .into()
}

impl TokenRow {
  /// *Show*, or *Hide* while the token shows, in one slot as wide as the wider of the two as lurq lays them out: both
  /// are laid out in it, flat, unseen and out of the pointer's way, and the one that acts fills its width, so the
  /// field and the buttons after it never move. `server` is the one to read the token from, when it can be read.
  pub(super) fn show_slot(&self, ctx: &Ctx, server: Option<AppMcp>, gate: &InputGate, shown: bool) -> Element {
    let button = |id: &str, hide: bool| {
      let (key, glyph) = if hide {
        ("settings.automation.server.token.hide", Icon::EyeOff)
      } else {
        ("settings.automation.server.token.show", Icon::Eye)
      };
      ActionButton::new(id.to_owned(), ButtonKind::Ghost, ctx.t(key)).icon(glyph)
    };
    let active = button("settings-mcp-token-show", shown);
    let active = if shown {
      let (reveal, left) = (self.reveal.clone(), self.left.clone());
      active.on_click(move || {
        reveal.hide();
        left.set(0);
      })
    } else if let Some(server) = server {
      let (reveal, left, blocked, gate) = (
        self.reveal.clone(),
        self.left.clone(),
        self.blocked.clone(),
        gate.clone(),
      );
      active.on_click(move || {
        if refused(&blocked) {
          return;
        }
        let Some(token) = server.token() else { return };
        // Clients stop reading the window before the token is drawn.
        *reveal.shown.lock() = Some((token, gate.show_token()));
        left.set(SHOWN_FOR);
      })
    } else {
      active.disabled()
    };
    let sizer = |id: &str, hide: bool| {
      Row::new()
        .height(Dimension::Px(0.0))
        .clip()
        .opacity(0.0)
        .pointer_events_none()
        .child(button(id, hide))
    };
    Column::new()
      .id("settings-mcp-token-show-slot")
      .align_items(Alignment::Stretch)
      .child(sizer("settings-mcp-token-show-sizer-show", false))
      .child(sizer("settings-mcp-token-show-sizer-hide", true))
      .child(active)
      .into()
  }

  /// That the token could not be read from the secret store, with *Try again*.
  pub(super) fn not_read(&self, ctx: &Ctx, read: &Read) -> Option<Element> {
    let reason = read.failure.as_ref()?;
    let store = ctx.t(STORE_KEY);
    let reason = message_text(ctx, reason);
    let mut again = ActionButton::new(
      "settings-mcp-token-read-again",
      ButtonKind::Ghost,
      ctx.t("settings.automation.server.token.tryAgain"),
    )
    .icon(Icon::RotateCcw);
    again = if read.pending {
      again.disabled()
    } else {
      let (blocked, count) = (self.blocked.clone(), self.read_again.clone());
      again.on_click(move || {
        if !refused(&blocked) {
          count.update(|count| *count += 1);
        }
      })
    };
    Some(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          field_error(
            "settings-mcp-token-not-read",
            &ctx.t_args(
              "settings.automation.server.token.notRead",
              [("store", &*store), ("reason", &*reason)],
            ),
          )
          .flex(1.0),
        )
        .child(again)
        .into(),
    )
  }
}
