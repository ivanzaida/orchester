//! The project's Tasks list (`Tasks · full page · dark` `C2E5Po`) as `list` names it: its tab, its search
//! and the task a Create just added (`Tasks · new task highlighted` `t3oAOi`). Not built yet.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ProjectId;

use super::placeholder::placeholder;
use crate::routes::{Screen, TaskList};

pub fn screen(ctx: &mut Ctx, _project: ProjectId, _list: &TaskList) -> Element {
  placeholder(ctx, Screen::Tasks)
}
