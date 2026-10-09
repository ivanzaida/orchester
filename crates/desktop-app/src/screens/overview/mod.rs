//! The project's Overview (`Overview · full page · dark` `q9mf0k`), its task list on `tab`. Not built yet.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ProjectId;

use super::placeholder::placeholder;
use crate::routes::{OverviewTab, Screen};

pub fn screen(ctx: &mut Ctx, _project: ProjectId, _tab: OverviewTab) -> Element {
  placeholder(ctx, Screen::Overview)
}
