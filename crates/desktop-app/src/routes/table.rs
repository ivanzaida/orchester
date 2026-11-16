//! The router's table: every screen's pattern inside the shell, and a fallback inside the shell for
//! paths that name nothing. Every entry shows `screens::screen`, which reads the whole path, query
//! included, as a route or as why it names none; the code viewer's and the Pipelines page's are layouts that show it,
//! whose routes only match the address.

use lurq::{app::ctx::Ctx, node::Element, router::Routes};

use super::Screen;
use crate::{screens, shell};

pub fn routes() -> Routes {
  Routes::new().layout("/", shell::layout, |routes| {
    Screen::ALL
      .into_iter()
      .filter(|screen| !matches!(screen, Screen::Files | Screen::Pipelines))
      .fold(routes, |routes, screen| {
        routes.route(&screen.pattern(), screens::screen)
      })
      .layout(&Screen::Files.pattern(), screens::screen, |files| {
        files.route("", matched_only).route(super::FILE_REST, matched_only)
      })
      .layout(&Screen::Pipelines.pattern(), screens::screen, |pipelines| {
        pipelines
          .route("", matched_only)
          .route(super::NEW_PIPELINE_REST, matched_only)
          .route(super::PIPELINE_REST, matched_only)
      })
      .route(super::AGENT_PATTERN, screens::screen)
      .route(super::TASK_REVIEW_PATTERN, screens::screen)
      .route(super::TASK_MERGE_PATTERN, screens::screen)
      .route(super::MODULE_PATTERN, screens::screen)
      .route(super::MODULE_TAB_PATTERN, screens::screen)
      .route(super::DELIVERY_PATTERN, screens::screen)
      .route(super::INVITE_PATTERN, screens::screen)
      .route(super::KEPT_TASK_PATTERN, screens::screen)
      .route(super::TASK_SUMMARY_PATTERN, screens::screen)
      .route(super::EVERY_PROJECT_USAGE_PATTERN, screens::screen)
      .fallback(screens::screen)
  })
}

/// What the code viewer's routes, with no file and with one, and the Pipelines page's routes (its hub, New pipeline,
/// a pipeline with its step) show: nothing. The router mounts a page per route, so the screen's layout shows the page
/// and it keeps its tree, open folders and reads as a file is selected, and the Pipelines hub keeps the edit its panel
/// holds as a step of the pipeline opens and closes; these only match the address.
fn matched_only(_: &mut Ctx) -> Element {
  Element::new()
}
