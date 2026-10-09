//! A task's page on the tab `view` names (`Task · Summary · running · full page · dark` `lUXff`,
//! `Task · Conversation · dark` `kazPZ`, `Task · Changes · dark` `DYun5`). Only the Changes tab is built,
//! on a stand-in frame until the page's frame is.

mod changes;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::project::TaskKey;

use crate::{project::ShownProject, routes::TaskView};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey, view: &TaskView) -> Element {
  changes::screen(ctx, project, key, view)
}
