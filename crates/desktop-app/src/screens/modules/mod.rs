//! Modules (ORC-48): the Modules list ([`list`]) and a module's page with its tasks and its burndown ([`page`]),
//! with the module editor ([`editor`]) on both. The data and the rules are `orchester_workspace::modules`. No frame
//! of `data/orchester2.pen` draws them yet: they follow the Tasks list's header and the Usage page's cards, chart
//! and tables.

mod burndown;
mod chart;
mod editor;
mod list;
mod page;
pub(crate) mod words;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ModuleId;

use crate::project::ShownProject;

/// Whether the module editor is open, and for what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editing {
  Closed,
  New,
  Edit,
}

impl lurq::app::component::DevtoolsInspectable for Editing {}

pub fn list(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let key = format!("modules-{}", project.open.project.id());
  ctx.mount_keyed::<list::ModulesList>(
    &key,
    list::ListProps {
      project: project.clone(),
    },
  )
}

pub fn page(ctx: &mut Ctx, project: &ShownProject, module: ModuleId) -> Element {
  let key = format!("module-{}-{module}", project.open.project.id());
  ctx.mount_keyed::<page::ModuleScreen>(
    &key,
    page::PageProps {
      project: project.clone(),
      module,
    },
  )
}
