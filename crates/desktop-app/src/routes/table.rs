//! The router's table: every screen's patterns inside the shell, and a fallback inside the shell for
//! paths that name nothing.

use lurq::router::Routes;

use super::Screen;
use crate::{screens, shell};

pub fn routes() -> Routes {
  Routes::new().layout("/", shell::layout, |routes| {
    Screen::ALL
      .into_iter()
      .flat_map(Screen::patterns)
      .fold(routes, |routes, pattern| routes.route(&pattern, screens::screen))
      .fallback(screens::not_found)
  })
}
