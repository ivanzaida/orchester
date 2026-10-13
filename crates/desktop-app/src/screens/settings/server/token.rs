//! *Access token* (`Form row/stacked` `WXgjE` with a `Masked value` `DShSq`, `lye6f`; its states `eM8tB`): where it
//! is kept, the token masked, and *Show*, *Copy* and *Rotate token*, each on a person's click only. Shown, the token
//! is sensitive text (lurq keeps it out of the element tree and greys it in MCP screenshots), the server refuses
//! screenshots and the window's text meanwhile, and it hides again after 30 seconds. *Rotate token* asks in place
//! under the row (`In-place confirmation` `RWODj`), then says the new token is in use, or that the secret store
//! refused it and the old one stays.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{background, messages::Message};
use parking_lot::Mutex;

use super::{Change, change, copy::copy_for_a_minute, enabled::refused, outcome};
use crate::{
  mcp::{
    AppMcp, BearerToken, ServerState,
    input_gate::{InputGate, TokenShown},
  },
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::field_error,
    form_section::{Label, Place, stacked_row},
    icon::{Icon, icon},
    inspector::note,
    message::message_text,
  },
};

/// How long the token shows before it hides again.
const SHOWN_FOR: u32 = 30;
/// The masked value's height (`DShSq`).
const MASK_HEIGHT: f32 = 32.0;

/// Where this platform keeps the token, as Settings names it.
#[cfg(target_os = "macos")]
pub const STORE_KEY: &str = "settings.automation.server.store.macos";
#[cfg(not(target_os = "macos"))]
pub const STORE_KEY: &str = "settings.automation.server.store.windows";
#[cfg(target_os = "macos")]
const WHERE_KEY: &str = "settings.automation.server.token.whereMacos";
#[cfg(not(target_os = "macos"))]
const WHERE_KEY: &str = "settings.automation.server.token.whereWindows";

#[derive(Clone)]
pub struct Props {
  pub server: Option<AppMcp>,
  pub state: ServerState,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.state == other.state && self.server.is_some() == other.server.is_some()
  }
}

impl DevtoolsInspectable for Props {}

/// What came of the last *Copy*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Copied {
  Not,
  Done,
  Failed,
}

impl DevtoolsInspectable for Copied {}

pub struct TokenRow {
  /// The token while it shows, and the mark that keeps clients from reading the window meanwhile.
  reveal: Arc<Mutex<Option<(BearerToken, TokenShown)>>>,
  /// Seconds until it hides again; 0 while it is hidden.
  left: Signal<u32>,
  _tick: Interval,
  confirming: Signal<bool>,
  /// Whether this row rotated the token since it showed.
  rotated: Signal<bool>,
  copied: Signal<Copied>,
  blocked: Signal<bool>,
}

impl Component for TokenRow {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let reveal: Arc<Mutex<Option<(BearerToken, TokenShown)>>> = Arc::default();
    let left = ctx.signal(0);
    let (hide, count) = (reveal.clone(), left.clone());
    // The signal changes only while the token shows, so a hidden token renders nothing each second.
    let tick = ctx.create_interval(Duration::from_secs(1), move || {
      let seconds = count.get();
      if seconds == 0 {
        return;
      }
      if seconds == 1 {
        hide.lock().take();
      }
      count.set(seconds - 1);
    });
    tick.start();
    Self {
      reveal,
      left,
      _tick: tick,
      confirming: ctx.signal(false),
      rotated: ctx.signal(false),
      copied: ctx.signal(Copied::Not),
      blocked: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let rotate: Change<()> = change(ctx, props.server.clone(), |server, ()| server.rotate_token());
    let rotating = rotate.state().get();
    let readable = ready(ctx, &props);
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    let runtime = Services::of(ctx).runtime;
    let shown = self.reveal.lock().as_ref().map(|(token, _)| token.clone());
    let value = value(ctx, shown.as_ref());
    let buttons = self.buttons(ctx, &props, (&gate, &runtime), (shown.is_some(), readable, &rotate));
    let mut lines: Vec<Element> = Vec::new();
    if shown.is_some() {
      lines.push(
        note(
          "settings-mcp-token-shown-for",
          Icon::Timer,
          &ctx.t_args(
            "settings.automation.server.token.shownFor",
            [("seconds", self.left.get().to_string())],
          ),
        )
        .into(),
      );
    }
    lines.extend(self.copy_line(ctx));
    if self.confirming.get() {
      lines.push(self.confirmation(ctx, rotate.clone(), rotating.is_pending()));
    }
    lines.extend(self.rotation_line(ctx, &rotate, rotating.error.as_ref(), rotating.is_fulfilled()));
    if self.blocked.get() {
      lines.push(
        field_error(
          "settings-mcp-token-blocked",
          &ctx.t("settings.automation.server.blocked"),
        )
        .into(),
      );
    }
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(value)
          .with_children(buttons),
      )
      .with_children(lines);
    stacked_row(
      Label::new("settings-mcp-token", ctx.t("settings.automation.server.token.title")).description(ctx.t(WHERE_KEY)),
      control,
      Place::Within,
    )
  }
}

/// The token masked, or shown as sensitive text while a person reads it.
fn value(ctx: &Ctx, shown: Option<&BearerToken>) -> Element {
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
  Row::new()
    .id("settings-mcp-token-value")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Px(MASK_HEIGHT))
    .align_items(Alignment::Center)
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(text.id("settings-mcp-token-text").flex(1.0).nowrap())
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .into()
}

impl TokenRow {
  /// *Show* or *Hide*, *Copy* and *Rotate token*.
  fn buttons(
    &self,
    ctx: &Ctx,
    props: &Props,
    (gate, runtime): (&InputGate, &tokio::runtime::Handle),
    (shown, readable, rotate): (bool, bool, &Change<()>),
  ) -> Vec<Element> {
    let server = props.server.clone();
    let show = if shown {
      let (reveal, left) = (self.reveal.clone(), self.left.clone());
      ActionButton::new(
        "settings-mcp-token-show",
        ButtonKind::Ghost,
        ctx.t("settings.automation.server.token.hide"),
      )
      .icon(Icon::EyeOff)
      .on_click(move || {
        reveal.lock().take();
        left.set(0);
      })
    } else {
      let button = ActionButton::new(
        "settings-mcp-token-show",
        ButtonKind::Ghost,
        ctx.t("settings.automation.server.token.show"),
      )
      .icon(Icon::Eye);
      match server.clone().filter(|_| readable) {
        Some(server) => {
          let (reveal, left, blocked, gate) = (
            self.reveal.clone(),
            self.left.clone(),
            self.blocked.clone(),
            gate.clone(),
          );
          button.on_click(move || {
            if refused(&blocked) {
              return;
            }
            let Some(token) = token_of(&server) else { return };
            // Clients stop reading the window before the token is drawn.
            *reveal.lock() = Some((token, gate.show_token()));
            left.set(SHOWN_FOR);
          })
        }
        None => button.disabled(),
      }
    };
    let copy = ActionButton::new(
      "settings-mcp-token-copy",
      ButtonKind::Ghost,
      ctx.t("settings.automation.server.token.copy"),
    )
    .icon(Icon::Copy);
    let copy = match server.filter(|_| readable) {
      Some(server) => {
        let (copied, blocked, runtime) = (self.copied.clone(), self.blocked.clone(), runtime.clone());
        copy.on_click(move || {
          if refused(&blocked) {
            return;
          }
          let done = token_of(&server).is_some_and(|token| copy_for_a_minute(&runtime, token.expose().to_owned()));
          copied.set(if done { Copied::Done } else { Copied::Failed });
        })
      }
      None => copy.disabled(),
    };
    let rotate_button = ActionButton::new(
      "settings-mcp-token-rotate",
      ButtonKind::Secondary,
      ctx.t("settings.automation.server.token.rotate"),
    )
    .icon(Icon::RefreshCw);
    let rotate_button = if props.server.is_none() || self.confirming.get() || rotate.state().get().is_pending() {
      rotate_button.disabled()
    } else {
      let (confirming, blocked, rotated) = (self.confirming.clone(), self.blocked.clone(), self.rotated.clone());
      rotate_button.on_click(move || {
        if !refused(&blocked) {
          rotated.set(false);
          confirming.set(true);
        }
      })
    };
    vec![show.into(), copy.into(), rotate_button.into()]
  }

  /// What came of the last *Copy*.
  fn copy_line(&self, ctx: &Ctx) -> Option<Element> {
    match self.copied.get() {
      Copied::Not => None,
      Copied::Done => Some(
        outcome(
          "settings-mcp-token-copied",
          Icon::ClipboardCheck,
          PaletteColor::Success,
          &ctx.t("settings.automation.server.token.copied"),
        )
        .into(),
      ),
      Copied::Failed => Some(
        field_error(
          "settings-mcp-token-copied",
          &ctx.t("settings.automation.server.token.copyFailed"),
        )
        .into(),
      ),
    }
  }

  /// *Rotate the token?* in place under the row.
  fn confirmation(&self, ctx: &Ctx, rotate: Change<()>, pending: bool) -> Element {
    let store = ctx.t(STORE_KEY);
    let (confirming, blocked, reveal, left, rotated) = (
      self.confirming.clone(),
      self.blocked.clone(),
      self.reveal.clone(),
      self.left.clone(),
      self.rotated.clone(),
    );
    let mut confirm = ActionButton::new(
      "settings-mcp-token-rotate-confirm",
      ButtonKind::Danger,
      ctx.t("settings.automation.server.token.rotateConfirm.confirm"),
    )
    .icon(Icon::RefreshCw);
    confirm = if pending {
      confirm.loading()
    } else {
      confirm.on_click(move || {
        if refused(&blocked) {
          return;
        }
        reveal.lock().take();
        left.set(0);
        confirming.set(false);
        rotated.set(true);
        rotate.run(());
      })
    };
    let keep = self.confirming.clone();
    card("settings-mcp-token-rotate-question", CardSurface::Decision)
      .background(PaletteColor::SurfaceRaised)
      .rounded(ExtraRadius::Tile.role())
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Loose.role())
      .child(
        Text::new(&ctx.t("settings.automation.server.token.rotateConfirm.title"))
          .variant(ExtraTypography::Strong.role()),
      )
      .child(note(
        "settings-mcp-token-rotate-agents",
        Icon::Unplug,
        &ctx.t("settings.automation.server.token.rotateConfirm.agents"),
      ))
      .child(note(
        "settings-mcp-token-rotate-sessions",
        Icon::Unlink,
        &ctx.t_args(
          "settings.automation.server.token.rotateConfirm.sessions",
          [("store", &*store)],
        ),
      ))
      .child(
        Row::new().spacing(SpacingSize::Sm).child(confirm).child(
          ActionButton::new(
            "settings-mcp-token-rotate-keep",
            ButtonKind::Secondary,
            ctx.t("settings.automation.server.token.rotateConfirm.cancel"),
          )
          .on_click(move || keep.set(false)),
        ),
      )
      .into()
  }

  /// That the new token is in use, or that the secret store refused it, with *Try again*.
  fn rotation_line(&self, ctx: &Ctx, rotate: &Change<()>, failed: Option<&Message>, done: bool) -> Option<Element> {
    if !self.rotated.get() {
      return None;
    }
    if let Some(reason) = failed {
      let store = ctx.t(STORE_KEY);
      let reason = message_text(ctx, reason);
      let again = rotate.clone();
      let blocked = self.blocked.clone();
      return Some(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(
            field_error(
              "settings-mcp-token-not-rotated",
              &ctx.t_args(
                "settings.automation.server.token.notRotated",
                [("store", &*store), ("reason", &*reason)],
              ),
            )
            .flex(1.0),
          )
          .child(
            ActionButton::new(
              "settings-mcp-token-rotate-again",
              ButtonKind::Ghost,
              ctx.t("settings.automation.server.token.tryAgain"),
            )
            .icon(Icon::RotateCcw)
            .on_click(move || {
              if !refused(&blocked) {
                again.run(());
              }
            }),
          )
          .into(),
      );
    }
    done.then(|| {
      outcome(
        "settings-mcp-token-rotated",
        Icon::CircleCheck,
        PaletteColor::Success,
        &ctx.t("settings.automation.server.token.rotated"),
      )
      .into()
    })
  }
}

/// The token in use, which [`ready`] read; `None` until it did.
fn token_of(server: &AppMcp) -> Option<BearerToken> {
  server.token()
}

/// Reads the token off the UI thread when the server has not read it yet (it is off, or has not started), so
/// *Show* and *Copy* never wait on the secret store. A token that cannot be read is said by the server's state.
fn ready(ctx: &mut Ctx, props: &Props) -> bool {
  let server = props.server.clone();
  let read = ctx.future(props.state.clone(), move |state| {
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
  // Reading the state subscribes the row: it renders again once the token is read.
  read.state().get();
  props.server.as_ref().is_some_and(|server| server.token().is_some())
}
