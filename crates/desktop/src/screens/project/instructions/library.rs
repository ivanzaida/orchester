//! The rules of one scope (`aYBe1` in `yRKeF`): New instruction, the scope's label, one rule item
//! per instruction (`jNTVN`) with its version and whom it applies to, and where rules are kept.
//! Rule items are pointer-only, as the list is outside any form.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, ScrollVertical, Text, TextOverflow},
  core::Signal,
  layout::layout_kind::ScrollState,
  node::{Element, Style, border::Border, dimension::Dimension},
};

use super::{super::ProjectNav, copy};
use crate::{
  instructions::data::{InstructionEntry, ProjectInstructions},
  navigation::route::{InstructionsTab, Page},
  theme::{ExtraColor, ExtraTypography},
  ui::{button::ActionButton, icon::Icon, pressable::surface},
};

/// Library width, New instruction height and rule item height (`aYBe1`, `La1cB`, `jNTVN`).
const LIBRARY_WIDTH: f32 = 236.0;
const NEW_HEIGHT: f32 = 34.0;
const ITEM_HEIGHT: f32 = 66.0;

/// What a rule of `tab` needs that the project does not have yet, as a copy key.
pub fn missing(tab: InstructionsTab, data: &ProjectInstructions) -> Option<&'static str> {
  match tab {
    InstructionsTab::ProjectRules => None,
    InstructionsTab::AgentRoles => data.profiles.is_empty().then_some("instructions.empty.needsProfile"),
    InstructionsTab::PipelineOverrides => data.pipelines.is_empty().then_some("instructions.empty.needsPipeline"),
    InstructionsTab::StepOverrides => data
      .pipelines
      .iter()
      .all(|pipeline| pipeline.steps.is_empty())
      .then_some("instructions.empty.needsStep"),
  }
}

/// What the library lists and how it navigates.
pub struct Library<'a> {
  pub project: &'a str,
  pub data: &'a ProjectInstructions,
  pub tab: InstructionsTab,
  /// The rule in the editor, as edited so far; it replaces the stored one in the list.
  pub edited: Option<&'a InstructionEntry>,
  pub nav: &'a ProjectNav,
  /// Whether the New instruction dialog is open.
  pub creating: &'a Signal<bool>,
  /// Where the list is scrolled.
  pub scroll: &'a ScrollState,
}

pub fn library(ctx: &Ctx, library: &Library<'_>) -> Element {
  let Library {
    project,
    data,
    tab,
    edited,
    nav,
    creating,
    scroll,
  } = *library;
  let new = ActionButton::new(ctx.t("instructions.new"))
    .icon(Icon::Plus)
    .compact()
    .id("new-instruction")
    .fill_width()
    .height(Dimension::Px(NEW_HEIGHT));
  let new = if missing(tab, data).is_some() {
    new.on_click(|| {}).enabled(false)
  } else {
    let creating = creating.clone();
    new.on_click(move || creating.set(true))
  };
  let items: Vec<Element> = data
    .entries
    .iter()
    .filter(|entry| copy::tab_of(entry.body.scope()) == tab)
    .map(|entry| {
      let shown = edited.filter(|edited| edited.id == entry.id);
      item(ctx, data, shown.unwrap_or(entry), tab, shown.is_some(), nav)
    })
    .collect();
  let label = match tab {
    InstructionsTab::ProjectRules => "instructions.library.projectRules",
    InstructionsTab::AgentRoles => "instructions.library.agentRoles",
    InstructionsTab::PipelineOverrides => "instructions.library.pipelineOverrides",
    InstructionsTab::StepOverrides => "instructions.library.stepOverrides",
  };
  Column::new()
    .id("instruction-library")
    .width(Dimension::Px(LIBRARY_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfacePanel)
    .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(new.render(ctx))
    .child(Text::new(&ctx.t(label)).variant(ExtraTypography::Overline.role()))
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .with_children(items),
      )
      .with_scroll_state(scroll.clone())
      .width(Dimension::Pct(100.0))
      .flex(1.0),
    )
    .child(
      Text::new(&ctx.t_args("instructions.library.storage", [("project", project)]))
        .variant(ExtraTypography::Detail.role()),
    )
    .into()
}

/// A rule (`jNTVN`): its name over its version and whom it applies to.
fn item(
  ctx: &Ctx,
  data: &ProjectInstructions,
  entry: &InstructionEntry,
  tab: InstructionsTab,
  selected: bool,
  nav: &ProjectNav,
) -> Element {
  let metadata = ctx.t_args(
    "instructions.library.metadata",
    [
      ("version", &*copy::version_label(ctx, entry.state)),
      ("appliesTo", copy::applies_to(ctx, &entry.body, data).as_str()),
    ],
  );
  let text = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&entry.name)
        .variant(ExtraTypography::StatusLabel.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(&metadata)
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let frame: Button = surface()
    .id(Arc::<str>::from(format!("instruction-{}", entry.id)))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(crate::theme::ExtraSpacing::Stack.role())
    .rounded(RadiusSize::Md)
    .background(if selected {
      PaletteColor::AccentMuted
    } else {
      PaletteColor::SurfacePanel
    })
    .child(text);
  if selected {
    return frame.describe("state", "selected").into();
  }
  nav
    .link(Page::Instructions(tab, Some(entry.id)))
    .on(frame)
    .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
    .into()
}
