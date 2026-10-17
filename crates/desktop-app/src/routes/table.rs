//! The router's table: every screen's pattern inside the shell, and a fallback inside the shell for
//! paths that name nothing. Every entry shows `screens::screen`, which reads the whole path, query
//! included, as a route or as why it names none; the code viewer's is a layout that shows it, whose
//! routes only match the address.

use lurq::{app::ctx::Ctx, node::Element, router::Routes};

use super::Screen;
use crate::{screens, shell};

pub fn routes() -> Routes {
  Routes::new().layout("/", shell::layout, |routes| {
    Screen::ALL
      .into_iter()
      .filter(|screen| *screen != Screen::Files)
      .fold(routes, |routes, screen| {
        routes.route(&screen.pattern(), screens::screen)
      })
      .layout(&Screen::Files.pattern(), screens::screen, |files| {
        files.route("", matched_only).route(super::FILE_REST, matched_only)
      })
      .route(super::NEW_PIPELINE_PATTERN, screens::screen)
      .route(super::PIPELINE_PATTERN, screens::screen)
      .route(super::AGENT_PATTERN, screens::screen)
      .route(super::PULL_REQUEST_PATTERN, screens::screen)
      .route(super::TASK_SUMMARY_PATTERN, screens::screen)
      .route(super::EVERY_PROJECT_USAGE_PATTERN, screens::screen)
      .fallback(screens::screen)
  })
}

/// What the code viewer's routes, with no file and with one, show: nothing. The router mounts a page per route,
/// so the screen's layout shows the page and it keeps its tree, open folders and reads as a file is selected;
/// these only match the address.
fn matched_only(_: &mut Ctx) -> Element {
  Element::new()
}
