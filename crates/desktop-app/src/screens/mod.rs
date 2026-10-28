//! The screens the router shows inside the shell, one module per screen. Every route of the router's
//! table comes here: the path is read as a [`Route`] once, and its screen gets what the route names. A
//! project's screen gets the project open ([`ShownProject`]); until it is, the content panel stays empty,
//! and a project that cannot be opened shows why instead (`unavailable_project`). A path that names nothing
//! says why (`not_found`).

mod add_project;
mod agents;
mod all_projects;
mod connections;
mod delivery;
mod files;
pub(crate) mod gate;
pub(crate) mod hub;
mod import;
mod instructions;
mod kept;
pub(crate) mod memory;
pub(crate) mod merge_queue;
pub(crate) mod modules;
mod not_found;
mod orchestrator;
mod overview;
mod pipelines;
mod placeholder;
mod project_settings;
mod pull_requests;
mod secrets;
mod settings;
mod skills;
pub(crate) mod task;
mod task_close;
mod task_form;
mod tasks;
mod tools;
mod unavailable_project;
mod usage;

use std::sync::Arc;

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
    Ok(Route::Usage(query)) => all_projects::usage_screen(ctx, *query),
    Ok(Route::AddProject(folder)) => add_project::screen(ctx, folder.map(|folder| folder.path())),
    Ok(Route::Settings) => settings::screen(ctx, settings::SettingsTab::Automation),
    Ok(Route::RotateToken) => settings::asking_rotation(ctx),
    Ok(Route::General) => settings::screen(ctx, settings::SettingsTab::General),
    Ok(Route::Notifications) => settings::screen(ctx, settings::SettingsTab::Notifications),
    Ok(Route::Accounts(view)) => settings::screen(ctx, settings::SettingsTab::Accounts(view)),
    Ok(Route::Orchester(view)) => settings::screen(ctx, settings::SettingsTab::Orchester(view)),
    Ok(Route::Invite(target)) => settings::orchester::landing::screen(ctx, &target),
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
    ProjectPage::Orchestrator(chat) => orchestrator::screen(ctx, project, *chat),
    ProjectPage::OrchestratorSettings => orchestrator::settings(ctx, project),
    ProjectPage::MergeQueue => merge_queue::screen(ctx, project),
    ProjectPage::Memory(tab) => memory::screen(ctx, project, *tab),
    ProjectPage::Tasks(list) => tasks::screen(ctx, project, list),
    ProjectPage::NewTask => task_form::new_task(ctx, project, None),
    ProjectPage::NewTaskInModule(module) => task_form::new_task(ctx, project, Some(*module)),
    ProjectPage::EditTask(key) => task_form::edit_task(ctx, project, key),
    ProjectPage::Task(key, view) => task::screen(ctx, project, (key, false), view),
    ProjectPage::KeptTask(key, view) => task::screen(ctx, project, (key, true), view),
    ProjectPage::Modules(tab) => modules::list(ctx, project, *tab),
    ProjectPage::NewModule => modules::new_module(ctx, project),
    ProjectPage::Module(id, tab) => modules::page(ctx, project, *id, *tab),
    ProjectPage::EditModule(id) => modules::edit_module(ctx, project, *id),
    ProjectPage::PullRequests(tab) => pull_requests::screen(ctx, project, *tab),
    ProjectPage::PullRequest(number, file) => pull_requests::pull_request(
      ctx,
      project,
      *number,
      pull_requests::PullRequestShown::Files(file.as_deref().map(Arc::from)),
    ),
    ProjectPage::PullRequestFeedback(number, thread) => pull_requests::pull_request(
      ctx,
      project,
      *number,
      pull_requests::PullRequestShown::Feedback(*thread),
    ),
    ProjectPage::Usage(query) => usage::screen(ctx, project, query),
    ProjectPage::Delivery(period) => delivery::screen(ctx, project, *period),
    ProjectPage::Pipelines => pipelines::screen(ctx, project),
    ProjectPage::NewPipeline | ProjectPage::NewEmptyPipeline | ProjectPage::CopyPipeline(_) => {
      pipelines::new_pipeline(ctx, project)
    }
    ProjectPage::Pipeline(id, view) => pipelines::editor(ctx, project, *id, (view.clone(), None)),
    ProjectPage::PipelineVersion(id, version, view) => {
      pipelines::editor(ctx, project, *id, (view.clone(), Some(*version)))
    }
    ProjectPage::Agents => agents::screen(ctx, project, agents::Shown::List),
    ProjectPage::NewAgent(tab) => agents::screen(ctx, project, agents::Shown::New(*tab)),
    ProjectPage::Agent(id, tab) => agents::screen(ctx, project, agents::Shown::Edit(*id, *tab)),
    ProjectPage::Instructions(view) => instructions::screen(ctx, project, *view),
    ProjectPage::Skills(_) | ProjectPage::NewSkill | ProjectPage::Skill(_) | ProjectPage::ImportSkills => {
      skills::screen(ctx, project, page)
    }
    ProjectPage::Tools | ProjectPage::NewTool | ProjectPage::Tool(..) => tools::screen(ctx, project, page),
    ProjectPage::Secrets(view) => project_settings::screen(ctx, project, project_settings::Shown::Secrets(*view)),
    ProjectPage::Connections(view) => {
      project_settings::screen(ctx, project, project_settings::Shown::Connections(*view))
    }
    ProjectPage::NewConnection(kind) => {
      project_settings::screen(ctx, project, project_settings::Shown::NewConnection(*kind))
    }
    ProjectPage::EditConnection(id) => {
      project_settings::screen(ctx, project, project_settings::Shown::EditConnection(*id))
    }
    ProjectPage::Settings(tab) => project_settings::screen(ctx, project, project_settings::Shown::Tab(*tab)),
    ProjectPage::ProjectRules => project_settings::screen(ctx, project, project_settings::Shown::Rules),
    ProjectPage::MakeLocal => project_settings::make_local_screen(ctx, project),
    ProjectPage::Join(target) => project_settings::join_screen(ctx, project, target),
    ProjectPage::Files(view) => files::screen(ctx, project, view),
    ProjectPage::Import(tab) => import::screen(ctx, project, *tab),
  }
}
