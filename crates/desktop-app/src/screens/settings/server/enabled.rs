//! *Let agents connect to Orchester* (`Form row` `XaNIz` with the server's state and a switch, `lye6f`, `eTrCO`):
//! off, nothing listens and the token stays stored. While the token cannot be read from the secret store, a danger
//! notice with *Try again* follows the row.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{InputSource, input_source},
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::messages::Message;

use super::{Change, change, state_label, token::STORE_KEY};
use crate::{
  mcp::{AppMcp, ServerState},
  screens::settings::save_state::{Outcome, SaveState},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    form_section::{Label, Place, side_row},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    toggle::{Toggle, ToggleKind},
  },
};

const SWITCH_ID: &str = "settings-mcp-enabled-switch";

#[derive(Clone)]
pub struct Props {
  pub server: Option<AppMcp>,
  pub state: ServerState,
}

/// Compared by what the row shows; the server is the window's one.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.state == other.state && self.server.is_some() == other.server.is_some()
  }
}

impl DevtoolsInspectable for Props {}

pub struct EnabledRow {
  /// A change the row did not take because an MCP client's input brought it.
  blocked: Signal<bool>,
}

impl Component for EnabledRow {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      blocked: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    // Hooks in the same order on every render.
    let switch_action: Change<bool> = change(ctx, props.server.clone(), AppMcp::set_enabled);
    let again: Change<()> = change(ctx, props.server.clone(), |server, ()| {
      server.try_again();
      Ok(())
    });
    let pending = switch_action.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(SWITCH_ID, Outcome::of(&pending));
    }
    let off = props.state == ServerState::Off;
    let title = ctx.t("settings.automation.server.enabled.title");
    let mut switch = Toggle::new(SWITCH_ID, ToggleKind::Switch, !off, title.clone()).hide_label();
    // While a change runs, and in a window without a server, the switch stays as it is.
    if props.server.is_some() && !pending.is_pending() {
      let blocked = self.blocked.clone();
      switch = switch.on_change(move |on| {
        if refused(&blocked) {
          return;
        }
        switch_action.run(on);
      });
    }
    let description = ctx.t(if off {
      "settings.automation.server.enabled.off"
    } else {
      "settings.automation.server.enabled.description"
    });
    let control = Row::new()
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(state_label(ctx, &props.state))
      .child(switch);
    let row = side_row(
      Label::new("settings-mcp-enabled", title).description(description),
      control,
      Place::Within,
      false,
    );
    let notice = unreadable(ctx, &props, again, &self.blocked);
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(row)
      .with_children(problems(ctx, self.blocked.get(), pending.error.as_ref()))
      .with_children(notice)
  }
}

/// Whether synthetic input brought the change: it is not taken, and the row says so.
pub fn refused(blocked: &Signal<bool>) -> bool {
  let synthetic = input_source() == InputSource::Synthetic;
  if synthetic {
    tracing::warn!("synthetic input reached a control of the MCP server's settings; not taken");
  }
  blocked.set(synthetic);
  synthetic
}

/// Why a change was not taken or not made, under the row.
fn problems(ctx: &Ctx, blocked: bool, failed: Option<&Message>) -> Vec<Row> {
  let mut problems = Vec::new();
  if blocked {
    problems.push(field_error(
      "settings-mcp-enabled-blocked",
      &ctx.t("settings.automation.server.blocked"),
    ));
  }
  if let Some(reason) = failed {
    let reason = message_text(ctx, reason);
    problems.push(field_error(
      "settings-mcp-enabled-error",
      &ctx.t_args("settings.automation.server.notChanged", [("reason", &*reason)]),
    ));
  }
  problems
}

/// While the token cannot be read: what the secret store said, that the server is off until it can, and *Try
/// again*.
fn unreadable(ctx: &Ctx, props: &Props, again: Change<()>, blocked: &Signal<bool>) -> Option<Element> {
  let ServerState::TokenUnreadable { reason } = &props.state else {
    return None;
  };
  let store = ctx.t(STORE_KEY);
  let reason = message_text(ctx, reason);
  let mut button = ActionButton::new(
    "settings-mcp-unreadable-again",
    ButtonKind::Secondary,
    ctx.t("settings.automation.server.token.tryAgain"),
  )
  .icon(Icon::RotateCcw);
  if props.server.is_none() || again.state().get().is_pending() {
    button = button.disabled();
  } else {
    let blocked = blocked.clone();
    button = button.on_click(move || {
      if !refused(&blocked) {
        again.run(());
      }
    });
  }
  Some(
    Notice::new(
      "settings-mcp-unreadable",
      NoticeTone::Danger,
      ctx.t_args("settings.automation.server.token.unreadable", [("store", &*store)]),
    )
    .body(ctx.t_args(
      "settings.automation.server.token.unreadableBody",
      [("reason", &*reason)],
    ))
    .action(button)
    .into(),
  )
}
