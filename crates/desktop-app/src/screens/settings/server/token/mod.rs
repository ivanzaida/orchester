//! *Access token* (`Form row/stacked` `WXgjE` with a `Masked value` `DShSq`, `lye6f`; its states `eM8tB`): where it
//! is kept, the token masked, and *Show*, *Copy* and *Rotate token*, each on a person's click only. Shown, the token
//! is sensitive text (lurq keeps it out of the element tree and greys it in MCP screenshots), the server refuses
//! screenshots and the window's text meanwhile, and it hides again after 30 seconds (`reveal`). *Rotate token* asks
//! in place under the row (`In-place confirmation` `RWODj`), then says the new token is in use, or that the secret
//! store refused it and the old one stays (`rotation`). While the server is off the row reads the token off the UI
//! thread first, and says so with *Try again* if it cannot.

mod reveal;
mod rotation;

use std::time::Duration;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use self::reveal::{Read, Reveal, ready, value};
use super::{Change, change, enabled::refused, outcome};
use crate::{
  clipboard::copy_for_a_minute,
  mcp::{AppMcp, ServerState, input_gate::InputGate},
  screens::settings::save_state::{Outcome, SaveState},
  services::Services,
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    form_section::{Label, Place, stacked_row},
    icon::Icon,
    inspector::note,
  },
};

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
  reveal: Reveal,
  /// Seconds until it hides again; 0 while it is hidden.
  left: Signal<u32>,
  _tick: Interval,
  /// Raised by *Try again* when the token could not be read.
  read_again: Signal<u32>,
  confirming: Signal<bool>,
  /// Whether this row rotated the token since it showed.
  rotated: Signal<bool>,
  copied: Signal<Copied>,
  blocked: Signal<bool>,
}

impl Component for TokenRow {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let reveal = Reveal::default();
    let left = ctx.signal(0);
    let (hide, count) = (reveal.clone(), left.clone());
    // The signal changes only while the token shows, so a hidden token renders nothing each second.
    let tick = ctx.create_interval(Duration::from_secs(1), move || {
      let seconds = count.get();
      if seconds == 0 {
        return;
      }
      if seconds == 1 {
        hide.hide();
      }
      count.set(seconds - 1);
    });
    tick.start();
    Self {
      reveal,
      left,
      _tick: tick,
      read_again: ctx.signal(0),
      confirming: ctx.signal(false),
      rotated: ctx.signal(false),
      copied: ctx.signal(Copied::Not),
      blocked: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let rotate: Change<()> = change(ctx, props.server.clone(), |server, ()| server.rotate_token());
    if let Some(saves) = SaveState::of(ctx) {
      saves.report("settings-mcp-rotate", Outcome::of(&rotate.state().get()));
    }
    let read = ready(ctx, &props, self.read_again.get());
    let shown = self.reveal.token();
    let value = value(ctx, shown.as_ref());
    let buttons = self.buttons(ctx, &props, &read, (shown.is_some(), &rotate));
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
    lines.extend(self.not_read(ctx, &read));
    lines.extend(self.copy_line(ctx));
    lines.extend(self.rotation_lines(ctx, &rotate));
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
    let row = stacked_row(
      Label::new("settings-mcp-token", ctx.t("settings.automation.server.token.title")).description(ctx.t(WHERE_KEY)),
      control,
      Place::Within,
    );
    // This render no longer draws a token that was hidden: clients may read the window again once it is shown.
    if shown.is_none() {
      self.reveal.done_hiding();
    }
    row
  }
}

impl TokenRow {
  /// *Show* or *Hide*, *Copy* and *Rotate token*.
  fn buttons(&self, ctx: &mut Ctx, props: &Props, read: &Read, (shown, rotate): (bool, &Change<()>)) -> Vec<Element> {
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    let runtime = Services::of(ctx).runtime;
    let readable = props.server.clone().filter(|_| read.readable);
    let show = self.show_slot(ctx, readable.clone(), &gate, shown);
    let copy = ActionButton::new(
      "settings-mcp-token-copy",
      ButtonKind::Ghost,
      ctx.t("settings.automation.server.token.copy"),
    )
    .icon(Icon::Copy);
    let copy = match readable {
      Some(server) => {
        let (copied, blocked) = (self.copied.clone(), self.blocked.clone());
        copy.on_click(move || {
          if refused(&blocked) {
            return;
          }
          let done = server
            .token()
            .is_some_and(|token| copy_for_a_minute(&runtime, token.expose().to_owned()));
          copied.set(if done { Copied::Done } else { Copied::Failed });
        })
      }
      None => copy.disabled(),
    };
    vec![show, copy.into(), self.rotate_button(ctx, props, read, rotate)]
  }

  /// *Rotate token*: available once the token was read, or when it cannot be (a new one replaces it), never while a
  /// read or a rotation runs.
  fn rotate_button(&self, ctx: &Ctx, props: &Props, read: &Read, rotate: &Change<()>) -> Element {
    let button = ActionButton::new(
      "settings-mcp-token-rotate",
      ButtonKind::Secondary,
      ctx.t("settings.automation.server.token.rotate"),
    )
    .icon(Icon::RefreshCw);
    let unreadable = read.failure.is_some() || matches!(props.state, ServerState::TokenUnreadable { .. });
    if props.server.is_none()
      || !(read.readable || unreadable)
      || self.confirming.get()
      || rotate.state().get().is_pending()
    {
      return button.disabled().into();
    }
    let (confirming, blocked, rotated) = (self.confirming.clone(), self.blocked.clone(), self.rotated.clone());
    button
      .on_click(move || {
        if !refused(&blocked) {
          rotated.set(false);
          confirming.set(true);
        }
      })
      .into()
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
}
