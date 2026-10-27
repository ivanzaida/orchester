//! *Let agents connect to Orchester* (`Let agents connect to Orchester` `v7cMEO` in `04 · Automation`): its switch,
//! one line on what off means, and what it does in the row's tooltip. Off, nothing listens and the token stays stored.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{InputSource, input_source},
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::messages::Message;

use super::{Change, change};
use crate::{
  mcp::{AppMcp, ServerState},
  screens::settings::save_state::{Outcome, SaveState},
  ui::{
    field::field_error,
    message::message_text,
    setting_rows::SettingRow,
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
    let switch_action: Change<bool> = change(ctx, props.server.clone(), AppMcp::set_enabled);
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
    let (hint, info) = if off {
      (
        "settings.automation.server.enabled.hintOff",
        "settings.automation.server.enabled.off",
      )
    } else {
      (
        "settings.automation.server.enabled.hint",
        "settings.automation.server.enabled.description",
      )
    };
    let problems = problems(ctx, self.blocked.get(), pending.error.as_ref());
    SettingRow::new("settings-mcp-enabled", title)
      .hint(ctx.t(hint))
      .info(ctx.t(info))
      .control(switch)
      .below_all(problems)
      .render(ctx)
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
fn problems(ctx: &Ctx, blocked: bool, failed: Option<&Message>) -> Vec<Element> {
  let mut problems = Vec::new();
  if blocked {
    problems.push(
      field_error(
        "settings-mcp-enabled-blocked",
        &ctx.t("settings.automation.server.blocked"),
      )
      .into(),
    );
  }
  if let Some(reason) = failed {
    let reason = message_text(ctx, reason);
    problems.push(
      field_error(
        "settings-mcp-enabled-error",
        &ctx.t_args("settings.automation.server.notChanged", [("reason", &*reason)]),
      )
      .into(),
    );
  }
  problems
}
