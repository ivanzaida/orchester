//! Who runs the selected step (`x9u2n`): a project agent profile or a published tool, required while
//! none is chosen (`rToi2`). Tools a step cannot run in this version (HTTP and database tools, tools
//! with access rules) are listed but cannot be chosen, with the reason. A tool step shows what its
//! command runs and opens its tool; without a published command tool it leads to Tools.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, SelectOption, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{StepKind, ToolKind};

use super::{
  step_fields::{SMALL_BUTTON_HEIGHT, label, labelled, select},
  step_inspector::{Fields, StepInspectorProps},
};
use crate::{
  navigation::route::{Page, ToolSelection},
  pipelines::{data::ToolChoice, fields::Executor},
  screens::project::ProjectNav,
  theme::ExtraTypography,
  ui::{button::ActionButton, icon::Icon},
};

pub fn executor(ctx: &Ctx, props: &StepInspectorProps, fields: &Fields) -> Element {
  let tool_step = matches!(props.step.kind, StepKind::Tool { .. });
  let unset_label = if tool_step {
    ctx.t("pipelines.inspector.chooseTool")
  } else {
    ctx.t("pipelines.inspector.chooseProfile")
  };
  let mut values = vec![Executor::Unset];
  let mut options = vec![SelectOption::new(Executor::Unset, unset_label)];
  for profile in &props.assignables.profiles {
    let version = profile.version.to_string();
    let label = ctx.t_args(
      "pipelines.inspector.profileOption",
      [("profile", &*profile.name), ("version", version.as_str())],
    );
    values.push(Executor::Profile(profile.id));
    options.push(SelectOption::new(Executor::Profile(profile.id), label));
  }
  for tool in &props.assignables.tools {
    values.push(Executor::Tool(tool.id));
    options.push(tool_option(ctx, tool));
  }
  let current = Executor::of(&props.step);
  if !values.contains(&current) {
    options.push(SelectOption::new(
      current,
      ctx.t("pipelines.inspector.unpublishedExecutor"),
    ));
  }
  let unset = current == Executor::Unset;
  let key = if tool_step {
    "pipelines.inspector.tool"
  } else {
    "pipelines.inspector.profile"
  };
  let mut field = labelled(
    label(ctx, key),
    select(ctx, "step-executor", &fields.executor, options, unset),
  );
  if unset {
    let required = ctx.t_args(
      if tool_step {
        "pipelines.inspector.toolRequired"
      } else {
        "pipelines.inspector.profileRequired"
      },
      [("step", props.step.name.as_str())],
    );
    field = field.child(
      Text::new(&required)
        .variant(ExtraTypography::Hint.role())
        .color(PaletteColor::TextPrimary),
    );
  }
  field.into()
}

/// "Tool · Run test suite"; one a step cannot run says why and cannot be chosen.
fn tool_option(ctx: &Ctx, tool: &ToolChoice) -> SelectOption<Executor> {
  let option = SelectOption::new(
    Executor::Tool(tool.id),
    ctx.t_args("pipelines.inspector.toolOption", [("tool", &*tool.name)]),
  );
  if tool.runnable {
    return option;
  }
  let reason = match tool.kind {
    ToolKind::Http | ToolKind::Database => ctx.t("pipelines.inspector.toolNeedsSecrets"),
    ToolKind::Command => ctx.t("pipelines.inspector.toolAccessRules"),
  };
  option.disabled(true).detail(reason)
}

/// What a tool step's command runs, with the way to its tool (`LRvkv`, drawn for a tool); a step
/// without a runnable tool leads to Tools.
pub fn tool(ctx: &Ctx, props: &StepInspectorProps, nav: &ProjectNav) -> Option<Element> {
  let StepKind::Tool { tool } = &props.step.kind else {
    return None;
  };
  let chosen = tool
    .and_then(|id| props.assignables.tool(id))
    .filter(|tool| tool.runnable);
  let nav = nav.clone();
  let (line, hint, button, target): (Arc<str>, Arc<str>, Arc<str>, Page) = match chosen {
    Some(tool) => (
      ctx.t_args(
        "pipelines.inspector.toolCommand",
        [("command", tool.command.as_deref().unwrap_or_default())],
      ),
      ctx.t("pipelines.inspector.toolCommandHint"),
      ctx.t("pipelines.inspector.openTool"),
      Page::Tools(ToolSelection::Tool(tool.id)),
    ),
    None if props.assignables.tools.iter().any(|tool| tool.runnable) => return None,
    None => (
      ctx.t("pipelines.inspector.noCommandTool"),
      ctx.t("pipelines.inspector.noCommandToolHint"),
      ctx.t("pipelines.inspector.openTools"),
      Page::Tools(ToolSelection::Library),
    ),
  };
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      // The full-width button's focus ring reaches past the column.
      .overflow_visible()
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(&line)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextPrimary)
          .id("step-tool-command"),
      )
      .child(Text::new(&hint).variant(ExtraTypography::Hint.role()))
      .child(
        ActionButton::new(button)
          .icon(Icon::ExternalLink)
          .id("step-open-tool")
          .in_form()
          .fill_width()
          .height(Dimension::Px(SMALL_BUTTON_HEIGHT))
          .on_click(move || nav.show(target))
          .render(ctx),
      )
      .into(),
  )
}
