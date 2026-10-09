//! All projects: every project the app knows, and adding one. Not built yet.

use lurq::{app::ctx::Ctx, node::Element};

use super::placeholder::placeholder;
use crate::routes::Screen;

pub fn screen(ctx: &mut Ctx) -> Element {
  placeholder(ctx, Screen::AllProjects)
}
