//! The project's usage: tokens and time its runs spent. Not built yet.

use lurq::{app::ctx::Ctx, node::Element};

use super::placeholder::placeholder;
use crate::{project::ShownProject, routes::Screen};

pub fn screen(ctx: &mut Ctx, _project: &ShownProject) -> Element {
  placeholder(ctx, Screen::Usage)
}
