//! The project's Overview (`Overview · full page · dark` `q9mf0k`), its task list on `tab`. Not built yet.

use lurq::{app::ctx::Ctx, node::Element};

use super::placeholder::placeholder;
use crate::{
  project::ShownProject,
  routes::{OverviewTab, Screen},
};

pub fn screen(ctx: &mut Ctx, _project: &ShownProject, _tab: OverviewTab) -> Element {
  placeholder(ctx, Screen::Overview)
}
