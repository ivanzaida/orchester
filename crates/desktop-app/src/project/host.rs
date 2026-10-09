use std::path::PathBuf;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{background, messages, workspace::OpenProject};
use parking_lot::Mutex;

use super::{
  ProjectState, Projects, Unavailable,
  feed::{FeedLoader, FeedProps},
  resolve,
};
use crate::{navigation::Navigation, services::Services};

/// Follows the projects the tabs show: opens each one once ([`ProjectLoader`]), keeps the runner of
/// every open one running and shows the active tab's project's (`Runners::keep_open`, `Runners::show`),
/// and forgets what the projects no tab shows any more could not be opened as. Renders the loaders,
/// which render nothing.
pub struct ProjectsHost {
  /// The projects the runners were last told about: kept open, and shown.
  told: Mutex<Option<(Vec<PathBuf>, Option<PathBuf>)>>,
}

impl Component for ProjectsHost {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self { told: Mutex::new(None) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tabs = Navigation::of(ctx).tabs();
    let projects = Projects::of(ctx);
    let mut shown: Vec<ProjectId> = Vec::new();
    for project in tabs.tabs().iter().filter_map(|tab| tab.route().project()) {
      if !shown.contains(&project) {
        shown.push(project);
      }
    }
    projects.forget_unavailable(&shown);
    let active = tabs.active().route().project();
    let open: Vec<OpenProject> = shown
      .iter()
      .filter_map(|project| projects.state(*project)?.open().cloned())
      .collect();
    let shown_open = open.iter().find(|open| Some(open.project.id()) == active);
    self.tell_runners(&Services::of(ctx), &open, shown_open);
    shown.iter().fold(Column::new(), |loaders, &project| {
      let props = LoaderProps {
        project,
        active: Some(project) == active,
      };
      loaders.child(ctx.mount_keyed::<ProjectLoader>(&format!("project-{project}"), props))
    })
  }
}

impl ProjectsHost {
  /// Tells the runners which projects the tabs show and which one the active tab shows, when either
  /// changed since they were told last.
  fn tell_runners(&self, services: &Services, open: &[OpenProject], shown: Option<&OpenProject>) {
    let now = (
      open.iter().map(|open| open.repo_root.clone()).collect(),
      shown.map(|open| open.repo_root.clone()),
    );
    let mut told = self.told.lock();
    if told.as_ref() == Some(&now) {
      return;
    }
    services.runners.keep_open(open);
    services.runners.show(shown);
    *told = Some(now);
  }
}

#[derive(Clone, PartialEq)]
struct LoaderProps {
  project: ProjectId,
  /// Whether the active tab shows the project.
  active: bool,
}

impl lurq::app::component::DevtoolsInspectable for LoaderProps {}

/// Opens one project the tabs show, off the UI thread, and records what it stands for; once it is open,
/// keeps its feed current.
struct ProjectLoader;

impl Component for ProjectLoader {
  type Props = LoaderProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let LoaderProps { project, active } = ctx.props::<LoaderProps>().clone();
    let projects = Projects::of(ctx);
    let workspace = Services::of(ctx).workspace;
    let attempt = projects.attempt(project);
    let resolved = ctx.future((project, attempt), move |(project, _)| {
      let workspace = workspace.clone();
      async move {
        background::run(move || resolve(&workspace, project))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot read the workspace's projects");
            messages::store_error(&error)
          })
      }
    });
    let resolved = resolved.state().get();
    // While it is opened (again), there is no answer yet: the one the attempt before gave is not it.
    let opening = resolved.is_pending();
    match (resolved.data, resolved.error) {
      _ if opening => {}
      (Some(state), _) => projects.set(project, state),
      (None, Some(reason)) => projects.set(project, ProjectState::Unavailable(Unavailable::Failed(reason))),
      (None, None) => {}
    }
    match projects.state(project) {
      Some(ProjectState::Open(open)) => ctx.mount_keyed::<FeedLoader>("feed", FeedProps { open, active }),
      Some(ProjectState::Unavailable(_)) | None => Element::new(),
    }
  }
}
