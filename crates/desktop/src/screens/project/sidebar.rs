//! Shell sidebar (`zo9Py`). Overview, Tasks, Pull requests, Pipelines, Instructions, Agents, Connections and Project
//! settings open their screens; every other entry is shown but unavailable. The runner card (`O2Jjjf`) is not
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

use super::{ProjectNav, TasksState};
use crate::{
  navigation::route::{AgentSelection, InstructionsTab, Page, SettingsTab},
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

/// `shown` is the page shown; `None` while the path names no page.
pub fn sidebar(ctx: &Ctx, open: &OpenProject, shown: Option<Page>, tasks: &TasksState, nav: &ProjectNav) -> Element {
  Column::new()
    .width(Dimension::Px(SIDEBAR_WIDTH))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Column::new().padding_horizontal(SpacingSize::Lg).child(brand(ctx)))
    .child(project_selector(open))
    .child(primary_navigation(ctx, shown, tasks, nav))
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
    .child(footer(ctx, shown, nav))
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

/// Tasks stays selected while a new task is written (`l2YA4`). An item opens its section at its
/// start (the task list, the library, the first profile, all connections), also from a screen
/// inside the section; it does nothing on the start itself.
fn primary_navigation(ctx: &Ctx, shown: Option<Page>, tasks: &TasksState, nav: &ProjectNav) -> Element {
  let item = |item: NavItem, target: Page| section_item(item, shown, target, nav);
  let items = [
    item(
      NavItem::new("nav-overview", Icon::LayoutDashboard, ctx.t("nav.overview")),
      Page::OVERVIEW,
    ),
    NavItem::new("nav-orchestrator", Icon::MessagesSquare, ctx.t("nav.orchestrator")),
    item(
      NavItem::new("nav-tasks", Icon::CircleCheck, ctx.t("nav.tasks")),
      Page::TASKS,
    )
    .count(tasks.ready().map(|tasks| tasks.tasks.len())),
    item(
      NavItem::new("nav-pull-requests", Icon::GitPullRequest, ctx.t("nav.pullRequests")),
      Page::PullRequests,
    ),
    item(
      NavItem::new("nav-pipelines", Icon::Workflow, ctx.t("nav.pipelines")),
      Page::Pipelines,
    ),
    item(
      NavItem::new("nav-instructions", Icon::BookOpen, ctx.t("nav.instructions")),
      Page::Instructions(InstructionsTab::ProjectRules, None),
    ),
    item(
      NavItem::new("nav-agents", Icon::Bot, ctx.t("nav.agents")),
      Page::Agents(AgentSelection::First),
    ),
    NavItem::new("nav-tools", Icon::Wrench, ctx.t("nav.tools")),
    NavItem::new("nav-secrets", Icon::KeyRound, ctx.t("nav.secrets")),
    item(
      NavItem::new("nav-connections", Icon::Plug, ctx.t("nav.connections")),
      Page::CONNECTIONS,
    ),
  ];
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Xs)
    .with_children(items)
    .into()
}

fn footer(ctx: &Ctx, shown: Option<Page>, nav: &ProjectNav) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Stack.role())
    .padding_horizontal(SpacingSize::Md)
    .padding_bottom(ExtraSpacing::Step.role())
    .child(section_item(
      NavItem::new("nav-settings", Icon::Settings, ctx.t("nav.projectSettings")).compact(),
      shown,
      Page::Settings(SettingsTab::General),
      nav,
    ))
    .into()
}

/// `item` of the section `target` starts: selected while a page of the section is shown, and
/// opening `target` unless it is shown.
fn section_item(item: NavItem, shown: Option<Page>, target: Page, nav: &ProjectNav) -> NavItem {
  let item = item.selected(shown.map(|page| page.sidebar_item()) == Some(target.sidebar_item()));
  if shown == Some(target) {
    return item;
  }
  let nav = nav.clone();
  item.on_click(move || nav.show(target))
}
