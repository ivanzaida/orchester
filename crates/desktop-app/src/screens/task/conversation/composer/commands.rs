//! A run's own commands in its Conversation composer's slash menu (ORC-321, `06` `yKWQx`): *Stop turn*, which stops
//! the turn of the agent at work as the working line's *Stop turn* and Esc in the composer do, and says why it cannot
//! while no turn runs or the turn cannot be stopped alone. The head names the run's agent and its step. Compact, Model
//! and effort, and Permissions join it once their controls exist (ORC-309, ORC-319 and the run model switch); until
//! then they are not offered. The project's published skills follow them (ORC-325), greyed while an agent the message
//! may reach cannot take one.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::ElementRef, node::Element};
use orchester_core::conversation::Recipient;
use orchester_workspace::{
  attachments::reachable_providers,
  skills::offered::taken_by,
  tasks::conversation::{WorkingAgent, composer::ComposerMode},
};

use super::{super::timeline::step_label, Composing};
use crate::ui::{
  icon::Icon,
  slash::{self, Does, SkillsOffer, SlashCommand, SlashField, SlashMenuProps},
};

/// The slash menu over the box `anchor` for the run's state `mode`, for a message to `recipient`.
pub(super) fn menu(
  ctx: &mut Ctx,
  (field, anchor): (&SlashField, &ElementRef),
  composing: &Composing,
  (mode, recipient): (&ComposerMode, Recipient),
) -> Element {
  let note = composing.detail.run.as_ref().and_then(|run| {
    let step = run.current_step()?;
    let agent = step.agent.as_deref()?;
    let step = step_label(ctx, step.number, &step.name);
    Some(ctx.t_args("composer.slash.agentStep", [("agent", agent), ("step", &*step)]))
  });
  let providers = composing
    .detail
    .run
    .as_ref()
    .map(|run| reachable_providers(run, recipient))
    .unwrap_or_default();
  let skills = SkillsOffer {
    root: composing.project.open.repo_root.clone(),
    revision: composing.project.feed.saves.get(),
    refused: (!taken_by(&providers)).then(|| ctx.t("composer.skill.notTaken")),
  };
  slash::menu(
    ctx,
    SlashMenuProps {
      id: "composer",
      field: field.clone(),
      anchor: anchor.clone(),
      commands: vec![stop_turn(ctx, composing, mode)],
      skills,
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
    does: run.map(|stop| Does::Run(stop as Arc<dyn Fn() + Send + Sync>)),
  }
}
