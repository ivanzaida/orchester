//! Shell sidebar (`zo9Py`). The project selector opens the project switcher (see `switcher`).
//! Overview, Tasks, Pull requests, Pipelines, Instructions, Skills, Agents, Tools, Secrets, Connections and
//! Project settings open their screens, All projects opens the projects of the workspace and
//! Import resources opens over the screen shown; every other entry is shown but unavailable. The runner card
//! (`O2Jjjf`) above Project settings shows the project's runner.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Spacer},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{ProjectNav, TasksState, runner_card::runner_card, switcher::ProjectSwitcher};
use crate::{
  navigation::{
    Navigation,
    import::ImportTab,
    route::{AgentSelection, InstructionsTab, Page, Route, SettingsTab, ToolSelection},
  },
  runner::RunnerStatus,
  theme::{ExtraColor, ExtraSpacing},
  ui::{brand::brand, icon::Icon, nav_item::NavItem},
  workspace::OpenProject,
};

/// What the sidebar shows besides the navigation: the page shown (`None` while the path names no
/// page), the project's tasks and its runner.
pub struct SidebarState<'a> {
  pub shown: Option<Page>,
  pub tasks: &'a TasksState,
  pub runner: &'a RunnerStatus,
}

pub fn sidebar(ctx: &mut Ctx, open: &OpenProject, state: &SidebarState<'_>, nav: &ProjectNav) -> Element {
  let navigation = ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens");
  let switcher = ctx.mount_keyed::<ProjectSwitcher>("project-switcher", open.clone());
  Column::new()
    .width(Dimension::Px(crate::theme::PROJECT_SIDEBAR_WIDTH))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Column::new().padding_horizontal(SpacingSize::Lg).child(brand(ctx)))
    .child(switcher)
    .child(primary_navigation(ctx, state.shown, state.tasks, nav))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_top(SpacingSize::Lg)
        .padding_horizontal(ExtraSpacing::Stack.role())
        .spacing(SpacingSize::Xs)
        .child(
          NavItem::new("nav-all-projects", Icon::Folders, ctx.t("nav.allProjects"))
            .secondary()
            .link(navigation.link(Route::AllProjects)),
        )
        .child(import_item(ctx, nav)),
    )
    .child(Spacer::new().flex(1.0))
    .child(footer(ctx, open, state, nav))
    .into()
}

/// Import resources (`cyp7s`): opens the import over the page shown.
fn import_item(ctx: &Ctx, nav: &ProjectNav) -> NavItem {
  let nav = nav.clone();
  NavItem::new("nav-import", Icon::Download, ctx.t("nav.importResources"))
    .secondary()
    .on_click(move || nav.show_import(ImportTab::All))
}

/// Tasks stays selected while a new task is written (`l2YA4`). An item opens its section at its
/// start (the task list, the library, the first profile, the first secret, all connections), also from a screen
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
      NavItem::new("nav-skills", Icon::Sparkles, ctx.t("nav.skills")),
      Page::Skills(None),
    ),
    item(
      NavItem::new("nav-agents", Icon::Bot, ctx.t("nav.agents")),
      Page::Agents(AgentSelection::First),
    ),
    item(
      NavItem::new("nav-tools", Icon::Wrench, ctx.t("nav.tools")),
      Page::Tools(ToolSelection::Library),
    ),
    item(
      NavItem::new("nav-secrets", Icon::KeyRound, ctx.t("nav.secrets")),
      Page::SECRETS,
    ),
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

/// The runner card and Project settings (`PAgSR`).
fn footer(ctx: &Ctx, open: &OpenProject, state: &SidebarState<'_>, nav: &ProjectNav) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Stack.role())
    .padding_horizontal(SpacingSize::Md)
    .padding_bottom(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Group.role())
    .child(runner_card(ctx, open, state.runner))
    .child(section_item(
      NavItem::new("nav-settings", Icon::Settings, ctx.t("nav.projectSettings")).compact(),
      state.shown,
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
  item.link(nav.link(target))
}
