//! An agent step's permissions, in place in *Gate and access* (`Commands` `bTWt9` … `Use profile commands and network`
//! `rXZV4` in P2b `EGEos`): *Commands*, one checkbox per command the profile allows, in the mono font, of which a step
//! can only keep fewer; *Turn network off*, drawn locked with why when the profile and its connection cannot enforce it
//! here; *Ask when a tool needs approval*; and *Use profile commands and network* once the step narrows the profile.
//! A step without a published profile has none of them: there is nothing to narrow. Each changes the edit the panel
//! holds; turning *Ask* off is a change Save asks the person to allow ([`super::super::save`]).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{dimension::Dimension, padding::Padding},
};
use orchester_core::{
  permission::{StepCommands, StepNetwork, StepPermissions},
  resource::{Step, StepKey, StepKind},
};
use orchester_workspace::{
  messages::Message,
  pipelines::{data::ProfileChoice, narrowing},
};

use super::{
  Open,
  gate::{OnSwitch, switch},
};
use crate::{
  screens::{
    pipeline_hub::edit::Edit,
    team_hub::rows::{self, Sub},
  },
  theme::{DISABLED_OPACITY, ExtraSpacing, ExtraTypography, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    toggle::{Toggle, ToggleKind},
    tooltip::Tooltip,
  },
};

/// What the step allows now: the commands it keeps (the profile's when it narrows none), whether its network is off and
/// whether it asks about tool calls.
#[derive(Clone)]
struct Now {
  commands: Vec<String>,
  network_off: bool,
  ask: bool,
}

impl Now {
  fn of(permissions: &StepPermissions, choice: &ProfileChoice) -> Self {
    let commands = match &permissions.commands {
      StepCommands::AsProfile => narrowing::commands(choice),
      // A command the profile no longer allows is not a box to tick; the check under the group says it is kept.
      StepCommands::Only(only) => {
        let listed = narrowing::commands(choice);
        only
          .entries()
          .iter()
          .map(ToString::to_string)
          .filter(|command| listed.contains(command))
          .collect()
      }
    };
    Self {
      commands,
      network_off: permissions.network == StepNetwork::Off,
      ask: permissions.interactive_tool_approvals,
    }
  }
}

/// How a row changes the step's permissions.
#[derive(Clone)]
struct Change {
  edit: Edit,
  key: StepKey,
  choice: ProfileChoice,
  now: Now,
}

impl Change {
  /// Makes `next` the step's narrowing, keeping whether it merges the base branch, which is not part of it.
  fn apply(&self, next: &Now) {
    let listed = !narrowing::commands(&self.choice).is_empty();
    let only = listed.then_some(next.commands.as_slice());
    // The rows offer only what the profile they were drawn with enforces here, and keep only its commands, so the check
    // refuses nothing they make; losing a refusal is right, as it could only follow a press nothing was offered for.
    let Ok(value) = narrowing::permissions(only, next.network_off, next.ask, &self.choice) else {
      return;
    };
    self.edit.change(|current| {
      current.update(&self.key, |step| {
        if let StepKind::Agent { permissions, .. } = &mut step.kind {
          *permissions = StepPermissions {
            takes_base_moves: permissions.takes_base_moves,
            ..value
          };
        }
      });
    });
  }
}

/// The rows of `step`'s permissions; none without a published profile.
pub(super) fn rows(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> Vec<Row> {
  let StepKind::Agent {
    profile: Some(profile),
    permissions,
    ..
  } = &step.kind
  else {
    return Vec::new();
  };
  let Some(choice) = open.assignables.profile(*profile).cloned() else {
    return Vec::new();
  };
  let now = Now::of(permissions, &choice);
  let change = (!open.locked).then(|| Change {
    edit: open.edit.clone(),
    key: step.key.clone(),
    choice: choice.clone(),
    now: now.clone(),
  });
  let mut rows = Vec::new();
  if !narrowing::commands(&choice).is_empty() {
    rows.push(commands_row(ctx, &choice, &now, change.as_ref()));
  }
  rows.push(network_row(ctx, &choice, &now, change.as_ref()));
  rows.push(ask_row(ctx, &now, change.as_ref()));
  rows.extend(reset_row(ctx, open, step, (permissions, &choice)));
  rows
}

/// Why the profile does not take the step's narrowing as it is stored, e.g. a command it no longer allows: Save is
/// refused for it until the step takes the profile's again.
fn refusal(permissions: &StepPermissions, choice: &ProfileChoice) -> Option<Message> {
  let commands = match &permissions.commands {
    StepCommands::AsProfile => None,
    StepCommands::Only(only) => Some(only.entries().iter().map(ToString::to_string).collect::<Vec<_>>()),
  };
  narrowing::permissions(
    commands.as_deref(),
    permissions.network == StepNetwork::Off,
    permissions.interactive_tool_approvals,
    choice,
  )
  .err()
}

/// *Commands*: a checkbox per command the profile allows (`Command · cargo` `e7iwp`), 6 apart, 10 in from the value's
/// start.
fn commands_row(ctx: &Ctx, choice: &ProfileChoice, now: &Now, change: Option<&Change>) -> Row {
  let mut list = Column::new()
    .id("pipeline-hub-step-commands")
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role());
  for (index, command) in narrowing::commands(choice).into_iter().enumerate() {
    let toggle = Toggle::new(
      format!("pipeline-hub-step-command-{index}"),
      ToggleKind::Checkbox,
      now.commands.contains(&command),
      command.as_str(),
    )
    .label_style(ExtraTypography::PropertyMono.role());
    let toggle = match change.cloned() {
      Some(change) => toggle.on_change(move |on| {
        let mut next = change.now.clone();
        next.commands.retain(|kept| kept != &command);
        if on {
          next.commands.push(command.clone());
        }
        change.apply(&next);
      }),
      None => toggle,
    };
    list = list.child(toggle);
  }
  let sub = ctx.t_args("pipeline_hub.stepEdit.commandsSub", [("profile", choice.name.as_ref())]);
  rows::row(
    "pipeline-hub-step-commands-row",
    &ctx.t("pipeline_hub.stepEdit.commands"),
    Some(Sub::quiet(sub)),
    list,
  )
  .align_items(Alignment::Start)
}

/// *Turn network off*: locked, its label faded and why in its switch's tooltip, when the profile and its connection
/// cannot enforce it here.
fn network_row(ctx: &mut Ctx, choice: &ProfileChoice, now: &Now, change: Option<&Change>) -> Row {
  let id = "pipeline-hub-step-network-row";
  let label = ctx.t("pipeline_hub.stepEdit.networkOff");
  let can_block = narrowing::can_block_network(choice);
  let on_change = change.filter(|_| can_block).cloned().map(|change| {
    Box::new(move |off: bool| {
      let mut next = change.now.clone();
      next.network_off = off;
      change.apply(&next);
    }) as OnSwitch
  });
  let toggle = switch(
    "pipeline-hub-step-network-off",
    label.clone(),
    now.network_off,
    on_change,
  );
  let mut name = Text::new(&label)
    .id(format!("{id}-label"))
    .variant(HubText::Status.role());
  if can_block {
    return rows::keyed(id, name, None, toggle);
  }
  name = name.opacity(DISABLED_OPACITY);
  let reason = Tooltip::new(
    "pipeline-hub-step-network-reason",
    ctx.t("pipeline_hub.stepEdit.networkUnavailable"),
  )
  .end()
  .wrap(ctx, toggle);
  rows::keyed(id, name, None, Row::new().flex(1.0).justify(Justify::End).child(reason)).describe("locked", "true")
}

/// *Ask when a tool needs approval*.
fn ask_row(ctx: &Ctx, now: &Now, change: Option<&Change>) -> Row {
  let label = ctx.t("pipeline_hub.stepEdit.askTools");
  let on_change = change.cloned().map(|change| {
    Box::new(move |ask: bool| {
      let mut next = change.now.clone();
      next.ask = ask;
      change.apply(&next);
    }) as OnSwitch
  });
  let toggle = switch("pipeline-hub-step-ask-tools", label.clone(), now.ask, on_change);
  rows::row_one_line("pipeline-hub-step-ask-tools-row", &label, toggle)
}

/// *Use profile commands and network* (`Reset` `uhKjE`), the group's last line, once the step narrows its profile's
/// commands or network: it takes the profile's again and keeps the step's other choices. A narrowing the profile does
/// not take is said over it, as the field error the kit draws (no design frame has one).
fn reset_row(
  ctx: &Ctx,
  open: &Open<'_>,
  step: &Step,
  (permissions, choice): (&StepPermissions, &ProfileChoice),
) -> Option<Row> {
  let inherited = narrowing::inherit_profile(permissions);
  if inherited == *permissions {
    return None;
  }
  let button = ActionButton::new(
    "pipeline-hub-step-permissions-reset",
    ButtonKind::Ghost,
    ctx.t("pipeline_hub.stepEdit.resetPermissions"),
  )
  .icon(Icon::RotateCcw);
  let button = if open.locked {
    button.disabled()
  } else {
    let (edit, key) = (open.edit.clone(), step.key.clone());
    button.on_click(move || {
      edit.change(|current| {
        current.update(&key, |step| {
          if let StepKind::Agent { permissions, .. } = &mut step.kind {
            *permissions = inherited.clone();
          }
        });
      });
    })
  };
  let refused = refusal(permissions, choice)
    .map(|message| field_error("pipeline-hub-step-permissions-error", &message_text(ctx, &message)));
  Some(
    Row::new()
      .id("pipeline-hub-step-permissions-reset-row")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Snug.role(), SpacingSize::Sm))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Snug.role())
          .with_children(refused)
          .child(button),
      ),
  )
}
