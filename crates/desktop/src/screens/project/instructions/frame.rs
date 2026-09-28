//! The Instructions page around the library and the editor (`CWoy4` in `yRKeF`): the topbar
//! (`wuKkf`) with the save state and Publish, the page heading (`fRdKl`) and the scope tabs
//! (`GfEHL`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::super::ProjectNav;
use crate::{
  navigation::route::{InstructionsTab, Page},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::tab::{Tab, tab_row},
  workspace::OpenProject,
};

/// Topbar and scope tab row heights (`wuKkf`, `GfEHL`).
const TOPBAR_HEIGHT: f32 = 64.0;
const TABS_HEIGHT: f32 = 42.0;

pub struct Frame<'a> {
  pub project: &'a OpenProject,
  /// The tab shown as selected.
  pub tab: InstructionsTab,
  pub nav: &'a ProjectNav,
  /// Right of the topbar: the save state and Publish.
  pub trailing: Vec<Element>,
  /// `None` while the rules are read.
  pub library: Option<Element>,
  /// Right of the library: the editor and the preview, or an empty or failed state.
  pub body: Element,
}

pub fn frame(ctx: &Ctx, frame: Frame<'_>) -> Element {
  let project = frame.project.project.name().as_str();
  let topbar = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t_args("instructions.breadcrumb", [("project", project)]))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(frame.trailing);
  let heading = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Lg)
    .padding_horizontal(SpacingSize::Lg)
    .padding_bottom(ExtraSpacing::Divider.role())
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t("instructions.title")).variant(ExtraTypography::PageTitle.role()))
    .child(
      Text::new(&ctx.t_args("instructions.description", [("project", project)])).variant(TypographyStyle::Description),
    );
  Column::new()
    .id("instructions-screen")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(ExtraColor::Surface.role())
    .child(topbar)
    .child(heading)
    .child(scope_tabs(ctx, frame.tab, frame.nav))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .with_children(frame.library)
        .child(frame.body),
    )
    .into()
}

/// Project rules, Agent roles, Pipeline overrides, Step overrides (`GfEHL`).
fn scope_tabs(ctx: &Ctx, shown: InstructionsTab, nav: &ProjectNav) -> Element {
  let tab = |id: &'static str, key: &str, target: InstructionsTab| {
    Tab::new(id, ctx.t(key), Dimension::Pct(100.0))
      .selected(shown == target)
      .link(nav.link(Page::Instructions(target, None)))
  };
  tab_row(
    Dimension::Px(TABS_HEIGHT),
    SpacingSize::Md,
    Some(SpacingSize::Lg),
    [
      tab(
        "instructions-tab-project",
        "instructions.tabs.projectRules",
        InstructionsTab::ProjectRules,
      ),
      tab(
        "instructions-tab-roles",
        "instructions.tabs.agentRoles",
        InstructionsTab::AgentRoles,
      ),
      tab(
        "instructions-tab-pipelines",
        "instructions.tabs.pipelineOverrides",
        InstructionsTab::PipelineOverrides,
      ),
      tab(
        "instructions-tab-steps",
        "instructions.tabs.stepOverrides",
        InstructionsTab::StepOverrides,
      ),
    ],
  )
}
