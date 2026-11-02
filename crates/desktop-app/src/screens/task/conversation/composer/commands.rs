//! A run's own commands in its Conversation composer's slash menu (ORC-321, `06` `yKWQx`): *Stop turn*, which stops
//! the turn of the agent at work as the working line's *Stop turn* and Esc in the composer do, and says why it cannot
//! while no turn runs or the turn cannot be stopped alone. The head names the run's agent and its step. Compact, Model
//! and effort, and Permissions join it once their controls exist (ORC-309, ORC-319 and the run model switch); until
//! then they are not offered.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::ElementRef, node::Element};
use orchester_workspace::tasks::conversation::{WorkingAgent, composer::ComposerMode};

use super::{super::timeline::step_label, Composing};
use crate::ui::{
  icon::Icon,
  slash::{self, SlashCommand, SlashField, SlashMenuProps},
};

/// The slash menu over the box `anchor` for the run's state `mode`.
pub(super) fn menu(
  ctx: &mut Ctx,
  (field, anchor): (&SlashField, &ElementRef),
  composing: &Composing,
  mode: &ComposerMode,
) -> Element {
  let note = composing.detail.run.as_ref().and_then(|run| {
    let step = run.current_step()?;
    let agent = step.agent.as_deref()?;
    let step = step_label(ctx, step.number, &step.name);
    Some(ctx.t_args("composer.slash.agentStep", [("agent", agent), ("step", &*step)]))
  });
  slash::menu(
    ctx,
    SlashMenuProps {
      id: "composer",
      field: field.clone(),
      anchor: anchor.clone(),
      commands: vec![stop_turn(ctx, composing, mode)],
      note,
      send: None,
    },
  )
}

/// *Stop turn*: while an agent works in a turn that can be stopped alone, it stops; otherwise why not.
fn stop_turn(ctx: &Ctx, composing: &Composing, mode: &ComposerMode) -> SlashCommand {
  let with_agent = |key: &str, agent: &str| ctx.t_args(key, [("agent", agent)]);
  let (detail, run) = match mode {
    ComposerMode::Steer(WorkingAgent {
      agent,
      stops_turn: false,
      ..
    }) => (with_agent("composer.slash.command.stopTurn.cannot", agent), None),
    ComposerMode::Steer(WorkingAgent { agent, .. }) if composing.stopping() => {
      (with_agent("composer.slash.command.stopTurn.stopping", agent), None)
    }
    ComposerMode::Steer(WorkingAgent { agent, .. }) => match composing.stop_turn() {
      Some(stop) => (with_agent("composer.slash.command.stopTurn.detail", agent), Some(stop)),
      None => (ctx.t("composer.slash.command.stopTurn.noTurn"), None),
    },
    ComposerMode::Answer { agent, .. } => (with_agent("composer.slash.command.stopTurn.noTurnAnswer", agent), None),
    _ => (ctx.t("composer.slash.command.stopTurn.noTurn"), None),
  };
  SlashCommand {
    name: "stop-turn",
    icon: Icon::Square,
    label: ctx.t("composer.slash.command.stopTurn.label"),
    detail,
    run: run.map(|stop| stop as Arc<dyn Fn() + Send + Sync>),
  }
}
