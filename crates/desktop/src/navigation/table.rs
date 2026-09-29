//! The route table. [`super::route::Route`] says what a path means; this table decides which
//! component instances a path renders into. Each project section is a layout, so its screen
//! stays mounted while only its selection (a profile, a connection, a tab, a pipeline) changes,
//! as it did before history existed. Guards turn a path that stands for nothing (an unknown
//! project, a malformed id) into the nearest place that exists.

use lurq::{
  app::ctx::Ctx,
  components::Outlet,
  node::Element,
  router::{GuardAction, RouteMatch, Routes},
};
use orchester_core::id::ProjectId;

use super::{
  Navigation, Places,
  route::{Page, Route},
};
use crate::{
  root::with_status_bar,
  runner::Runners,
  screens::{all_projects::AllProjects, project, setup::ProjectSetup, welcome::Welcome},
};

/// The `:project` parameter of project paths.
pub const PROJECT_PARAM: &str = "project";

pub fn routes(places: &Places) -> Routes {
  let (start, project, other) = (places.clone(), places.clone(), places.clone());
  Routes::new()
    // Empty until the start screen is known; afterwards it leads there.
    .route("/", |_ctx| Element::new())
    .guard(move |_matched| start.start().map_or(GuardAction::Allow, redirect))
    .layout("/project/:project", project::shell, project_pages)
    .guard(move |matched| project_guard(&project, matched))
    .layout("/", outside_project, |routes| {
      routes
        .route("/welcome", |ctx| ctx.mount_keyed::<Welcome>("welcome", ()))
        .route("/setup", |ctx| ctx.mount_keyed::<ProjectSetup>("setup", ()))
        .route("/projects", |ctx| ctx.mount_keyed::<AllProjects>("all-projects", ()))
        .fallback(|_ctx| Element::new())
    })
    .guard(move |matched| outside_guard(&other, matched))
}

fn project_pages(routes: Routes) -> Routes {
  routes
    .route("/", project::page)
    .route("/tasks", project::page)
    .route("/tasks/new", project::page)
    .route("/tasks/:task", project::page)
    .route("/tasks/:task/edit", project::page)
    .route("/tasks/:task/retry", project::page)
    .route("/pull-requests", project::page)
    .route("/pull-requests/:number", project::page)
    .layout("/pipelines", project::page, |routes| selection(routes, "/:pipeline"))
    .layout("/instructions", project::page, |routes| {
      selection(routes, "/:instruction")
    })
    .layout("/skills", project::page, |routes| selection(routes, "/:skill"))
    .layout("/agents", project::page, |routes| {
      selection(routes, "/:profile").route("/new", |_ctx| Element::new())
    })
    .layout("/tools", project::page, |routes| {
      selection(routes, "/:tool").route("/new", |_ctx| Element::new())
    })
    .layout("/secrets", project::page, |routes| selection(routes, "/:secret"))
    .layout("/connections", project::page, |routes| {
      selection(routes, "/:connection")
    })
    .layout("/settings", project::page, |routes| {
      selection(routes, "/limits").route("/environment", |_ctx| Element::new())
    })
    .fallback(|_ctx| Element::new())
}

/// Paths below a section layout: the section's screen reads the selection from the path, so
/// they render nothing themselves.
fn selection(routes: Routes, selected: &str) -> Routes {
  routes
    .route("/", |_ctx| Element::new())
    .route(selected, |_ctx| Element::new())
}

/// Welcome, setup and All projects, above the status bar without a project.
fn outside_project(ctx: &mut Ctx) -> Element {
  ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens")
    .show(None);
  let navigation = ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens");
  // Going back and forward passes no guard. An entry that no longer applies (the welcome
  // screen before a project was created) gives way to the start screen, so going back stops
  // at the project.
  let places = navigation.places();
  if Route::parse(&ctx.route_path()).is_some_and(|route| !places.applies(route)) {
    navigation
      .router()
      .replace(places.start().unwrap_or(Route::Start).path());
    return Element::new();
  }
  let screen = Outlet::mount(ctx);
  with_status_bar(ctx, screen)
}

fn project_guard(places: &Places, matched: &RouteMatch) -> GuardAction {
  let project = matched
    .params()
    .get_parsed::<ProjectId>(PROJECT_PARAM)
    .filter(|project| places.project(*project).is_some());
  let Some(project) = project else {
    return to_start(places);
  };
  if Route::parse(matched.path()).is_some() {
    return GuardAction::Allow;
  }
  let nearest = match Route::nearest(matched.path()) {
    Some(route @ Route::Project(..)) => route,
    _ => Route::Project(project, Page::OVERVIEW),
  };
  redirect(nearest)
}

fn outside_guard(places: &Places, matched: &RouteMatch) -> GuardAction {
  match Route::parse(matched.path()) {
    Some(route) if places.applies(route) => GuardAction::Allow,
    _ => to_start(places),
  }
}

/// The start screen, or the empty start route while it is not known yet.
fn to_start(places: &Places) -> GuardAction {
  redirect(places.start().unwrap_or(Route::Start))
}

fn redirect(route: Route) -> GuardAction {
  GuardAction::Redirect(route.path())
}
