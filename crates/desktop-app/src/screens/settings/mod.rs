//! The app's settings. Not built yet.

use lurq::{app::ctx::Ctx, node::Element};

use super::placeholder::placeholder;
use crate::routes::Screen;

pub fn screen(ctx: &mut Ctx) -> Element {
  placeholder(ctx, Screen::Settings)
}
