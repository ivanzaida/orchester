//! One host per open tab, each with a router of its own (its Back/Forward history). The active
//! tab's router is mounted; every other one is offstage: lurq retains its components with their
//! signals, futures, scroll and last render, but leaves them out of layout, painting, input,
//! timers and future polling. A signal change in an inactive tab only marks it for rendering once
//! it is active again. The hosts also keep a [`FeedLoader`] per project the tabs show, and tell the
//! runners which projects stay open.

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Router, Stack},
  node::{Element, dimension::Dimension},
  router::RouterHandle,
};
use orchester_core::id::ProjectId;
use parking_lot::Mutex;

use super::{
  Tabs,
  dirty::TabEdits,
  feed::{FeedLoader, FeedProps},
  model::TabId,
};
use crate::{
  navigation::{self, Navigation, route::Route},
  runner::Runners,
  workspace::OpenProject,
};

#[derive(Clone)]
struct TabHostProps {
  id: TabId,
  /// Where the tab opens; read once, when its host mounts.
  path: Arc<str>,
  active: bool,
}

impl PartialEq for TabHostProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.active == other.active
  }
}

impl lurq::app::component::DevtoolsInspectable for TabHostProps {}

struct TabHost {
  router: RouterHandle,
  navigation: Navigation,
  edits: TabEdits,
}

impl Component for TabHost {
  type Props = TabHostProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<TabHostProps>().clone();
    let tabs = tabs(ctx);
    let router = ctx.router(navigation::routes(tabs.places()));
    router.replace(props.path.to_string());
    tabs.register(props.id, router.clone());
    Self {
      navigation: Navigation::new(
        tabs.workspace().clone(),
        router.clone(),
        tabs.places().clone(),
        tabs.clone(),
      ),
      edits: TabEdits::new(tabs, props.id),
      router,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let TabHostProps { id, active, .. } = ctx.props::<TabHostProps>().clone();
    // Provided on every render: a render from above replaces the inherited context.
    ctx.provide(self.navigation.clone());
    ctx.provide(self.edits.clone());
    let content = Router::mount_offstage(ctx, self.router.clone(), active);
    if active {
      let path = self.router.path().get();
      tabs(ctx).sync(id, &path);
      // The status bar and the run controls follow the active tab's project.
      let shown = Route::parse(&path).and_then(|route| match route {
        Route::Project(project, _) => self.navigation.places().project(project),
        _ => None,
      });
      runners(ctx).show(shown.as_ref());
    }
    content
  }
}

/// The hosts of every tab, stacked; only the active one has content.
pub(super) struct TabHosts {
  /// The projects the tabs showed at the last render, to tell the runners when they change.
  kept: Mutex<Vec<ProjectId>>,
}

impl Component for TabHosts {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      kept: Mutex::new(Vec::new()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tabs = tabs(ctx);
    let set = tabs.set().get();
    let active_project = project_of(set.active_tab().path());
    let projects = self.keep_open(ctx, &tabs, set.tabs().iter().filter_map(|tab| project_of(tab.path())));
    let mut stack = Stack::new().width(Dimension::Pct(100.0)).height(Dimension::Pct(100.0));
    for open in projects {
      let active = Some(open.project.id()) == active_project;
      let key = format!("feed-{}", open.project.id());
      stack = stack.child(ctx.mount_keyed::<FeedLoader>(&key, FeedProps { open, active }));
    }
    for tab in set.tabs() {
      let props = TabHostProps {
        id: tab.id(),
        path: tab.path().clone(),
        active: tab.id() == set.active(),
      };
      stack = stack.child(ctx.mount_keyed::<TabHost>(&format!("tab-{}", tab.id()), props));
    }
    stack
  }
}

impl TabHosts {
  /// The opened projects among `projects`, once each; the runners keep them open.
  fn keep_open(&self, ctx: &mut Ctx, tabs: &Tabs, projects: impl Iterator<Item = ProjectId>) -> Vec<OpenProject> {
    let mut ids: Vec<ProjectId> = Vec::new();
    for project in projects {
      if !ids.contains(&project) {
        ids.push(project);
      }
    }
    let open: Vec<OpenProject> = ids.iter().filter_map(|id| tabs.places().project(*id)).collect();
    let mut kept = self.kept.lock();
    if *kept != ids {
      runners(ctx).keep_open(&open);
      kept.clone_from(&ids);
    }
    open
  }
}

/// The project a tab at `path` shows.
fn project_of(path: &str) -> Option<ProjectId> {
  match Route::parse(path)? {
    Route::Project(project, _) => Some(project),
    Route::Start | Route::Welcome | Route::Setup | Route::AllProjects => None,
  }
}

fn tabs(ctx: &mut Ctx) -> Tabs {
  ctx
    .use_context::<Tabs>()
    .expect("the window provides Tabs before mounting its tabs")
}

fn runners(ctx: &mut Ctx) -> Runners {
  ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting the window")
}
