//! A lane of the merge queue (ORC-410, 05 `S1pTL`): the runs that deliver into one base branch, one at a time,
//! whatever the other lanes do. Its head (`Lane · master` `BBL6x`): a 14 branch glyph, *Into* and the branch in mono
//! 13/600, the count of its runs in the queue, and at its end how its runs land, a 6 dot before the words (`success`
//! in the checkout, `info` beside it, `warning` while it waits for the checkout). Under it its rows in one `border`
//! box, radius 6. Under the lanes the note that a lane delivers on its own (`Note` `MXB0P`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::merge_queue::QueueState;
use orchester_workspace::merge_queue::{LaneLanding, QueueLaneRead, QueueRead, QueueRow};

use super::{
  header::dot,
  row::{self, Placed, Rows},
};
use crate::{
  theme::{DeliverySpacing, DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// `lane` of `read` with its rows.
pub(super) fn lane(ctx: &mut Ctx, rows: &Rows, read: &QueueRead, lane: &QueueLaneRead) -> Element {
  let shown: Vec<&QueueRow> = read.lane_rows(lane).collect();
  let id = format!("merge-queue-lane-{}", lane.base.branch.as_str());
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(DeliverySpacing::Lanes.role())
    .child(head(ctx, &id, lane, &shown))
    .child(lane_box(ctx, &id, rows, &shown))
    .into()
}

/// The runs that recorded no base branch, in a box with no head.
pub(super) fn without_lane(ctx: &mut Ctx, rows: &Rows, shown: &[&QueueRow]) -> Element {
  lane_box(ctx, "merge-queue-lane-none", rows, shown).into()
}

fn head(ctx: &Ctx, id: &str, lane: &QueueLaneRead, shown: &[&QueueRow]) -> Row {
  let count = shown.iter().filter(|row| row.entry.state.in_queue()).count();
  let (mark, how) = landing(ctx, &lane.lands);
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(DeliverySpacing::LaneHead.role()))
    .spacing(DeliverySpacing::LaneHead.role())
    .align_items(Alignment::Center)
    .describe("lands", landing_name(&lane.lands))
    .child(icon(Icon::GitBranch, IconSize::Md, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t("delivery.queue.lane.into")).variant(ExtraTypography::FormSection.role()))
    .child(
      Text::new(lane.base.branch.as_str())
        .id(format!("{id}-branch"))
        .variant(DeliveryText::LaneBranch.role())
        .nowrap(),
    )
    .with_children((count > 0).then(|| {
      Text::new(&count.to_string())
        .id(format!("{id}-count"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
    }))
    .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)))
    .child(
      Row::new()
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(dot(mark))
        .child(
          Text::new(&how)
            .id(format!("{id}-lands"))
            .variant(ExtraTypography::Property.role())
            .nowrap(),
        ),
    )
}

/// How a lane's runs land: its dot's color and its words.
fn landing(ctx: &Ctx, lands: &LaneLanding) -> (PaletteColor, std::sync::Arc<str>) {
  match lands {
    LaneLanding::InCheckout => (PaletteColor::Success, ctx.t("delivery.queue.lane.inCheckout")),
    LaneLanding::Beside(Some(branch)) => (
      PaletteColor::Info,
      ctx.t_args("delivery.queue.lane.beside", [("branch", branch.as_str())]),
    ),
    LaneLanding::Beside(None) => (PaletteColor::Info, ctx.t("delivery.queue.lane.besideDetached")),
    LaneLanding::WaitsForCheckout(Some(branch)) => (
      PaletteColor::Warning,
      ctx.t_args("delivery.queue.lane.waits", [("branch", branch.as_str())]),
    ),
    LaneLanding::WaitsForCheckout(None) => (PaletteColor::Warning, ctx.t("delivery.queue.lane.waitsDetached")),
  }
}

const fn landing_name(lands: &LaneLanding) -> &'static str {
  match lands {
    LaneLanding::InCheckout => "in_checkout",
    LaneLanding::Beside(_) => "beside",
    LaneLanding::WaitsForCheckout(_) => "waits_for_checkout",
  }
}

/// The rows of a lane in their box (`Lane master` `N6C9Ma`): its runs in the queue numbered from 1, each waiting run
/// after the one before it, and those merged or out of it today after them.
fn lane_box(ctx: &mut Ctx, id: &str, rows: &Rows, shown: &[&QueueRow]) -> Column {
  let held = shown
    .iter()
    .any(|row| matches!(row.entry.state, QueueState::Blocked(_)));
  let mut place = 0;
  let mut after: Option<String> = None;
  let mut children = Vec::with_capacity(shown.len());
  for (index, queued) in shown.iter().enumerate() {
    let in_queue = queued.entry.state.in_queue();
    if in_queue {
      place += 1;
    }
    let key = queued.task_key.to_string();
    let placed = Placed {
      place: in_queue.then_some(place),
      after: after.as_deref(),
      held,
      last: index + 1 == shown.len(),
    };
    children.push(row::row(ctx, rows, queued, &placed));
    if in_queue {
      after = Some(key);
    }
  }
  Column::new()
    .id(format!("{id}-rows"))
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .with_children(children)
}

/// The note under the lanes: a run waiting for the person in one lane holds nothing in another, named when there is
/// one and another lane beside it.
pub(super) fn note(ctx: &Ctx, read: &QueueRead) -> Row {
  let waiting = read.lanes.iter().find_map(|lane| {
    read
      .lane_rows(lane)
      .find(|row| matches!(row.entry.state, QueueState::WaitingForApproval | QueueState::Blocked(_)))
      .map(|row| (lane, row))
  });
  let other = waiting.and_then(|(waiting, row)| {
    read
      .lanes
      .iter()
      .find(|lane| lane.base.branch != waiting.base.branch)
      .map(|lane| (row, lane))
  });
  let words = match other {
    Some((row, lane)) => ctx.t_args(
      "delivery.queue.lane.note",
      [
        ("key", row.task_key.to_string()),
        ("other", lane.base.branch.as_str().to_owned()),
      ],
    ),
    None => ctx.t("delivery.queue.lane.noteAny"),
  };
  Row::new()
    .id("merge-queue-note")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
