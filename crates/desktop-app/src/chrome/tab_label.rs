//! What a tab shows: its label and icon, the mark of a project that needs the person or has an unresolved
//! failure, and whether its task form holds edits that are not saved. A project that waits for the person marks each of
//! its tabs, the active one too, with a ✋ and how many wait, the count of its Needs you, as the Needs you panel counts
//! a project (`Count` `ruG7r` in `o3xekF`; the design's tab mark `Tab/attention` `iJNwr` draws the hand without its
//! count).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};

use crate::{
  navigation::tabs::Tab,
  project::Projects,
  routes::{ProjectPage, Route, Screen},
  shell::needs_you::{self, Attention, count_words},
  theme::{IconSize, NeedsYouText},
  ui::{
    icon::{Icon, file_icon, icon, screen_icon},
    tooltip::Tooltip,
  },
};

/// The mark of a tab whose project needs the person (an amber hand and how many wait, *Kontur · 2 need you*) or
/// has an unresolved failure (a red alert, *Kontur · runner offline*), with what it means in its tooltip; `id`
/// is the mark's element id.
pub fn attention(ctx: &mut Ctx, projects: &Projects, tab: &Tab, id: String) -> Option<Element> {
  let project = tab.route().project()?;
  let mark = needs_you::attention(projects, project)?;
  let name = projects.name(project)?;
  let (glyph, color, what, state) = match mark {
    Attention::Waiting(count) => (Icon::Hand, PaletteColor::Warning, count_words(ctx, count), "needs-you"),
    Attention::RunnerOffline => (
      Icon::CircleAlert,
      PaletteColor::Destructive,
      ctx.t("shell.tabs.runnerOffline").to_string(),
      "runner-offline",
    ),
    Attention::TasksUnread => (
      Icon::CircleAlert,
      PaletteColor::Destructive,
      ctx.t("shell.tabs.tasksUnread").to_string(),
      "tasks-unread",
    ),
  };
  let text = ctx.t_args(
    "shell.tabs.attention",
    [("project", name.as_str()), ("waiting", what.as_str())],
  );
  let marker: Element = match mark {
    Attention::Waiting(count) => Row::new()
      .id(id.clone())
      .describe("state", state)
      .describe("count", count.to_string())
      .flex_shrink(0.0)
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Sm, color))
      .child(
        Text::new(&count.to_string())
          .id(format!("{id}-count"))
          .variant(NeedsYouText::Tally.role())
          .nowrap(),
      )
      .into(),
    Attention::RunnerOffline | Attention::TasksUnread => icon(glyph, IconSize::Xs, color)
      .id(id.clone())
      .describe("state", state)
      .into(),
  };
  Some(Tooltip::new(id, text).wrap(ctx, marker))
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
/// (`Qp1B0`), a pipeline by its name, a file by its name (`Orchester · plan_limits.rs` in `o1Uea4`); Memory, a
/// filter of the Agents hub, as the hub (`Kontur · Agents` in Y1 `tsfxN`).
fn page(ctx: &mut Ctx, projects: &Projects, route: &Route) -> String {
  match route {
    Route::Project(_, ProjectPage::Task(key, _) | ProjectPage::KeptTask(key, _)) => key.to_string(),
    Route::Project(_, ProjectPage::NewPipeline | ProjectPage::CopyPipeline(_)) => ctx.t("pipeline_hub.new").to_string(),
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
    Route::Project(_, ProjectPage::Memory(_)) => ctx.t(Screen::Agents.title_key()).to_string(),
    route => ctx.t(route.screen().title_key()).to_string(),
  }
}

/// The icon of a tab at `route`: its screen's, or for a file of the code viewer the file's, as the page's title
/// shows it (`o1Uea4`, `c2jICC`, `ISM2O`); All projects' the layout grid (`Active tab` `jMVvA` in 09 `i5G19A`).
pub fn tab_icon(route: &Route) -> Icon {
  match route {
    Route::Project(_, ProjectPage::Files(view)) => view.file().map_or(Icon::FolderTree, file_icon),
    Route::AllProjects | Route::Usage(_) => Icon::LayoutGrid,
    Route::Project(_, ProjectPage::Memory(_)) => screen_icon(Screen::Agents),
    route => screen_icon(route.screen()),
  }
}

/// The name of the project `route` is in, once it is known to be in the list.
fn project_name(projects: &Projects, route: &Route) -> Option<String> {
  route
    .project()
    .and_then(|project| projects.name(project))
    .map(|name| name.as_str().to_owned())
}
