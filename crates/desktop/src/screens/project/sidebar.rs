//! Shell sidebar (`zo9Py`). Overview is the only project screen so far; every other entry is
//! shown but unavailable, and the runner card says that no runner exists yet.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Rect, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{brand::brand, icon::Icon, nav_item::NavItem},
  workspace::OpenProject,
};

/// Sidebar width, project selector and avatar sizes, runner indicator size (`zo9Py`, `ZcJDx`,
/// `I01Ke`, `fK4mR`).
const SIDEBAR_WIDTH: f32 = 216.0;
const SELECTOR_HEIGHT: f32 = 42.0;
const AVATAR_SIZE: f32 = 24.0;
const INDICATOR_SIZE: f32 = 5.0;

pub fn sidebar(ctx: &Ctx, open: &OpenProject) -> Element {
  Column::new()
    .width(Dimension::Px(SIDEBAR_WIDTH))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Column::new().padding_horizontal(SpacingSize::Lg).child(brand(ctx)))
    .child(project_selector(open))
    .child(primary_navigation(ctx))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_top(SpacingSize::Lg)
        .padding_horizontal(ExtraSpacing::Stack.role())
        .spacing(SpacingSize::Xs)
        .child(NavItem::new("nav-all-projects", Icon::Folders, ctx.t("nav.allProjects")).secondary())
        .child(NavItem::new("nav-import", Icon::Download, ctx.t("nav.importResources")).secondary()),
    )
    .child(Spacer::new().flex(1.0))
    .child(footer(ctx))
    .into()
}

/// The current project. Switching needs All projects, which does not exist yet, so the
/// selector has no switch control.
fn project_selector(open: &OpenProject) -> Element {
  let name = open.project.name().as_str();
  let initial: String = name.chars().next().into_iter().flat_map(char::to_uppercase).collect();
  let avatar = Row::new()
    .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
    .background(PaletteColor::AccentMuted)
    .rounded(RadiusSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(Text::new(&initial).variant(TypographyStyle::Button));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Sm)
    .padding_horizontal(SpacingSize::Md)
    .padding_bottom(SpacingSize::Md)
    .child(
      Row::new()
        .id("project-selector")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(SELECTOR_HEIGHT))
        .padding(SpacingSize::Sm)
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
        .rounded(RadiusSize::Md)
        .child(avatar)
        .child(Text::new(name).variant(TypographyStyle::Label).nowrap().flex(1.0)),
    )
    .into()
}

fn primary_navigation(ctx: &Ctx) -> Element {
  let items = [
    NavItem::new("nav-overview", Icon::LayoutDashboard, ctx.t("nav.overview")).selected(true),
    NavItem::new("nav-orchestrator", Icon::MessagesSquare, ctx.t("nav.orchestrator")),
    NavItem::new("nav-tasks", Icon::CircleCheck, ctx.t("nav.tasks")),
    NavItem::new("nav-pull-requests", Icon::GitPullRequest, ctx.t("nav.pullRequests")),
    NavItem::new("nav-pipelines", Icon::Workflow, ctx.t("nav.pipelines")),
    NavItem::new("nav-instructions", Icon::BookOpen, ctx.t("nav.instructions")),
    NavItem::new("nav-agents", Icon::Bot, ctx.t("nav.agents")),
    NavItem::new("nav-tools", Icon::Wrench, ctx.t("nav.tools")),
    NavItem::new("nav-secrets", Icon::KeyRound, ctx.t("nav.secrets")),
    NavItem::new("nav-connections", Icon::Plug, ctx.t("nav.connections")),
  ];
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Xs)
    .with_children(items)
    .into()
}

fn footer(ctx: &Ctx) -> Element {
  let runner = Column::new()
    .id("runner-status")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Sm)
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .rounded(ExtraRadius::Notice.role())
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Rect::new(Dimension::Px(INDICATOR_SIZE), Dimension::Px(INDICATOR_SIZE))
            .background(PaletteColor::TextMuted)
            .rounded(RadiusSize::Lg),
        )
        .child(
          Text::new(&ctx.t("runner.none"))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary),
        ),
    )
    .child(Text::new(&ctx.t("runner.noneDetail")).variant(ExtraTypography::Footnote.role()));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Stack.role())
    .padding_horizontal(SpacingSize::Md)
    .padding_bottom(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Group.role())
    .child(runner)
    .child(NavItem::new("nav-settings", Icon::Settings, ctx.t("nav.projectSettings")).compact())
    .into()
}
