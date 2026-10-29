//! The Merge queue while the queue is read (`07 Review · Merge queue · loading` `zMx0E`): the lead keeps its words, the
//! base box its marks and names while the base and the checkout are bones; which sections exist is data, so the queue
//! is one panel of row bones in the rows' own geometry (state, task key and title, run line, action); Today is always
//! there, so its heading stays with its count a bone. Lower rows fade. *Reading the merge queue…*
//! (`delivery.queue.reading`) names it. The loaded list has no design yet: these bones follow the built rows.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{DeliverySpacing, DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    skeleton::{self, Skeleton, bone, fading, round_bone},
  },
};

/// A row's state column, as `row::row` draws it (`State` in `kcUhy`), and its action's bone.
const STATE_WIDTH: f32 = 168.0;
const ACTION: f32 = 68.0;
/// The queue's rows: each title's and state's bone; Today's rows: each title's.
const QUEUE: [(f32, f32); 4] = [(282.0, 132.0), (240.0, 104.0), (326.0, 152.0), (260.0, 120.0)];
const TODAY: [f32; 3] = [262.0, 218.0, 304.0];

/// The page under the hub's head while the queue is read.
pub(super) fn page(ctx: &mut Ctx) -> Element {
  let queue = QUEUE
    .iter()
    .map(|(title, state)| queue_row(ctx, *title, *state))
    .collect();
  let today = TODAY.iter().map(|title| today_row(ctx, *title)).collect();
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(DeliverySpacing::Sections.role())
    .child(header(ctx))
    .child(panel(fading(queue)))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(Text::new(&ctx.t("delivery.queue.section.today")).variant(ExtraTypography::FormSection.role()))
            .child(skeleton::count()),
        )
        .child(panel(fading(today))),
    );
  Skeleton::new("merge-queue-loading", ctx.t("delivery.queue.reading")).show(ctx, bones)
}

/// The lead's words, and the base box with its marks and names and the base and the checkout as bones.
fn header(ctx: &Ctx) -> Row {
  let key = |key: &str| {
    Text::new(&ctx.t(key))
      .variant(ExtraTypography::Property.role())
      .color(PaletteColor::TextMuted)
  };
  let base = Column::new()
    .flex_shrink(0.0)
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::Inset.role(),
    ))
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::GitBranch, IconSize::Base, PaletteColor::TextSecondary))
        .child(key("delivery.queue.base.key"))
        .child(skeleton::line(ctx, ExtraTypography::PropertyMono.role(), 104.0)),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(round_bone(6.0))
        .child(key("delivery.queue.base.checkout"))
        .child(skeleton::line(ctx, ExtraTypography::PropertyMono.role(), 228.0)),
    );
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xl)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(Text::new(&ctx.t("delivery.queue.leadNoBase")).variant(DeliveryText::Lead.role())),
    )
    .child(base)
}

/// A box of rows on `surface_panel`, as the queue's sections draw theirs.
fn panel(rows: Vec<Row>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .with_children(rows)
}

/// A queued run's row: its state and what it says under it, its task's key and title and its run line, its action.
fn queue_row(ctx: &Ctx, title: f32, state: f32) -> Row {
  let state = Column::new()
    .width(Dimension::Px(STATE_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(round_bone(IconSize::Md.size()))
        .child(skeleton::line(
          ctx,
          DeliveryText::State.role(),
          state - IconSize::Md.size(),
        )),
    )
    .child(skeleton::line(ctx, ExtraTypography::Note.role(), state));
  let run = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(skeleton::line(ctx, DeliveryText::Count.role(), 40.0))
        .child(skeleton::line(ctx, ExtraTypography::Strong.role(), title)),
    )
    .child(skeleton::line(ctx, ExtraTypography::Note.role(), 186.0));
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(state)
    .child(run)
    .child(skeleton::button(ACTION))
}

/// A run delivered today: its mark, its title, and when and how.
fn today_row(ctx: &Ctx, title: f32) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(round_bone(IconSize::Md.size()))
    .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(skeleton::line(
      ctx,
      ExtraTypography::Strong.role(),
      title,
    )))
    .child(bone(62.0, 9.0))
    .child(bone(40.0, 9.0))
}
