//! The screens the router shows inside the shell, one module per screen. Every route of the router's
//! table comes here: the path is read as a [`Route`] once, and its screen gets what the route names. A
//! project's screen gets the project open ([`ShownProject`]); until it is, the content panel stays empty,
//! and a project that cannot be opened shows why instead (`unavailable_project`). A path that names nothing
//! says why (`not_found`).

mod add_project;
mod agents;
mod all_projects;
pub(crate) mod cloud_sign_in;
mod connections;
mod delivery;
mod files;
mod first_download;
pub(crate) mod hub;
mod instructions;
pub(crate) mod join_team;
mod kept;
mod local_hub;
pub(crate) mod memory;
pub(crate) mod modules;
mod not_found;
mod orchestrator;
mod overview;
mod pipeline_hub;
mod pipelines;
mod placeholder;
mod project_settings;
mod review;
mod reviewer_trouble;
mod secrets;
pub(crate) mod settings;
mod skills;
pub(crate) mod task;
mod task_close;
mod task_form;
pub(crate) mod tasks;
mod team_hub;
mod tools;
mod unavailable_project;
pub(crate) mod usage;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::StepKey,
};

use crate::{
  project::{ProjectState, Projects, ShownProject, TeamRead},
  routes::{
    AgentTab, HubFilter, HubView, NewConnection, PipelineView, PipelinesFilter, PipelinesView, ProjectPage, Route,
  },
};

/// The screen at the router's path.
pub fn screen(ctx: &mut Ctx) -> Element {
  let path = ctx.route_path();
  match Route::parse(&path) {
    Ok(Route::AllProjects) => all_projects::screen(ctx),
    Ok(Route::Usage(query)) => all_projects::usage_screen(ctx, *query),
    Ok(Route::AddProject(folder)) => add_project::screen(ctx, folder.map(|folder| folder.path())),
    Ok(Route::JoinTeam(way)) => join_team::screen(ctx, way),
    Ok(Route::FirstDownload(project)) => first_download::screen(ctx, project),
    Ok(Route::Settings) => settings::screen(ctx, settings::SettingsTab::Automation),
    Ok(Route::RotateToken) => settings::asking_rotation(ctx),
    Ok(Route::General) => settings::screen(ctx, settings::SettingsTab::General(None)),
    Ok(Route::Updates(show)) => settings::screen(
      ctx,
      settings::SettingsTab::General(Some(settings::GeneralShow::Updates(show))),
    ),
    Ok(Route::PlanLimits) => settings::screen(
      ctx,
      settings::SettingsTab::General(Some(settings::GeneralShow::PlanLimits)),
    ),
    Ok(Route::Notifications) => settings::screen(ctx, settings::SettingsTab::Notifications),
    Ok(Route::Browser) => settings::screen(ctx, settings::SettingsTab::Browser),
    Ok(Route::Accounts(view)) => settings::screen(ctx, settings::SettingsTab::Accounts(view)),
    Ok(Route::Orchester(view)) => settings::screen(ctx, settings::SettingsTab::Orchester(view)),
    Ok(Route::Invite(target)) => settings::invite(ctx, target),
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
    // The Overview shows every section on one page; a tab older links name lands on it all the same.
    ProjectPage::Overview(_) => overview::screen(ctx, project),
    ProjectPage::Orchestrator(chat) => orchestrator::screen(ctx, project, *chat),
    ProjectPage::OrchestratorSettings => orchestrator::settings(ctx, project),
    ProjectPage::Review => review::list(ctx, project),
    ProjectPage::TaskReview(key) => review::task::screen(ctx, project, key),
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
    ProjectPage::TaskMerge(key, number) => task::merge::attempt::screen(ctx, project, key, *number),
    ProjectPage::Usage(query) => usage::screen(ctx, project, query),
    ProjectPage::Delivery(period) => delivery::screen(ctx, project, *period),
    ProjectPage::Pipelines => pipelines_or_hub(ctx, project, PipelinesView::default()),
    ProjectPage::PipelinesHub(view) => pipelines_or_hub(ctx, project, *view),
    ProjectPage::NewPipeline | ProjectPage::NewEmptyPipeline | ProjectPage::CopyPipeline(_) => {
      pipelines::new_pipeline(ctx, project)
    }
    ProjectPage::Pipeline(id, PipelineView::Step(key)) => pipeline_step(ctx, project, *id, key),
    ProjectPage::Pipeline(id, view) => pipelines::editor(ctx, project, *id, (view.clone(), None)),
    ProjectPage::PipelineVersion(id, version, view) => {
      pipelines::editor(ctx, project, *id, (view.clone(), Some(*version)))
    }
    ProjectPage::Agents => agents_or_hub(ctx, project, HubView::default()),
    ProjectPage::AgentsHub(view) => agents_or_hub(ctx, project, *view),
    ProjectPage::NewAgent(tab) => new_agent(ctx, project, *tab),
    ProjectPage::Agent(id, tab) => agents::screen(ctx, project, agents::Shown::Edit(*id, *tab)),
    ProjectPage::Instructions(view) => instructions::screen(ctx, project, *view),
    ProjectPage::Skills(_) | ProjectPage::NewSkill | ProjectPage::Skill(_) => skills::screen(ctx, project, page),
    ProjectPage::Tools | ProjectPage::NewTool | ProjectPage::Tool(..) => tools::screen(ctx, project, page),
    ProjectPage::Secrets(view) => project_settings::screen(ctx, project, project_settings::Shown::Secrets(*view)),
    ProjectPage::Connections(view) => {
      project_settings::screen(ctx, project, project_settings::Shown::Connections(*view))
    }
    ProjectPage::NewConnection(NewConnection::McpServer) => new_mcp_server(ctx, project),
    ProjectPage::NewConnection(kind) => {
      project_settings::screen(ctx, project, project_settings::Shown::NewConnection(*kind))
    }
    ProjectPage::EditConnection(id) => {
      project_settings::screen(ctx, project, project_settings::Shown::EditConnection(*id))
    }
    ProjectPage::Settings(tab) => project_settings::screen(ctx, project, project_settings::Shown::Tab(*tab)),
    ProjectPage::ProjectRules => project_settings::screen(ctx, project, project_settings::Shown::Rules),
    ProjectPage::Storage => project_settings::screen(ctx, project, project_settings::Shown::Storage),
    ProjectPage::MakeLocal => project_settings::make_local_screen(ctx, project),
    ProjectPage::Join(target) => project_settings::join_screen(ctx, project, target),
    ProjectPage::Files(view) => files::screen(ctx, project, view),
  }
}

/// New profile: the hub's form in a local project (`Agents · v2` L3), else the page of its own.
fn new_agent(ctx: &mut Ctx, project: &ShownProject, tab: AgentTab) -> Element {
  local_hub::form_or(ctx, project, HubFilter::Profiles, |ctx| {
    agents::screen(ctx, project, agents::Shown::New(tab))
  })
}

/// New MCP server: the hub's form in a local project (L4), else the connections' page of its own.
fn new_mcp_server(ctx: &mut Ctx, project: &ShownProject) -> Element {
  local_hub::form_or(ctx, project, HubFilter::McpServers, |ctx| {
    project_settings::screen(
      ctx,
      project,
      project_settings::Shown::NewConnection(NewConnection::McpServer),
    )
  })
}

/// The Agents hub is master–detail (`Agents · v2`) in a team project ([`team_hub`]) and in a local one ([`local_hub`]).
/// Whether the project is shared is read from its database with the feed, so the page waits for that read, and says so
/// when it failed, rather than show the local hub for a team project. A team's hub has no *Archived* filter: a route
/// that names it shows All.
fn agents_or_hub(ctx: &mut Ctx, project: &ShownProject, view: HubView) -> Element {
  match project.feed.team.get() {
    TeamRead::Read(Some(team)) => {
      let view = HubView {
        filter: if view.filter == HubFilter::Archived {
          HubFilter::All
        } else {
          view.filter
        },
        ..view
      };
      team_hub::screen(ctx, project, team, view)
    }
    TeamRead::Read(None) => local_hub::screen(ctx, project, view),
    TeamRead::Unread => team_hub::unread::reading(ctx),
    TeamRead::Failed(error) => team_hub::unread::failed(ctx, project, error),
  }
}

/// The Pipelines page: the hub (`Pipelines · v2`) in a local project ([`pipeline_hub`]), the page it has in a team one.
/// Whether the project is shared is read with the feed, as for the Agents hub ([`agents_or_hub`]), so the page waits
/// for that read and says so when it failed.
fn pipelines_or_hub(ctx: &mut Ctx, project: &ShownProject, view: PipelinesView) -> Element {
  match project.feed.team.get() {
    TeamRead::Read(None) => pipeline_hub::screen(ctx, project, view, None),
    TeamRead::Read(Some(_)) => pipelines::screen(ctx, project),
    TeamRead::Unread => team_hub::unread::reading(ctx),
    TeamRead::Failed(error) => team_hub::unread::failed(ctx, project, error),
  }
}

/// A step of pipeline `id` (a card of the flow, a fix): in a local project it opens in the Pipelines hub's panel
/// (`Pipelines · v2` P2), which keeps the edit the pipeline's panel holds; a team project keeps the step's own page.
fn pipeline_step(ctx: &mut Ctx, project: &ShownProject, id: ResourceId, key: &StepKey) -> Element {
  match project.feed.team.get() {
    TeamRead::Read(None) => {
      let view = PipelinesView::new(PipelinesFilter::All, Some(id));
      pipeline_hub::screen(ctx, project, view, Some(key.clone()))
    }
    TeamRead::Read(Some(_)) => pipelines::editor(ctx, project, id, (PipelineView::Step(key.clone()), None)),
    TeamRead::Unread => team_hub::unread::reading(ctx),
    TeamRead::Failed(error) => team_hub::unread::failed(ctx, project, error),
  }
}
