//! *Access token* (`Access token` `v8hHM` in `04 · Automation`; its states on `shpsk`): where it is kept, the token
//! masked, and *Show*, Copy and *Rotate*, each on a person's click only. Shown, the token is sensitive text (lurq keeps
//! it out of the element tree and greys it in MCP screenshots), the line under the label counts down until it hides
//! again after 30 seconds, and the server refuses screenshots and the window's text meanwhile (`reveal`). *Rotate*
//! asks in place under the row, then says the new token is in use, or that the secret store refused it and the old
//! one stays (`rotation`). While the server is off the row reads the token off the UI thread first, and says so with
//! *Try again* if it cannot; while the server cannot read it, a notice under the row says why, with *Try again* and
//! *Rotate token*.

mod reveal;
mod rotation;

use std::time::Duration;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::SpacingSize,
  },
  components::Row,
  core::Signal,
  layout::Alignment,
  node::Element,
};

use self::reveal::{Read, Reveal, ready, value};
use super::{Change, change, enabled::refused, outcome};
use crate::{
  clipboard::copy_for_a_minute,
  mcp::{AppMcp, ServerState, input_gate::InputGate},
  screens::settings::save_state::{Outcome, SaveState},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    icon_button::IconButton,
    setting_rows::SettingRow,
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
#[cfg(target_os = "macos")]
const KEPT_KEY: &str = "settings.automation.server.token.keptMacos";
#[cfg(not(target_os = "macos"))]
const KEPT_KEY: &str = "settings.automation.server.token.keptWindows";

#[derive(Clone)]
pub struct Props {
  pub server: Option<AppMcp>,
  pub state: ServerState,
  /// The route asks *Rotate the token?*: the question opens in place once it does.
  pub rotation: bool,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.state == other.state && self.server.is_some() == other.server.is_some() && self.rotation == other.rotation
  }
}

impl DevtoolsInspectable for Props {}

/// What came of the last Copy.
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
  /// Whether the route asked *Rotate the token?* when the row last rendered: it opens the question once each time.
  asked: parking_lot::Mutex<bool>,
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
      asked: parking_lot::Mutex::new(false),
      rotated: ctx.signal(false),
      copied: ctx.signal(Copied::Not),
      blocked: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.follow_route(&props);
    // Hooks in the same order on every render.
    let rotate: Change<()> = change(ctx, props.server.clone(), |server, ()| server.rotate_token());
    let again: Change<()> = change(ctx, props.server.clone(), |server, ()| {
      server.try_again();
      Ok(())
    });
    if let Some(saves) = SaveState::of(ctx) {
      saves.report("settings-mcp-rotate", Outcome::of(&rotate.state().get()));
    }
    let read = ready(ctx, &props, self.read_again.get());
    let shown = self.reveal.token();
    let control = Row::new()
      .id("settings-mcp-token-control")
      .flex_shrink(1.0)
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(value(ctx, shown.as_ref()))
      .with_children(self.buttons(ctx, &props, &read, (shown.is_some(), &rotate)));
    let hint = if shown.is_some() {
      ctx.t_args(
        "settings.automation.server.token.shownFor",
        [("seconds", self.left.get().to_string())],
      )
    } else {
      ctx.t(KEPT_KEY)
    };
    let mut below: Vec<Element> = Vec::new();
    below.extend(self.unreadable(ctx, &props, again, &rotate));
    below.extend(self.not_read(ctx, &read));
    below.extend(self.copy_line(ctx));
    below.extend(self.rotation_lines(ctx, &rotate));
    if self.blocked.get() {
      below.push(
        field_error(
          "settings-mcp-token-blocked",
          &ctx.t("settings.automation.server.blocked"),
        )
        .into(),
      );
    }
    let row = SettingRow::new("settings-mcp-token", ctx.t("settings.automation.server.token.title"))
      .hint(hint)
      .info(ctx.t(WHERE_KEY))
      .control(control)
      .below_all(below)
      .render(ctx);
    // This render no longer draws a token that was hidden: clients may read the window again once it is shown.
    if shown.is_none() {
      self.reveal.done_hiding();
    }
    row
  }
}

impl TokenRow {
  /// *Show* or *Hide*, Copy and *Rotate*.
  fn buttons(&self, ctx: &mut Ctx, props: &Props, read: &Read, (shown, rotate): (bool, &Change<()>)) -> Vec<Element> {
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    let runtime = Services::of(ctx).runtime;
    let readable = props.server.clone().filter(|_| read.readable);
    let show = self.show_slot(ctx, readable.clone(), &gate, shown);
    let copy = IconButton::new(
      "settings-mcp-token-copy",
      Icon::Copy,
      ctx.t("settings.automation.server.token.copy"),
    )
    .small();
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
      None => copy,
    };
    vec![show, copy.render(ctx), self.rotate_button(ctx, props, read, rotate)]
  }

  /// *Rotate*: available once the token was read, or when it cannot be (a new one replaces it), never while a read or
  /// a rotation runs.
  fn rotate_button(&self, ctx: &Ctx, props: &Props, read: &Read, rotate: &Change<()>) -> Element {
    let button = ActionButton::new(
      "settings-mcp-token-rotate",
      ButtonKind::Ghost,
      ctx.t("settings.automation.server.token.rotateShort"),
    )
    .icon(Icon::RefreshCw);
    if !self.can_rotate(props, read, rotate) {
      return button.disabled().into();
    }
    button.on_click(self.ask_rotate()).into()
  }

  fn can_rotate(&self, props: &Props, read: &Read, rotate: &Change<()>) -> bool {
    let unreadable = read.failure.is_some() || matches!(props.state, ServerState::TokenUnreadable { .. });
    props.server.is_some()
      && (read.readable || unreadable)
      && !self.confirming.get()
      && !rotate.state().get().is_pending()
  }

  /// Opens the question under the row, unless synthetic input brought the press.
  fn ask_rotate(&self) -> impl Fn() + Send + Sync + 'static {
    let (confirming, blocked, rotated) = (self.confirming.clone(), self.blocked.clone(), self.rotated.clone());
    move || {
      if !refused(&blocked) {
        rotated.set(false);
        confirming.set(true);
      }
    }
  }

  /// Opens the question when the route starts asking it, as a press would; only a person's answer rotates.
  fn follow_route(&self, props: &Props) {
    let asks = props.rotation && props.server.is_some();
    let mut asked = self.asked.lock();
    if asks && !*asked {
      self.rotated.set(false);
      self.confirming.set(true);
    }
    *asked = asks;
  }

  /// What came of the last Copy.
  fn copy_line(&self, ctx: &Ctx) -> Option<Element> {
    match self.copied.get() {
      Copied::Not => None,
      Copied::Done => Some(outcome(
        "settings-mcp-token-copied",
        &ctx.t("settings.automation.server.token.copied"),
      )),
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
