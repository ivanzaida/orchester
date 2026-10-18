//! What a tab shows in the strip and in the tab list alike: its label, the mark of a project that needs
//! the person or has an unresolved failure, whether its task form holds edits that are not saved, and
//! its close button.

use lurq::{
  app::{
    ctx::Ctx,
    events::MouseEvent,
    theme::{PaletteColor, RadiusSize},
  },
  components::Button,
  layout::Alignment,
  node::{CursorIcon, Element, Style},
};

use crate::{
  navigation::{
    Navigation,
    tabs::{Tab, TabId},
  },
  project::Projects,
  routes::{ProjectPage, Route},
  shell::needs_you::{self, Attention, count_words},
  theme::{ExtraColor, IconSize},
  ui::{
    icon::{Icon, file_icon, icon, screen_icon},
    pressable::in_title_bar,
    tooltip::Tooltip,
  },
};

/// The mark of a tab whose project needs the person (an amber hand, *Kontur · 2 need you*) or has an
/// unresolved failure (a red alert, *Kontur · runner offline*), with what it means in its tooltip; `id`
/// is the mark's element id.
pub fn attention(ctx: &mut Ctx, projects: &Projects, tab: &Tab, id: String) -> Option<Element> {
  let project = tab.route().project()?;
  let mark = needs_you::attention(projects, project)?;
  let name = projects.name(project)?;
  let (glyph, color, what, state) = match mark {
    Attention::Waiting(count) => (Icon::Hand, PaletteColor::Warning, count_words(ctx, count), "needs-you"),
    Attention::RunnerOffline => (
      Icon::CircleAlert,
      PaletteColor::Danger,
      ctx.t("shell.tabs.runnerOffline").to_string(),
      "runner-offline",
    ),
    Attention::TasksUnread => (
      Icon::CircleAlert,
      PaletteColor::Danger,
      ctx.t("shell.tabs.tasksUnread").to_string(),
      "tasks-unread",
    ),
  };
  let text = ctx.t_args(
    "shell.tabs.attention",
    [("project", name.as_str()), ("waiting", what.as_str())],
  );
  let marker = icon(glyph, IconSize::Xs, color).id(id.clone()).describe("state", state);
  Some(Tooltip::new(id, text).wrap(ctx, marker))
}

/// Whether `tab` ends in a mark ([`attention`]).
pub fn has_attention(projects: &Projects, tab: &Tab) -> bool {
  tab
    .route()
    .project()
    .is_some_and(|project| needs_you::attention(projects, project).is_some() && projects.name(project).is_some())
}

/// Whether the task form `route` shows holds edits that are not saved.
pub fn unsaved(projects: &Projects, route: &Route) -> bool {
  let (project, task) = match route {
    Route::Project(project, ProjectPage::NewTask) => (*project, None),
    Route::Project(project, ProjectPage::EditTask(key)) => (*project, Some(key.to_string())),
    _ => return false,
  };
  projects
    .feed_now(project)
    .is_some_and(|feed| feed.unsaved.get().contains(task.as_deref()))
}

/// The tab's label: its page ([`page`]) after the project's name once it is known
/// (`Kontur · KON-12`).
pub fn label(ctx: &mut Ctx, projects: &Projects, route: &Route) -> String {
  let screen = page(ctx, projects, route);
  match project_name(projects, route) {
    Some(name) => ctx
      .t_args(
        "shell.tabs.inProject",
        [("project", name.as_str()), ("label", screen.as_str())],
      )
      .to_string(),
    None => screen,
  }
}

/// The page a tab shows: its screen's name, a task by its key and its Edit task as `Edit <key>`
/// (`Qp1B0`), a pipeline by its name, a file by its name (`Orchester · plan_limits.rs` in `o1Uea4`), a pull request by
/// its number (`#142`, `e3Wty`).
pub fn page(ctx: &mut Ctx, projects: &Projects, route: &Route) -> String {
  match route {
    Route::Project(_, ProjectPage::Task(key, _) | ProjectPage::KeptTask(key, _)) => key.to_string(),
    Route::Project(_, ProjectPage::NewPipeline) => ctx.t("pipelines.new").to_string(),
    Route::Project(project, ProjectPage::Pipeline(id, _) | ProjectPage::PipelineVersion(id, ..)) => projects
      .feed(*project)
      .pipeline_names
      .get()
      .0
      .get(id)
      .cloned()
      .unwrap_or_else(|| ctx.t(route.screen().title_key()).to_string()),
    Route::Project(_, ProjectPage::EditTask(key)) => ctx
      .t_args("shell.screens.editTaskOf", [("key", key.to_string())])
      .to_string(),
    Route::Project(_, ProjectPage::Files(view)) => view.file().map_or_else(
      || ctx.t(route.screen().title_key()).to_string(),
      |file| file.rsplit('/').next().unwrap_or(file).to_owned(),
    ),
    Route::Project(_, ProjectPage::PullRequest(number, _)) => ctx
      .t_args("shell.screens.pullRequestOf", [("number", number.to_string())])
      .to_string(),
    route => ctx.t(route.screen().title_key()).to_string(),
  }
}

/// The icon of a tab at `route`: its screen's, or for a file of the code viewer the file's, as the page's title
/// shows it (`o1Uea4`, `c2jICC`, `ISM2O`).
pub fn tab_icon(route: &Route) -> Icon {
  match route {
    Route::Project(_, ProjectPage::Files(view)) => view.file().map_or(Icon::FolderTree, file_icon),
    route => screen_icon(route.screen()),
  }
}

/// The name of the project `route` is in, once it is known to be in the list.
pub fn project_name(projects: &Projects, route: &Route) -> Option<String> {
  route
    .project()
    .and_then(|project| projects.name(project))
    .map(|name| name.as_str().to_owned())
}

/// The close button of the tab `id` (`Close` `hfcCG`), with the element id `element`.
pub fn close_button(ctx: &mut Ctx, navigation: Navigation, id: TabId, element: String) -> Element {
  in_title_bar(Button::empty())
    .id(element)
    .describe("a11y_name", ctx.t("shell.tabs.close"))
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .align_items(Alignment::Center)
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      navigation.close(id);
    })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .rounded(RadiusSize::Sm)
    .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}
