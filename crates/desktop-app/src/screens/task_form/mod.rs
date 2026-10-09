//! The task form: New task (`New task · empty · dark` `s7jKkA`) and Edit task (`Edit task · draft · no
//! changes · dark` `Qp1B0`). Not built yet.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::project::TaskKey;

use super::placeholder::{placeholder, titled};
use crate::{project::ShownProject, routes::Screen};

pub fn new_task(ctx: &mut Ctx, _project: &ShownProject) -> Element {
  placeholder(ctx, Screen::NewTask)
}

pub fn edit_task(ctx: &mut Ctx, _project: &ShownProject, key: &TaskKey) -> Element {
  let title = ctx.t_args("shell.screens.editTaskOf", [("key", key.to_string())]);
  titled(ctx, Screen::EditTask, &title)
}
