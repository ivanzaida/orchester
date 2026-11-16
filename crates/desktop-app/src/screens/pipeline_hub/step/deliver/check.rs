//! *Check* of the Deliver step (`Check` `mFEyb` in P15 `nPaMj`): *Re-run Check on the merged result*, a switch with
//! what it does beside it. Auto-merge keeps it on: the switch is faded with a lock beside it and says why. With no
//! command tool step there is nothing to run again: the row is muted, its switch off and faded, and it says so.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{DeliverStep, StepKind, ToolKind};
use orchester_workspace::merge_queue::rechecks;

use super::{Open, change};
use crate::{
  screens::team_hub::rows,
  theme::{HubText, IconSize, LOCKED_SWITCH_OPACITY},
  ui::{
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};

const ROW: &str = "pipeline-hub-deliver-recheck-row";
const SWITCH: &str = "pipeline-hub-deliver-recheck";

pub(super) fn group(ctx: &mut Ctx, open: &Open<'_>, deliver: &DeliverStep) -> Element {
  let label = ctx.t("pipeline_hub.deliver.recheck");
  let tools = check_tools(open);
  let nothing_to_run = tools.is_none() && !deliver.recheck_locked();
  // Auto-merge's own words come first: it never merges a result that was not checked.
  let words = match (&tools, deliver.recheck_locked(), deliver.recheck()) {
    (_, true, _) => ctx.t("pipeline_hub.deliver.recheckLocked"),
    (None, false, _) => ctx.t("pipeline_hub.deliver.recheckNoTools"),
    (Some(_), false, false) => ctx.t("pipeline_hub.deliver.recheckOff"),
    (Some(tools), false, true) => ctx.t_args("pipeline_hub.deliver.recheckHint", [("tools", tools.as_str())]),
  };
  let toggle = Toggle::new(
    SWITCH,
    ToggleKind::Switch,
    deliver.recheck() && !nothing_to_run,
    label.clone(),
  )
  .hide_label();
  let switch: Element = if deliver.recheck_locked() {
    Row::new()
      .id(format!("{SWITCH}-locked"))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .opacity(LOCKED_SWITCH_OPACITY)
      .describe("locked", "true")
      .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
      .child(toggle.settled())
      .into()
  } else if nothing_to_run || open.locked {
    // Without a handler the switch is faded and cannot be flipped.
    toggle.into()
  } else {
    let edit = open.edit.clone();
    toggle
      .on_change(move |on| change(&edit, |deliver| deliver.with_recheck(on)))
      .into()
  };
  let value = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("unavailable", nothing_to_run.to_string())
    .child(
      Text::new(&words)
        .id(format!("{SWITCH}-hint"))
        .variant(HubText::Status.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(switch);
  let key = Text::new(&label)
    .id(format!("{ROW}-label"))
    .variant(HubText::Status.role())
    .color(if nothing_to_run {
      PaletteColor::TextMuted
    } else {
      PaletteColor::TextSecondary
    });
  rows::section(
    "pipeline-hub-deliver-check",
    &ctx.t("pipeline_hub.deliver.check"),
    Vec::new(),
    vec![rows::keyed(ROW, key, None, value)],
  )
  .into()
}

/// The names of the command tools the pipeline's checking tool steps run, joined: what Check runs again
/// (`orchester_workspace::merge_queue::rechecks`; it runs command tools only).
fn check_tools(open: &Open<'_>) -> Option<String> {
  let names: Vec<String> = open
    .current
    .steps()
    .iter()
    .filter(|step| rechecks(&step.key))
    .filter_map(|step| match step.kind {
      StepKind::Tool { tool: Some(tool) } => open
        .assignables
        .tool(tool)
        .filter(|choice| choice.kind == ToolKind::Command)
        .map(|choice| choice.name.to_string()),
      _ => None,
    })
    .collect();
  (!names.is_empty()).then(|| names.join(", "))
}
