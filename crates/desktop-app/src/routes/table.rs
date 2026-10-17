//! The router's table: every screen's pattern inside the shell, and a fallback inside the shell for
//! paths that name nothing. Every entry shows `screens::screen`, which reads the whole path, query
//! included, as a route or as why it names none.

use lurq::router::Routes;

use super::Screen;
use crate::{screens, shell};

pub fn routes() -> Routes {
  Routes::new().layout("/", shell::layout, |routes| {
    Screen::ALL
      .into_iter()
      .fold(routes, |routes, screen| {
        routes.route(&screen.pattern(), screens::screen)
      })
      .route(super::NEW_PIPELINE_PATTERN, screens::screen)
      .route(super::PIPELINE_PATTERN, screens::screen)
      .route(super::AGENT_PATTERN, screens::screen)
      .route(super::FILE_PATTERN, screens::screen)
      .route(super::PULL_REQUEST_PATTERN, screens::screen)
      .route(super::INVITE_PATTERN, screens::screen)
      .route(super::TASK_SUMMARY_PATTERN, screens::screen)
      .route(super::EVERY_PROJECT_USAGE_PATTERN, screens::screen)
      .fallback(screens::screen)
  })
}
