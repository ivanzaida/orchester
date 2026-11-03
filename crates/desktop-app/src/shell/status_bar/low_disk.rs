//! Storage's pinned low-disk item (SqEDB): no dismiss; the full item opens Storage.
use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  layout::layout_kind::ShrinkLimit,
  node::Element,
};
use orchester_core::{id::ProjectId, workspaces::GIGABYTE};

use super::{
  facts::{Dispatch, go},
  item::{dropping, give_way, glyph, label, pressed},
};
use crate::{
  routes::{ProjectPage, Route},
  services::storage,
  ui::{icon::Icon, tooltip::Tooltip},
};

#[derive(Clone)]
pub struct Props {
  pub project: ProjectId,
  pub root: PathBuf,
  pub dispatch: Dispatch,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.root == other.root
  }
}
impl DevtoolsInspectable for Props {}
pub struct LowDisk;
impl Component for LowDisk {
  type Props = Props;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let reading = storage::follow(ctx, props.root, 0);
    let Some(Ok(disk)) = reading else {
      return Element::from(lurq::components::Row::new());
    };
    item(ctx, props.project, &disk, &props.dispatch).unwrap_or_else(|| lurq::components::Row::new().into())
  }
}

pub fn item(ctx: &mut Ctx, project: ProjectId, disk: &storage::Disk, dispatch: &Dispatch) -> Option<Element> {
  if !disk.guard.holds() {
    return None;
  }
  let free = disk.guard.disk?.free / GIGABYTE;
  let numbers = ctx.t_args(
    if disk.paused == 0 {
      "shell.status.low_disk.waiting"
    } else if disk.paused == 1 {
      "shell.status.low_disk.pausedOne"
    } else {
      "shell.status.low_disk.paused"
    },
    [("free", free.to_string()), ("count", disk.paused.to_string())],
  );
  let row = pressed(
    "status-low-disk",
    go(dispatch, Route::Project(project, ProjectPage::Storage)),
  )
  .background(PaletteColor::SurfaceRaised)
  .flex_shrink(1.0)
  .shrink_order(give_way::ACTIVITY)
  .shrink_limit(ShrinkLimit::Content)
  .child(glyph(Icon::HardDrive, PaletteColor::Warning))
  .child(dropping(&ctx.t("shell.status.low_disk.title"), PaletteColor::Warning))
  .child(label(&numbers, PaletteColor::Warning))
  .child(glyph(Icon::ChevronRight, PaletteColor::Warning));
  Some(Tooltip::new("status-low-disk", ctx.t("shell.status.low_disk.hint")).wrap(ctx, row))
}
