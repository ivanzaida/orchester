//! A task's page on the tab `view` names (`Task · Summary · running · full page · dark` `lUXff`,
//! `Task · Conversation · dark` `kazPZ`, `Task · Changes · dark` `DYun5`). Not built yet.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{id::ProjectId, project::TaskKey};

use super::placeholder::titled;
use crate::routes::{Screen, TaskView};

pub fn screen(ctx: &mut Ctx, _project: ProjectId, key: &TaskKey, view: &TaskView) -> Element {
  let tab = ctx.t(view.tab().title_key());
  let title = ctx.t_args(
    "shell.screens.taskTab",
    [("key", key.to_string()), ("tab", tab.to_string())],
  );
  titled(ctx, Screen::Task, &title)
}
