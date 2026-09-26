//! Shell sidebar (`zo9Py`). Overview, Tasks, Pull requests, Pipelines, Agents, Connections and Project settings
//! open their screens; every other entry is shown but unavailable. The runner card (`O2Jjjf`) is not
//! shown: the status bar reports the runner on every screen.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{ProjectNav, Section, TasksState};
use crate::{
  theme::{ExtraColor, ExtraSpacing},
  ui::{brand::brand, icon::Icon, nav_item::NavItem},
  workspace::OpenProject,
};

/// Sidebar width, current project block, project selector and avatar sizes (`zo9Py`, `RaOff`,
/// `ZcJDx`, `I01Ke`).
const SIDEBAR_WIDTH: f32 = 216.0;
const CURRENT_PROJECT_HEIGHT: f32 = 70.0;
const SELECTOR_HEIGHT: f32 = 42.0;
const AVATAR_SIZE: f32 = 24.0;

pub fn sidebar(ctx: &Ctx, open: &OpenProject, section: Section, tasks: &TasksState, nav: &ProjectNav) -> Element {
  Column::new()
    .width(Dimension::Px(SIDEBAR_WIDTH))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Column::new().padding_horizontal(SpacingSize::Lg).child(brand(ctx)))
    .child(project_selector(open))
    .child(primary_navigation(ctx, section, tasks, nav))
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
    .child(footer(ctx, section, nav))
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
    .height(Dimension::Px(CURRENT_PROJECT_HEIGHT))
    .padding_top(SpacingSize::Sm)
    .padding_horizontal(SpacingSize::Md)
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

/// Tasks stays selected while a new task is written (`l2YA4`).
fn primary_navigation(ctx: &Ctx, section: Section, tasks: &TasksState, nav: &ProjectNav) -> Element {
  let open = |target: Section| {
    let nav = nav.clone();
    move || nav.show(target)
  };
  let items = [
    NavItem::new("nav-overview", Icon::LayoutDashboard, ctx.t("nav.overview"))
      .selected(section == Section::Overview)
      .on_click(open(Section::Overview)),
    NavItem::new("nav-orchestrator", Icon::MessagesSquare, ctx.t("nav.orchestrator")),
    NavItem::new("nav-tasks", Icon::CircleCheck, ctx.t("nav.tasks"))
      .selected(matches!(section, Section::Tasks | Section::NewTask | Section::Task(_)))
      .on_click(open(Section::Tasks))
      .count(tasks.ready().map(|tasks| tasks.tasks.len())),
    NavItem::new("nav-pull-requests", Icon::GitPullRequest, ctx.t("nav.pullRequests"))
      .selected(matches!(section, Section::PullRequests | Section::PullRequest(_)))
      .on_click(open(Section::PullRequests)),
    NavItem::new("nav-pipelines", Icon::Workflow, ctx.t("nav.pipelines"))
      .selected(section == Section::Pipelines)
      .on_click(open(Section::Pipelines)),
    NavItem::new("nav-instructions", Icon::BookOpen, ctx.t("nav.instructions")),
    NavItem::new("nav-agents", Icon::Bot, ctx.t("nav.agents"))
      .selected(section == Section::Agents)
      .on_click(open(Section::Agents)),
    NavItem::new("nav-tools", Icon::Wrench, ctx.t("nav.tools")),
    NavItem::new("nav-secrets", Icon::KeyRound, ctx.t("nav.secrets")),
    NavItem::new("nav-connections", Icon::Plug, ctx.t("nav.connections"))
      .selected(section == Section::Connections)
      .on_click(open(Section::Connections)),
  ];
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Xs)
    .with_children(items)
    .into()
}

fn footer(ctx: &Ctx, section: Section, nav: &ProjectNav) -> Element {
  let nav = nav.clone();
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Stack.role())
    .padding_horizontal(SpacingSize::Md)
    .padding_bottom(ExtraSpacing::Step.role())
    .child(
      NavItem::new("nav-settings", Icon::Settings, ctx.t("nav.projectSettings"))
        .compact()
        .selected(section == Section::Settings)
        .on_click(move || nav.show(Section::Settings)),
    )
    .into()
}
