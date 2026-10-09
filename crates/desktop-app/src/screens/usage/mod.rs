//! The project's usage: tokens and time its runs spent. Not built yet.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ProjectId;

use super::placeholder::placeholder;
use crate::routes::Screen;

pub fn screen(ctx: &mut Ctx, _project: ProjectId) -> Element {
  placeholder(ctx, Screen::Usage)
}
