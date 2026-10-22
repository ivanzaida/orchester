//! The Delivery view's pinned header, laid out as the Usage page's (`screens::usage::toolbar`, no frame of its own
//! yet): the title *Delivery* and what it shows of the project, and at its end the *Usage* · *Delivery* switch, which
//! the Usage page's header has too, and the period (*30 days* · *90 days* · *365 days*). Each choice is a new entry of
//! the tab's history.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::delivery::DeliveryPeriod;

use super::words;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, UsageSpacing, UsageText},
  ui::{
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// *Usage* · *Delivery*: the two views of a project's Usage; `delivery` when the Delivery view shows.
pub fn view_switch(ctx: &mut Ctx, project: ProjectId, delivery: bool) -> Element {
  let navigation = Navigation::of(ctx);
  let to_usage = navigation.clone();
  segmented(
    "usage-views",
    [
      Segment::new(
        "usage-view-usage",
        SegmentFace::Text(ctx.t("delivery.switch.usage")),
        !delivery,
        move || to_usage.go(Route::Project(project, ProjectPage::Usage(Box::default()))),
      ),
      Segment::new(
        "usage-view-delivery",
        SegmentFace::Text(ctx.t("delivery.switch.delivery")),
        delivery,
        move || {
          navigation.go(Route::Project(
            project,
            ProjectPage::Delivery(DeliveryPeriod::default()),
          ))
        },
      ),
    ],
    Focus::Pointer,
  )
  .into()
}

pub fn toolbar(ctx: &mut Ctx, (project, name): (ProjectId, &str), period: DeliveryPeriod) -> Element {
  let heading = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&ctx.t("delivery.title"))
        .id("delivery-title")
        .variant(ExtraTypography::Display.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t_args("delivery.description", [("project", name)]))
        .id("delivery-description")
        .variant(UsageText::Description.role()),
    );
  let controls = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(view_switch(ctx, project, true))
    .child(periods(ctx, project, period));
  Column::new()
    .id("delivery-toolbar")
    .width(Dimension::Pct(100.0))
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .padding(
      Padding::new()
        .top(UsageSpacing::Top.role())
        .bottom(ExtraSpacing::Inset.role())
        .left(UsageSpacing::Page.role())
        .right(UsageSpacing::Page.role()),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::End)
        .child(heading)
        .child(controls),
    )
    .into()
}

/// *30 days* · *90 days* · *365 days*.
fn periods(ctx: &mut Ctx, project: ProjectId, chosen: DeliveryPeriod) -> Element {
  let navigation = Navigation::of(ctx);
  let segments = DeliveryPeriod::ALL.map(|period| {
    let navigation = navigation.clone();
    Segment::new(
      format!("delivery-period-{}", period.name()),
      SegmentFace::Text(words::period(ctx, period)),
      period == chosen,
      move || navigation.go(Route::Project(project, ProjectPage::Delivery(period))),
    )
  });
  segmented("delivery-period", segments, Focus::Pointer).into()
}
