use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, Placement},
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Popup, Row},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::TaskId;

use super::organization::tag_chip;
use crate::ui::pressable::surface;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChipNames(Vec<Arc<str>>);
impl lurq::app::component::DevtoolsInspectable for ChipNames {}
struct RowTags {
  open: Signal<bool>,
  anchor: ElementRef,
}
impl Component for RowTags {
  type Props = ChipNames;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let names = &ctx.props::<ChipNames>().0;
    let mut row = Row::new()
      .spacing(SpacingSize::Xs)
      .with_children(names.iter().take(ROW_TAG_LIMIT).map(|name| tag_chip(name)));
    if names.len() > ROW_TAG_LIMIT {
      let open = self.open.clone();
      let label = ctx.t_args(
        "tasks.tags.more",
        [("count", (names.len() - ROW_TAG_LIMIT).to_string().as_str())],
      );
      row = row.child(
        surface()
          .ref_element(self.anchor.clone())
          .on_click(move |event: MouseEvent| {
            event.stop_propagation();
            open.update(|value| *value = !*value);
          })
          .child(tag_chip(&label)),
      );
      let menu = Column::new()
        .width(Dimension::Px(MENU_WIDTH))
        .padding(SpacingSize::Sm)
        .spacing(SpacingSize::Sm)
        .background(PaletteColor::SurfaceRaised)
        .rounded(RadiusSize::Md)
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .with_children(names.iter().map(|name| tag_chip(name)))
        .on_click(|event: MouseEvent| event.stop_propagation());
      row = row.child(
        Popup::new(self.anchor.clone(), menu)
          .open(self.open.clone())
          .placement(Placement::BottomStart),
      );
    }
    row
  }
}

pub fn row_tag_chips(ctx: &mut Ctx, task: TaskId, names: &[Arc<str>]) -> Element {
  ctx.mount_keyed::<RowTags>(&format!("row-tags-{task}"), ChipNames(names.to_vec()))
}

// yrT73: row chip overflow opens the remaining names.
const ROW_TAG_LIMIT: usize = 2;
const MENU_WIDTH: f32 = 240.0;
