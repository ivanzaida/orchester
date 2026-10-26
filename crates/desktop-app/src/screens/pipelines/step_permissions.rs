//! A step's Permissions page (`02e · step permissions` `T46Jq`): its commands, narrowed from the profile's in a
//! picker, *Turn network off* and *Ask when a tool needs approval*, what the profile allows, and *Use profile
//! commands and network* once the step narrows them. A step can only narrow what its profile and its connection
//! enforce on this platform; a choice they cannot is locked with why (`03b` `g6Ecy`). Turning Ask off asks before
//! saving (`super::save`).
use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  permission::{StepCommands, StepNetwork, StepPermissions},
  resource::{Step, StepKind},
};
use orchester_workspace::pipelines::{data::ProfileChoice, narrowing};

use super::{state::State, step_fields::Props};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  row_group::{GroupRow, RowGroup, note},
  toggle::{Toggle, ToggleKind},
};

/// The page's groups under its head.
pub fn page(ctx: &mut Ctx, props: &Props) -> Vec<Element> {
  let editing = props.state.get();
  let StepKind::Agent {
    profile: Some(profile), ..
  } = props.step.kind
  else {
    return vec![note("step-narrow-note", &ctx.t("pipelines.narrowHint")).into()];
  };
  let Some(choice) = editing.opened.assignables.profile(profile).cloned() else {
    return vec![note("step-narrow-note", &ctx.t("pipelines.narrowHint")).into()];
  };
  let picker =
    ctx.mount_keyed::<super::command_picker::Commands>(&format!("step-commands-{}", props.step.key), props.clone());
  let commands = RowGroup::new("step-commands", ctx.t("pipelines.commands"))
    .lead(inheritance(ctx, &choice))
    .row(GroupRow::new("step-command-summary", ctx.t("pipelines.commands")).end(picker));
  let mut parts: Vec<Element> = vec![commands.into(), network(ctx, props, &choice)];
  parts.extend(reset(ctx, props));
  parts
}

/// What the profile allows, which a step can only narrow (`g2zgHb`).
fn inheritance(ctx: &Ctx, choice: &ProfileChoice) -> std::sync::Arc<str> {
  let commands = narrowing::commands(choice);
  if commands.is_empty() {
    ctx.t("pipelines.narrowHint")
  } else {
    ctx.t_args(
      "pipelines.profileCommandsHint",
      [
        ("profile", choice.name.as_ref()),
        ("commands", commands.join(", ").as_str()),
      ],
    )
  }
}

/// The commands the step allows now: the profile's, or its own narrowing.
fn current(step: &Step, choice: &ProfileChoice) -> (Vec<String>, bool, bool) {
  let StepKind::Agent { permissions, .. } = &step.kind else {
    return (Vec::new(), false, false);
  };
  let commands = match &permissions.commands {
    StepCommands::AsProfile => narrowing::commands(choice),
    StepCommands::Only(only) => only.entries().iter().map(ToString::to_string).collect(),
  };
  (
    commands,
    permissions.network == StepNetwork::Off,
    permissions.interactive_tool_approvals,
  )
}

/// The command picker's rows: one checkbox per command the profile allows.
pub(super) fn command_rows(state: &State, step: &Step, choice: &ProfileChoice, locked: bool) -> Column {
  let mut commands = Column::new().spacing(lurq::app::theme::SpacingSize::Xs);
  let (current, off, ask) = current(step, choice);
  for (index, command) in narrowing::commands(choice).iter().enumerate() {
    let toggle = Toggle::new(
      format!("step-command-{index}"),
      ToggleKind::Checkbox,
      current.contains(command),
      command.as_str(),
    );
    let (edit, key, choice, chosen, current) = (
      state.clone(),
      step.key.clone(),
      choice.clone(),
      command.clone(),
      current.clone(),
    );
    commands = commands.child(if locked {
      toggle
    } else {
      toggle.on_change(move |on| {
        let mut only = current.clone();
        only.retain(|c| c != &chosen);
        if on {
          only.push(chosen.clone());
        }
        apply(&edit, &key, &choice, (Some(&only), off, ask));
      })
    });
  }
  commands
}

/// Network and tools (`x402h`).
fn network(ctx: &mut Ctx, props: &Props, choice: &ProfileChoice) -> Element {
  let (commands, off, ask) = current(&props.step, choice);
  let list = (!narrowing::commands(choice).is_empty()).then_some(commands);
  let can_block = narrowing::can_block_network(choice);
  let (edit, key, picked, only) = (
    props.state.clone(),
    props.step.key.clone(),
    choice.clone(),
    list.clone(),
  );
  let network = Toggle::new(
    "step-network-off",
    ToggleKind::Switch,
    off,
    ctx.t("pipelines.networkOff"),
  )
  .hide_label();
  let network = if !props.locked && can_block {
    network.on_change(move |off| apply(&edit, &key, &picked, (only.as_deref(), off, ask)))
  } else {
    network
  };
  let network: Element = if can_block {
    network.into()
  } else {
    crate::ui::tooltip::Tooltip::new("step-network-reason", ctx.t("pipelines.networkUnavailable")).wrap(ctx, network)
  };
  let network = GroupRow::new("step-network-row", ctx.t("pipelines.networkOff")).end(network);
  let network = if can_block { network } else { network.muted() };
  let (edit, key, picked) = (props.state.clone(), props.step.key.clone(), choice.clone());
  let asks = Toggle::new("step-ask-tools", ToggleKind::Switch, ask, ctx.t("pipelines.askTools")).hide_label();
  let asks = if props.locked {
    asks
  } else {
    asks.on_change(move |ask| apply(&edit, &key, &picked, (list.as_deref(), off, ask)))
  };
  RowGroup::new("step-network", ctx.t("pipelines.networkTools"))
    .row(network)
    .row(GroupRow::new("step-ask-tools-row", ctx.t("pipelines.askTools")).end(asks))
    .into()
}

fn apply(
  state: &State,
  key: &orchester_core::resource::StepKey,
  choice: &ProfileChoice,
  (only, off, ask): (Option<&[String]>, bool, bool),
) {
  let result = narrowing::permissions(only, off, ask, choice);
  state.update(|s| {
    s.field(&format!("{key}:step-permissions"), result, |s, value| {
      s.change(key, |step| {
        if let StepKind::Agent { permissions, .. } = &mut step.kind {
          // The narrowing is chosen anew; whether the step merges the base is not part of it.
          *permissions = StepPermissions {
            takes_base_moves: permissions.takes_base_moves,
            ..value
          };
        }
      })
    })
  });
}

/// *Use profile commands and network*, once the step narrows them.
fn reset(ctx: &Ctx, props: &Props) -> Option<Element> {
  let StepKind::Agent { permissions, .. } = &props.step.kind else {
    return None;
  };
  let inherited = narrowing::inherit_profile(permissions);
  if inherited == *permissions {
    return None;
  }
  let (edit, key) = (props.state.clone(), props.step.key.clone());
  let reset = ActionButton::new(
    "step-permissions-reset",
    ButtonKind::Ghost,
    ctx.t("pipelines.resetPermissions"),
  )
  .icon(Icon::RotateCcw)
  .on_click(move || {
    edit.update(|s| {
      s.field(&format!("{key}:step-permissions"), Ok(inherited.clone()), |s, value| {
        s.change(&key, |step| {
          if let StepKind::Agent { permissions, .. } = &mut step.kind {
            *permissions = value;
          }
        });
      });
    });
  });
  Some(
    lurq::components::Row::new()
      .width(Dimension::Pct(100.0))
      .child(if props.locked { reset.disabled() } else { reset })
      .into(),
  )
}
