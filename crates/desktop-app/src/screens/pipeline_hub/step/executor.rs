//! What the *Agent* row of an open step says besides its field: under its label *Open Builder*, the profile's own page
//! in the Agents hub, for every profile the step names (`Open Builder` `RAHPd` in P2b `EGEos`), or a tool step's rule
//! (*Its exit code decides the step*, `d96cH0` in P2d `D4MDQO`); and, under the row, why a tool the step runs cannot
//! run here, by its kind (`Tool unavailable notice` `ZGPcW`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ResourceId,
  resource::{Step, ToolKind},
};
use orchester_workspace::pipelines::{data::ToolChoice, fields::Executor};

use super::Open;
use crate::{
  navigation::Navigation,
  routes::Route,
  screens::{pipeline_hub::flow::Fix, team_hub::rows::Sub},
  theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    pressable::{Focus, Press},
  },
};

/// The row's key under its label, and how far its field stands from the row's top so that it stays in the middle of
/// the key: the field's box keeps that place when a note comes under it.
pub(super) struct Key {
  pub label: Element,
  pub sub: Option<Sub>,
  pub lead: Option<ExtraSpacing>,
}

/// The key of the *Agent* row of `step`; `label`, the row's label, is laid out on the field's line when nothing is
/// under it.
pub(super) fn key(ctx: &mut Ctx, open: &Open<'_>, step: &Step, label: Text) -> Key {
  let on_line = |label: Text| -> Element {
    Row::new()
      .height(Dimension::Px(ControlHeight::Md.value()))
      .align_items(Alignment::Center)
      .child(label)
      .into()
  };
  match Executor::of(step) {
    Executor::Profile(id) => match builder(ctx, open, id) {
      Some(link) => Key {
        label: Column::new().child(label).child(link).into(),
        sub: None,
        lead: Some(ExtraSpacing::Snug),
      },
      None => Key {
        label: on_line(label),
        sub: None,
        lead: None,
      },
    },
    Executor::Tool(_) => Key {
      label: label.into(),
      sub: Some(Sub::quiet(ctx.t("pipeline_hub.stepEdit.toolDecides"))),
      lead: Some(ExtraSpacing::Tight),
    },
    Executor::Unset => Key {
      label: on_line(label),
      sub: None,
      lead: None,
    },
  }
}

/// *Open Builder ↗*: 22 high, 6 at its sides and between its 12 arrow and its words, the `hover` fill under the
/// pointer; it opens profile `id` in the Agents hub (the panel asks first when it holds changes). None for a profile
/// the project no longer has: there is no page to open.
fn builder(ctx: &mut Ctx, open: &Open<'_>, id: ResourceId) -> Option<Element> {
  let assignables = open.assignables;
  let name = assignables
    .profile(id)
    .map(|profile| profile.name.clone())
    .or_else(|| {
      assignables
        .draft_profiles
        .iter()
        .find(|profile| profile.id == id)
        .map(|profile| profile.name.clone())
    })?;
  let route = Route::Project(open.project.open.project.id(), Fix::Profile(id).page());
  let words = ctx.t_args("pipeline_hub.stepEdit.openProfile", [("name", name.as_ref())]);
  Some(
    Focus::Keyboard
      .apply(Press::Open(Navigation::of(ctx).to(route)).button())
      .id("pipeline-hub-step-open-profile")
      .describe("a11y_name", words.clone())
      .height(Dimension::Px(ControlHeight::Helper.value()))
      .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextSecondary))
      .child(
        Text::new(&words)
          .id("pipeline-hub-step-open-profile-label")
          .variant(TypographyStyle::Button)
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
      .into(),
  )
}

/// The tool `step` runs when it cannot run as a step here.
pub(super) fn unavailable_tool<'a>(open: &Open<'a>, step: &Step) -> Option<&'a ToolChoice> {
  let Executor::Tool(id) = Executor::of(step) else {
    return None;
  };
  open.assignables.tool(id).filter(|tool| !tool.runnable)
}

/// Why `tool` cannot run as a step here, by its kind: the info notice under the *Agent* row.
pub(super) fn tool_notice(ctx: &Ctx, tool: &ToolChoice) -> Row {
  Row::new().width(Dimension::Pct(100.0)).child(
    Notice::new(
      "pipeline-hub-step-tool-unavailable",
      NoticeTone::Info,
      ctx.t("pipeline_hub.stepEdit.toolUnavailable"),
    )
    .icon(Icon::Lock)
    .body(ctx.t(tool_reason(tool.kind))),
  )
}

/// Why a tool of `kind` cannot run as a step.
const fn tool_reason(kind: ToolKind) -> &'static str {
  match kind {
    ToolKind::Database => "pipeline_hub.stepEdit.databaseUnavailable",
    ToolKind::Http => "pipeline_hub.stepEdit.httpUnavailable",
    ToolKind::Command => "pipeline_hub.stepEdit.toolAccessUnavailable",
  }
}
