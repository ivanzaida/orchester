//! The screens the router shows inside the shell, one module per screen. Every route of the router's
//! table comes here: the path is read as a [`Route`] once, and its screen gets what the route names. A
//! project's screen gets the project open ([`ShownProject`]); until it is, the content panel stays empty,
//! and a project that cannot be opened shows why instead (`unavailable_project`). None is built yet:
//! each shows an honest placeholder (`placeholder`), and a path that names nothing says why
//! (`not_found`).

mod agents;
mod all_projects;
mod connections;
mod instructions;
mod not_found;
mod overview;
mod pipelines;
mod placeholder;
mod project_settings;
mod pull_requests;
mod secrets;
mod settings;
mod skills;
mod task;
mod task_form;
mod tasks;
mod tools;
mod unavailable_project;
mod usage;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use crate::{
  project::{ProjectState, Projects, ShownProject},
  routes::{ProjectPage, Route},
};

/// The screen at the router's path.
pub fn screen(ctx: &mut Ctx) -> Element {
  let path = ctx.route_path();
  match Route::parse(&path) {
    Ok(Route::AllProjects) => all_projects::screen(ctx),
    Ok(Route::Settings) => settings::screen(ctx),
    Ok(Route::Project(project, page)) => project_screen(ctx, project, &page),
    Err(error) => not_found::screen(ctx, &path, &error),
  }
}

/// The screen of `project` that `page` names, once the project is open.
fn project_screen(ctx: &mut Ctx, project: ProjectId, page: &ProjectPage) -> Element {
  let projects = Projects::of(ctx);
  match projects.state(project) {
    Some(ProjectState::Open(open)) => {
      let shown = ShownProject {
        feed: projects.feed(project),
        open,
      };
      page_screen(ctx, &shown, page)
    }
    Some(ProjectState::Unavailable(unavailable)) => unavailable_project::screen(ctx, project, &unavailable),
    // Opening a project takes a moment; the content panel stays empty meanwhile.
    None => Column::new()
      .id("screen-opening")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .into(),
  }
}

/// The screen of the open `project` that `page` names.
fn page_screen(ctx: &mut Ctx, project: &ShownProject, page: &ProjectPage) -> Element {
  match page {
    ProjectPage::Overview(tab) => overview::screen(ctx, project, *tab),
    ProjectPage::Tasks(list) => tasks::screen(ctx, project, list),
    ProjectPage::NewTask => task_form::new_task(ctx, project),
    ProjectPage::EditTask(key) => task_form::edit_task(ctx, project, key),
    ProjectPage::Task(key, view) => task::screen(ctx, project, key, view),
    ProjectPage::PullRequests => pull_requests::screen(ctx, project),
    ProjectPage::Usage => usage::screen(ctx, project),
    ProjectPage::Pipelines => pipelines::screen(ctx, project),
    ProjectPage::NewPipeline => pipelines::new_pipeline(ctx, project),
    ProjectPage::Pipeline(id) => pipelines::editor(ctx, project, *id),
    ProjectPage::Agents => agents::screen(ctx, project),
    ProjectPage::Instructions => instructions::screen(ctx, project),
    ProjectPage::Skills => skills::screen(ctx, project),
    ProjectPage::Tools => tools::screen(ctx, project),
    ProjectPage::Secrets => secrets::screen(ctx, project),
    ProjectPage::Connections(view) => connections::screen(ctx, project, *view),
    ProjectPage::NewConnection(kind) => connections::new_connection(ctx, project, *kind),
    ProjectPage::EditConnection(id) => connections::edit_connection(ctx, project, *id),
    ProjectPage::Settings => project_settings::screen(ctx, project),
  }
}
