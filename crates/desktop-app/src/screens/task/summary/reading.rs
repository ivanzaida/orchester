//! The Summary while the task is read (`02 Task · Summary · loading` `d5E8Y`): its section headings and property names
//! stay; the run's meta and steps, the latest event, the brief, the acceptance criteria, the output and every property
//! value are bones in their own geometry, the lower rows fading.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer},
  layout::Alignment,
  node::{border::Border, dimension::Dimension, padding::Padding},
};

use super::{PROPERTIES_WIDTH, properties, section_head};
use crate::{
  theme::{ExtraSpacing, TaskSpacing, TaskText},
  ui::skeleton::{self, bone, fading, round_bone},
};

/// The run's steps: each one's mark, number and name bones (`Run` in `d5E8Y`).
const STEPS: [f32; 5] = [44.0, 30.0, 64.0, 60.0, 40.0];
/// The brief's lines, the criteria's and the output's.
const BRIEF: [f32; 2] = [870.0, 610.0];
const CRITERIA: [f32; 5] = [458.0, 392.0, 522.0, 326.0, 424.0];
const OUTPUT: [(f32, f32); 2] = [(90.0, 162.0), (74.0, 172.0)];
/// Each property's key and its value's bone.
const PROPERTIES: [(&str, f32); 8] = [
  ("summary.properties.status", 78.0),
  ("summary.properties.priority", 44.0),
  ("summary.properties.pipeline", 120.0),
  ("summary.properties.tags", 108.0),
  ("summary.properties.dependsOn", 22.0),
  ("summary.properties.limits", 128.0),
  ("summary.properties.branch", 96.0),
  ("summary.properties.created", 86.0),
];

/// The Summary's column and its properties in bones.
pub fn bones(ctx: &Ctx) -> Row {
  let column = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(TaskSpacing::Sections.role())
    .child(run(ctx))
    .child(brief(ctx))
    .child(criteria(ctx))
    .child(output(ctx));
  let values = PROPERTIES.iter().enumerate().map(|(at, (key, width))| {
    let value = skeleton::line(ctx, TypographyStyle::Description, *width);
    properties::row(&format!("task-reading-property-{at}"), &ctx.t(key), value, TaskSpacing::PropertyRow.role())
  });
  let properties = Column::new()
    .width(Dimension::Px(PROPERTIES_WIDTH))
    .flex_shrink(0.0)
    .padding(Padding::new().left(SpacingSize::Xl))
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(values)
    .child(
      Row::new()
        .padding(Padding::vertical(SpacingSize::Sm))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(bone(14.0, 14.0))
        .child(skeleton::line(ctx, TypographyStyle::Description, 112.0)),
    );
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xl)
        .left(TaskSpacing::Side.role())
        .right(TaskSpacing::Side.role())
        .bottom(TaskSpacing::Foot.role()),
    )
    .spacing(TaskSpacing::Columns.role())
    .align_items(Alignment::Start)
    .child(column)
    .child(properties)
}

/// Run, its meta, its steps along their line and the latest event's box.
fn run(ctx: &Ctx) -> Column {
  let mut steps = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center);
  for (at, name) in STEPS.iter().enumerate() {
    if at > 0 {
      steps = steps.child(Row::new().flex(1.0).height(Dimension::Px(1.0)).background(PaletteColor::Border));
    }
    steps = steps.child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(round_bone(12.0))
        .child(bone(14.0, 8.0))
        .child(skeleton::line(ctx, TypographyStyle::Description, *name)),
    );
  }
  let event = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(round_bone(12.0))
    .child(skeleton::line(ctx, TypographyStyle::Description, 456.0))
    .child(Spacer::new().flex(1.0))
    .child(skeleton::line(ctx, TypographyStyle::Description, 92.0));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      section_head(ctx, "task-reading-run", "summary.run.title", None).child(skeleton::line(
        ctx,
        TypographyStyle::Description,
        154.0,
      )),
    )
    .child(steps)
    .child(event)
}

/// Brief, its edit button's bone and its lines.
fn brief(ctx: &Ctx) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      section_head(ctx, "task-reading-brief", "summary.brief", None)
        .child(Spacer::new().flex(1.0))
        .child(skeleton::button(96.0)),
    )
    .with_children(BRIEF.iter().map(|width| skeleton::line(ctx, TaskText::Brief.role(), *width)))
}

/// Acceptance criteria with its count, each criterion's number and words, a line under each.
fn criteria(ctx: &Ctx) -> Column {
  let rows = CRITERIA
    .iter()
    .map(|width| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::vertical(SpacingSize::Sm))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(bone(14.0, 8.0))
        .child(skeleton::line(ctx, TaskText::Brief.role(), *width))
    })
    .collect();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(section_head(ctx, "task-reading-criteria", "summary.criteria", None).child(skeleton::count()))
    .child(Column::new().width(Dimension::Pct(100.0)).with_children(fading(rows)))
}

/// Output with its count and its reports' rows.
fn output(ctx: &Ctx) -> Column {
  let rows = OUTPUT
    .iter()
    .map(|(title, line)| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .rounded(RadiusSize::Md)
        .background(PaletteColor::SurfaceRaised)
        .child(bone(14.0, 14.0))
        .child(skeleton::line(ctx, TypographyStyle::Description, *title))
        .child(skeleton::line(ctx, TypographyStyle::Description, *line))
    })
    .collect::<Vec<_>>();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .opacity(crate::theme::SKELETON_FADE[1])
    .child(section_head(ctx, "task-reading-output", "summary.output.title", None).child(skeleton::count()))
    .with_children(rows)
}
