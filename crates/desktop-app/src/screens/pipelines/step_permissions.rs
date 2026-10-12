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
  inspector::{control, note},
  pressable::Focus,
  tabs::{Segment, SegmentFace, segmented},
  toggle::{Toggle, ToggleKind},
};

pub fn rows(ctx: &mut Ctx, state: &State, step: &Step, locked: bool) -> Element {
  let mut body = Column::new()
    .spacing(SpacingSize::Xs)
    .child(parts::section(ctx, "pipelines.gateAccess"));
  let (edit, key) = (state.clone(), step.key.clone());
  let gate = Toggle::new(
    "step-gate",
    ToggleKind::Switch,
    step.approval_gate,
    ctx.t("pipelines.gate"),
  );
  body = body.child(switch_row(
    ctx,
    "pipelines.gate",
    if locked {
      gate
    } else {
      gate.on_change(move |on| edit.update(|s| s.change(&key, |step| step.approval_gate = on)))
    },
  ));
  let StepKind::Agent {
    profile, repository, ..
  } = &step.kind
  else {
    return body.into();
  };
  let editing = state.get();
  let choice = profile.and_then(|id| editing.opened.assignables.profile(id));
  let (edit, key) = (state.clone(), step.key.clone());
  let writable = !locked && (choice.is_some_and(narrowing::can_write) || *repository == RepositoryAccess::Write);
  let segments = [(false, "pipelines.readOnly"), (true, "pipelines.canWrite")].map(|(write, label)| {
    let (edit, key) = (edit.clone(), key.clone());
    Segment::new(
      if write { "step-write" } else { "step-read" },
      SegmentFace::Text(ctx.t(label)),
      (*repository == RepositoryAccess::Write) == write,
      move || {
        if writable {
          edit.update(|s| {
            s.change(&key, |step| {
              if let StepKind::Agent { repository, .. } = &mut step.kind {
                *repository = if write {
                  RepositoryAccess::Write
                } else {
                  RepositoryAccess::ReadOnly
                };
              }
            })
          });
        }
      },
    )
  });
  body = body.child(control(
    "step-access",
    &ctx.t("pipelines.access"),
    segmented("step-access-choice", segments, Focus::Form),
  ));
  let Some(choice) = choice else {
    return body
      .child(note("step-narrow-note", Icon::Info, &ctx.t("pipelines.narrowHint")))
      .into();
  };
  body.child(permission_section(ctx, state, step, choice, locked)).into()
}

fn permission_section(ctx: &mut Ctx, state: &State, step: &Step, choice: &ProfileChoice, locked: bool) -> Element {
  let mut body = Column::new().spacing(SpacingSize::Xs);
  let context = ctx.t_args(
    if let StepKind::Agent { permissions, .. } = &step.kind
      && *permissions != narrowing::inherit_profile(permissions)
    {
      "pipelines.narrowerThan"
    } else {
      "pipelines.permissionsFrom"
    },
    [("profile", choice.name.as_ref())],
  );
  body = body.child(
    crate::ui::inspector::section("step-permissions", SpacingSize::Xs).child(crate::ui::inspector::section_title(
      "step-permissions-title",
      &ctx.t("pipelines.permissions"),
      Some(&context),
    )),
  );
  let picker = ctx.mount_keyed::<super::command_picker::Commands>(
    &format!("step-commands-{}", step.key),
    super::step_fields::Props {
      state: state.clone(),
      step: step.clone(),
      locked,
      navigation_locked: locked,
    },
  );
  body = body.child(control("step-command-summary", &ctx.t("pipelines.commands"), picker));
  body
    .child(network_rows(ctx, state, step, choice, locked))
    .child(reset(ctx, state, step, locked))
    .into()
}
pub(super) fn command_rows(state: &State, step: &Step, choice: &ProfileChoice, locked: bool) -> Column {
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
fn network_rows(ctx: &mut Ctx, state: &State, step: &Step, choice: &ProfileChoice, locked: bool) -> Element {
  let StepKind::Agent { permissions, .. } = &step.kind else {
    return Column::new().into();
  };
  let all = narrowing::commands(choice);
  let current: Vec<String> = match &permissions.commands {
    StepCommands::AsProfile => all.clone(),
    StepCommands::Only(only) => only.entries().iter().map(ToString::to_string).collect(),
  };
  let mut body = Column::new().spacing(SpacingSize::Xs);
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
  let network_row = switch_row(
    ctx,
    "pipelines.networkOff",
    if !locked && narrowing::can_block_network(choice) {
      network.on_change(move |off| apply(&edit, &key, &choice_clone, list.as_deref(), off, ask))
    } else {
      network
    },
  );
  body = body.child(if narrowing::can_block_network(choice) {
    network_row.into()
  } else {
    crate::ui::tooltip::Tooltip::new("step-network-reason", ctx.t("pipelines.networkUnavailable"))
      .wrap(ctx, network_row)
  });
  let inheritance = inheritance_note(ctx, choice);
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
    .child(switch_row(
      ctx,
      "pipelines.askTools",
      if locked {
        asks
      } else {
        asks.on_change(move |ask| apply(&edit, &key, &choice, list.as_deref(), off, ask))
      },
    ))
    .child(inheritance)
    .into()
}
fn inheritance_note(ctx: &Ctx, choice: &ProfileChoice) -> lurq::components::Row {
  let commands = narrowing::commands(choice);
  let text = if commands.is_empty() {
    ctx.t("pipelines.narrowHint")
  } else {
    ctx.t_args(
      "pipelines.profileCommandsHint",
      [
        ("profile", choice.name.as_ref()),
        ("commands", commands.join(", ").as_str()),
      ],
    )
  };
  note("step-narrow-note", Icon::Info, &text)
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
    s.field(&format!("{key}:step-permissions"), result, |s, value| {
      s.change(key, |step| {
        if let StepKind::Agent { permissions, .. } = &mut step.kind {
          *permissions = value;
        }
      })
    })
  });
}
fn switch_row(ctx: &Ctx, key: &str, toggle: Toggle) -> lurq::components::Row {
  lurq::components::Row::new()
    .width(lurq::node::dimension::Dimension::Pct(100.0))
    .min_height(lurq::node::dimension::Dimension::Px(28.0))
    .spacing(SpacingSize::Sm)
    .align_items(lurq::layout::Alignment::Center)
    .child(parts::caption(&ctx.t(key)).flex(1.0))
    .child(toggle.hide_label())
}
pub(super) fn command_summary(ctx: &Ctx, step: &Step) -> String {
  if let StepKind::Agent { permissions, .. } = &step.kind
    && let StepCommands::Only(only) = &permissions.commands
  {
    return ctx
      .t_args(
        "pipelines.narrowedCommands",
        [("count", only.entries().len().to_string())],
      )
      .to_string();
  }
  ctx.t("pipelines.inherit").to_string()
}

fn reset(ctx: &Ctx, state: &State, step: &Step, locked: bool) -> Column {
  use crate::ui::button::{ActionButton, ButtonKind};

  let body = Column::new();
  let StepKind::Agent { permissions, .. } = &step.kind else {
    return body;
  };
  let inherited = narrowing::inherit_profile(permissions);
  if inherited == *permissions {
    return body;
  }
  let (edit, key) = (state.clone(), step.key.clone());
  let reset = ActionButton::new(
    "step-permissions-reset",
    ButtonKind::Ghost,
    ctx.t("pipelines.resetPermissions"),
  )
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
  body.child(if locked { reset.disabled() } else { reset })
}
