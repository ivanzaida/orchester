//! Gate and permissions controls; the workspace decides what the profile can narrow.
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::Element,
};
use orchester_core::{
  permission::{StepCommands, StepNetwork},
  resource::{RepositoryAccess, Step, StepKind},
};
use orchester_workspace::pipelines::{data::ProfileChoice, narrowing};

use super::{parts, state::State};
use crate::ui::{
  icon::Icon,
  inspector::note,
  toggle::{Toggle, ToggleKind},
};

pub fn rows(ctx: &Ctx, state: &State, step: &Step, locked: bool) -> Element {
  let mut body = Column::new()
    .spacing(SpacingSize::Md)
    .child(parts::section(ctx, "pipelines.gateAccess"));
  let (edit, key) = (state.clone(), step.key.clone());
  let gate = Toggle::new(
    "step-gate",
    ToggleKind::Switch,
    step.approval_gate,
    ctx.t("pipelines.gate"),
  );
  body = body.child(if locked {
    gate
  } else {
    gate.on_change(move |on| edit.update(|s| s.change(&key, |step| step.approval_gate = on)))
  });
  let StepKind::Agent {
    profile, repository, ..
  } = &step.kind
  else {
    return body.into();
  };
  let editing = state.get();
  let choice = profile.and_then(|id| editing.opened.assignables.profile(id));
  let (edit, key) = (state.clone(), step.key.clone());
  let writes = Toggle::new(
    "step-write",
    ToggleKind::Switch,
    *repository == RepositoryAccess::Write,
    ctx.t("pipelines.canWrite"),
  );
  body = body.child(
    if !locked && (choice.is_some_and(narrowing::can_write) || *repository == RepositoryAccess::Write) {
      writes.on_change(move |on| {
        edit.update(|s| {
          s.change(&key, |step| {
            if let StepKind::Agent { repository, .. } = &mut step.kind {
              *repository = if on {
                RepositoryAccess::Write
              } else {
                RepositoryAccess::ReadOnly
              };
            }
          })
        })
      })
    } else {
      writes
    },
  );
  let Some(choice) = choice else {
    return body
      .child(note("step-narrow-note", Icon::Info, &ctx.t("pipelines.narrowHint")))
      .into();
  };
  body = body.child(parts::section(ctx, "pipelines.permissions"));
  if !narrowing::can_block_network(choice) || narrowing::commands(choice).is_empty() {
    body = body.child(parts::note(&ctx.t("pipelines.narrowUnavailable")));
  }
  body = body.child(command_rows(state, step, choice, locked));
  body.child(network_rows(ctx, state, step, choice, locked)).into()
}
fn command_rows(state: &State, step: &Step, choice: &ProfileChoice, locked: bool) -> Column {
  let StepKind::Agent { permissions, .. } = &step.kind else {
    return Column::new();
  };
  let mut commands = Column::new().spacing(SpacingSize::Xs);
  let all = narrowing::commands(choice);
  let current: Vec<String> = match &permissions.commands {
    StepCommands::AsProfile => all.clone(),
    StepCommands::Only(only) => only.entries().iter().map(ToString::to_string).collect(),
  };
  for (index, command) in all.iter().enumerate() {
    let included = current.contains(command);
    let chosen = command.clone();
    let current = current.clone();
    let (edit, key, choice, off, ask) = (
      state.clone(),
      step.key.clone(),
      choice.clone(),
      permissions.network == StepNetwork::Off,
      permissions.interactive_tool_approvals,
    );
    let toggle = Toggle::new(
      format!("step-command-{index}"),
      ToggleKind::Checkbox,
      included,
      command.as_str(),
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
        apply(&edit, &key, &choice, Some(&only), off, ask);
      })
    });
  }
  commands
}
fn network_rows(ctx: &Ctx, state: &State, step: &Step, choice: &ProfileChoice, locked: bool) -> Element {
  let StepKind::Agent { permissions, .. } = &step.kind else {
    return Column::new().into();
  };
  let all = narrowing::commands(choice);
  let current: Vec<String> = match &permissions.commands {
    StepCommands::AsProfile => all.clone(),
    StepCommands::Only(only) => only.entries().iter().map(ToString::to_string).collect(),
  };
  let mut body = Column::new().spacing(SpacingSize::Md);
  let (edit, key, choice_clone, only, ask) = (
    state.clone(),
    step.key.clone(),
    choice.clone(),
    current.clone(),
    permissions.interactive_tool_approvals,
  );
  let network = Toggle::new(
    "step-network-off",
    ToggleKind::Switch,
    permissions.network == StepNetwork::Off,
    ctx.t("pipelines.networkOff"),
  );
  let list = (!all.is_empty()).then_some(only);
  body = body.child(if !locked && narrowing::can_block_network(choice) {
    network.on_change(move |off| apply(&edit, &key, &choice_clone, list.as_deref(), off, ask))
  } else {
    network
  });
  let (edit, key, choice, only, off) = (
    state.clone(),
    step.key.clone(),
    choice.clone(),
    current,
    permissions.network == StepNetwork::Off,
  );
  let list = (!all.is_empty()).then_some(only);
  let asks = Toggle::new(
    "step-ask-tools",
    ToggleKind::Switch,
    permissions.interactive_tool_approvals,
    ctx.t("pipelines.askTools"),
  );
  body
    .child(if locked {
      asks
    } else {
      asks.on_change(move |ask| apply(&edit, &key, &choice, list.as_deref(), off, ask))
    })
    .child(note("step-narrow-note", Icon::Info, &ctx.t("pipelines.narrowHint")))
    .into()
}
fn apply(
  state: &State,
  key: &orchester_core::resource::StepKey,
  choice: &ProfileChoice,
  only: Option<&[String]>,
  off: bool,
  ask: bool,
) {
  let result = narrowing::permissions(only, off, ask, choice);
  state.update(|s| {
    s.field("step-permissions", result, |s, value| {
      s.change(key, |step| {
        if let StepKind::Agent { permissions, .. } = &mut step.kind {
          *permissions = value;
        }
      })
    })
  });
}
