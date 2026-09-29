//! What a tab says (`nAKyb`): `<project> · <page>`, where the page is the section's name, or the
//! object's key or name (*Kontur · ORC-12*, *Kontur · Feature delivery*); the full location adds
//! the object's title (*Kontur · ORC-12 · Add retry to webhook sender*). A tab without a project
//! names its page alone. Names and keys are the project's data; the separator is one message.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::id::ProjectId;

use super::feed::ProjectFeeds;
use crate::{
  navigation::{
    Places,
    route::{Page, Route},
  },
  screens::project::TasksState,
};

/// What stands before the label: the project's initial, or the folders icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabMark {
  Project { initial: Arc<str> },
  NoProject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabLabel {
  pub mark: TabMark,
  /// The project, for a tab that shows one.
  pub project: Option<ProjectId>,
  /// The project's name, for a tab that shows one.
  pub project_name: Option<Arc<str>>,
  /// `<project> · <page>`.
  pub text: Arc<str>,
  /// The label with the object's title, for the tooltip and the tab list.
  pub full: Arc<str>,
}

/// The label of a tab at `path`. Reads the project's tasks reactively, for task keys and pipeline
/// names.
pub fn label(ctx: &Ctx, path: &str, places: &Places, feeds: &ProjectFeeds) -> TabLabel {
  let route = Route::parse(path).unwrap_or(Route::AllProjects);
  let Route::Project(project, page) = route else {
    let text = ctx.t(outside_key(route));
    return TabLabel {
      mark: TabMark::NoProject,
      project: None,
      project_name: None,
      full: text.clone(),
      text,
    };
  };
  let name: Arc<str> = places.project(project).map_or_else(
    || ctx.t("tabs.unknownProject"),
    |open| open.project.name().as_str().into(),
  );
  let tasks = feeds.of(project).tasks.get();
  let (page, title) = page_name(ctx, page, &tasks);
  let text = ctx.t_args("tabs.label", [("project", &*name), ("page", &*page)]);
  let full = match title {
    Some(title) => ctx.t_args(
      "tabs.labelWithTitle",
      [("project", &*name), ("page", &*page), ("title", &*title)],
    ),
    None => text.clone(),
  };
  TabLabel {
    mark: TabMark::Project {
      initial: initial(&name),
    },
    project: Some(project),
    project_name: Some(name),
    text,
    full,
  }
}

/// The initial of a project's name, as its avatar shows it.
pub fn initial(name: &str) -> Arc<str> {
  let initial: String = name.chars().next().into_iter().flat_map(char::to_uppercase).collect();
  initial.into()
}

const fn outside_key(route: Route) -> &'static str {
  match route {
    Route::Welcome => "tabs.welcome",
    Route::Setup => "projects.add",
    Route::Start | Route::AllProjects | Route::Project(..) => "nav.allProjects",
  }
}

/// The page's part of the label, and the object's title when the page shows one object.
fn page_name(ctx: &Ctx, page: Page, tasks: &TasksState) -> (Arc<str>, Option<Arc<str>>) {
  let ready = tasks.ready();
  match page {
    Page::Task(task, _) | Page::EditTask(task) => ready
      .and_then(|tasks| tasks.tasks.iter().find(|candidate| candidate.id() == task))
      .map_or_else(
        || (ctx.t("nav.tasks"), None),
        |task| {
          (
            task.key().to_string().into(),
            Some(task.details().title.as_str().into()),
          )
        },
      ),
    Page::Pipeline(pipeline, _) => ready
      .and_then(|tasks| tasks.pipelines.iter().find(|choice| choice.id == pipeline))
      .map_or_else(|| (ctx.t("nav.pipelines"), None), |choice| (choice.name.clone(), None)),
    Page::PullRequest(number) => (ctx.t_args("tabs.pullRequest", [("number", &*number.to_string())]), None),
    Page::NewTask => (ctx.t("overview.newTask"), None),
    other => (ctx.t(section_key(other)), None),
  }
}

const fn section_key(page: Page) -> &'static str {
  match page {
    Page::Overview(_) => "nav.overview",
    Page::Tasks(_) | Page::NewTask | Page::Task(..) | Page::EditTask(_) => "nav.tasks",
    Page::PullRequests | Page::PullRequest(_) => "nav.pullRequests",
    Page::Pipelines | Page::Pipeline(..) => "nav.pipelines",
    Page::Instructions(..) => "nav.instructions",
    Page::Skills(_) => "nav.skills",
    Page::Agents(_) => "nav.agents",
    Page::Tools(_) => "nav.tools",
    Page::Secrets(_) => "nav.secrets",
    Page::Connections(..) => "nav.connections",
    Page::Settings(_) => "nav.projectSettings",
  }
}
