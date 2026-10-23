//! Where the run's delivery stands, at the top of the task's Summary. The state card (`State card` `IJVXv` in
//! `ulEuh`, `iEqGW` in `RRk9E`): a 17 glyph, *Delivering into main* with the run's place and since when at its end,
//! what happens now, the phases of its delivery (`Queue phase` `ED8Ct`), and *Open merge queue* and *Open
//! conversation*; 16 over and under, 18 at the sides, 12 apart, `surface_raised` in a 10 corner. *Merged into main*
//! (`lxSSL` in `lvupO`) and why the run left the queue are notices. *Review before merging* heads the review (`E2r4tN`
//! in `wN5Bz`): the run and how it delivers, and its merge line (`bwHYE`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  integration::MergeKind,
  merge_queue::{Blocked, Delivered, QueueEvent, QueueState},
  resource::DeliverMode,
};
use orchester_workspace::{
  display::{clock, short_revision},
  merge_queue::{RunDelivery, TaskDelivery},
  tasks::detail::RunView,
};

use super::{super::TaskContext, rows::since};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  screens::merge_queue::{phases, words},
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

/// The base branch the run delivers into.
pub(super) fn base(run: &RunView) -> &str {
  run.run.base_branch().map_or("", |branch| branch.as_str())
}

/// The state card of a run being delivered, waiting its turn, or joining the queue (`delivery` `None`).
pub(super) fn state_card(ctx: &mut Ctx, task: &TaskContext, (run, queue): (&RunView, &TaskDelivery)) -> Element {
  let base = base(run);
  let delivery = queue.delivery.as_ref();
  let integrator = queue.integrator.as_deref();
  let (title, body) = words_of(ctx, (base, integrator), (queue, delivery));
  let at_end = delivery.map(|delivery| place_line(ctx, delivery));
  let title_line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&title)
        .id("task-delivery-title")
        .variant(DeliveryText::CardTitle.role()),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(at_end.map(|line| {
      Text::new(&line)
        .id("task-delivery-place")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted)
        .nowrap()
    }));
  let blocked = matches!(
    delivery.map(|delivery| &delivery.entry.state),
    Some(QueueState::Blocked(_))
  );
  let glyph = if blocked {
    (Icon::CirclePause, PaletteColor::Warning)
  } else {
    (Icon::GitMerge, PaletteColor::TextSecondary)
  };
  let phases = delivery.and_then(|delivery| {
    phases::phases(
      ctx,
      "task-delivery",
      (&delivery.phases(&queue.deliver), phases::Named { base, integrator }),
    )
  });
  Column::new()
    .id("task-delivery")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .describe(
      "state",
      delivery.map_or("joining", |delivery| state_name(&delivery.entry.state)),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Start)
        .child(icon(glyph.0, IconSize::Lg, glyph.1))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .spacing(ExtraSpacing::Hair.role())
            .child(title_line)
            .child(
              Text::new(&body)
                .id("task-delivery-body")
                .variant(ExtraTypography::Prose.role())
                .color(PaletteColor::TextSecondary)
                .width(Dimension::Pct(100.0)),
            ),
        ),
    )
    .with_children(phases)
    .child(card_actions(ctx, task))
    .into()
}

/// The card's title and what happens now.
fn words_of(
  ctx: &Ctx,
  (base, integrator): (&str, Option<&str>),
  (queue, delivery): (&TaskDelivery, Option<&RunDelivery>),
) -> (Arc<str>, Arc<str>) {
  let auto = queue.deliver.mode() == DeliverMode::AutoMerge;
  let Some(delivery) = delivery else {
    return (
      ctx.t_args("delivery.task.joiningTitle", [("base", base)]),
      ctx.t_args("delivery.task.joiningBody", [("base", base)]),
    );
  };
  match &delivery.entry.state {
    QueueState::Queued => (
      ctx.t_args("delivery.task.queuedTitle", [("base", base)]),
      ctx.t("delivery.task.queuedBody"),
    ),
    QueueState::Integrating {
      base: revision, files, ..
    } => {
      let integrator = integrator.map_or_else(|| ctx.t("delivery.task.theIntegrator"), Arc::from);
      (
        ctx.t_args("delivery.task.integratingTitle", [("base", base)]),
        ctx.t_args(
          "delivery.task.integratingBody",
          [
            ("base", base.to_owned()),
            ("revision", short_revision(revision.as_str()).to_owned()),
            ("count", files.len().to_string()),
            ("integrator", integrator.to_string()),
          ],
        ),
      )
    }
    QueueState::Blocked(blocked) => (ctx.t("delivery.task.blockedTitle"), blocked_body(ctx, blocked, base)),
    _ if !queue.deliver.recheck() => (
      ctx.t_args("delivery.task.deliveringTitle", [("base", base)]),
      ctx.t_args("delivery.task.deliveringNoCheckBody", [("base", base)]),
    ),
    _ if auto => (
      ctx.t_args("delivery.task.deliveringTitle", [("base", base)]),
      ctx.t_args("delivery.task.deliveringAutoBody", [("base", base)]),
    ),
    _ => (
      ctx.t_args("delivery.task.deliveringTitle", [("base", base)]),
      ctx.t_args("delivery.task.deliveringBody", [("base", base)]),
    ),
  }
}

fn blocked_body(ctx: &Ctx, blocked: &Blocked, base: &str) -> Arc<str> {
  match blocked {
    Blocked::Uncommitted { .. } => ctx.t_args("delivery.task.blocked.uncommitted", [("base", base)]),
    Blocked::OtherBranch { .. } => ctx.t_args("delivery.task.blocked.otherBranch", [("base", base)]),
    Blocked::InProgress => ctx.t("delivery.task.blocked.inProgress"),
    Blocked::HeldLocally => ctx.t("delivery.task.blocked.held"),
  }
}

/// "1st in the merge queue · since 16:58".
fn place_line(ctx: &Ctx, delivery: &RunDelivery) -> Arc<str> {
  let since = since(ctx, delivery.entry.since);
  match delivery.place {
    Some(place) => ctx.t_args(
      "delivery.task.place",
      [
        ("place", words::place(ctx, place).to_string()),
        ("since", since.to_string()),
      ],
    ),
    None => since,
  }
}

/// *Open merge queue* and *Open conversation*.
fn card_actions(ctx: &mut Ctx, task: &TaskContext) -> Element {
  Row::new()
    .id("task-delivery-actions")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(open_queue(ctx, task, ButtonKind::Secondary))
    .child(
      ActionButton::new(
        "task-delivery-open-conversation",
        ButtonKind::Ghost,
        ctx.t("delivery.task.openConversation"),
      )
      .icon(Icon::MessageSquareText)
      .opens(Navigation::of(ctx).to(Route::Project(
        task.project.open.project.id(),
        task.page(TaskView::of(TaskTab::Conversation)),
      ))),
    )
    .into()
}

/// *Open merge queue*.
pub(super) fn open_queue(ctx: &mut Ctx, task: &TaskContext, kind: ButtonKind) -> ActionButton {
  ActionButton::new("task-delivery-open-queue", kind, ctx.t("delivery.task.openQueue"))
    .icon(Icon::GitMerge)
    .opens(Navigation::of(ctx).to(Route::Project(task.project.open.project.id(), ProjectPage::MergeQueue)))
}

/// *Merged into main · 9c41e02 · worktree removed* and how it was merged and cleared.
pub(super) fn merged(ctx: &Ctx, run: &RunView, (delivery, delivered): (&RunDelivery, &Delivered)) -> Element {
  let base = base(run);
  let revision = short_revision(delivered.revision.as_str()).to_owned();
  let title_key = if delivered.worktree_removed {
    "delivery.task.merged.titleRemoved"
  } else {
    "delivery.task.merged.title"
  };
  let title = ctx.t_args(title_key, [("base", base.to_owned()), ("revision", revision)]);
  let checked = delivery
    .events
    .iter()
    .any(|recorded| matches!(recorded.event, QueueEvent::CheckPassed { .. }));
  let how_key = match (delivered.kind, checked) {
    (MergeKind::Squash, true) => "delivery.task.merged.squashChecked",
    (MergeKind::Squash, false) => "delivery.task.merged.squash",
    (_, true) => "delivery.task.merged.mergeCommitChecked",
    (_, false) => "delivery.task.merged.mergeCommit",
  };
  let how = ctx.t_args(how_key, [("time", clock(delivery.entry.since))]);
  let cleared = match (delivered.worktree_removed, delivered.branch_deleted) {
    (true, true) => ctx.t_args("delivery.task.merged.cleared", [("branch", run.run.branch().as_str())]),
    (true, false) => ctx.t_args(
      "delivery.task.merged.clearedKept",
      [("branch", run.run.branch().as_str())],
    ),
    (false, _) => ctx.t("delivery.task.merged.notCleared"),
  };
  Notice::new("task-notice", NoticeTone::Success, title)
    .icon(Icon::GitMerge)
    .body(ctx.t_args("decisions.ready.sentences", [("first", &*how), ("second", &*cleared)]))
    .into()
}

/// *Review before merging* with the run and how it delivers, and its merge line: the run's branch into the base, when
/// it was synced and how large the change is (`size`).
pub(super) fn review_head(ctx: &Ctx, (run, queue): (&RunView, &TaskDelivery), size: Option<Arc<str>>) -> Element {
  let base = base(run);
  let pipeline = ctx.t_args(
    "delivery.queue.empty.pipeline",
    [
      ("name", run.pipeline.to_string()),
      ("version", run.version.get().to_string()),
    ],
  );
  let meta = ctx.t_args(
    "delivery.task.review.meta",
    [
      ("run", run.run.number().get().to_string()),
      ("pipeline", pipeline.to_string()),
      ("mode", words::mode(ctx, queue.deliver.mode()).to_string()),
    ],
  );
  let synced = queue.delivery.as_ref().and_then(|delivery| {
    delivery.events.iter().rev().find_map(|recorded| match &recorded.event {
      QueueEvent::Synced { base: revision, .. } => Some(ctx.t_args(
        "delivery.task.review.synced",
        [
          ("base", base.to_owned()),
          ("revision", short_revision(revision.as_str()).to_owned()),
          ("time", clock(recorded.at)),
        ],
      )),
      _ => None,
    })
  });
  let line = Row::new()
    .id("task-review-merge-line")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(MERGE_LINE_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(icon(Icon::GitBranch, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(run.run.branch().as_str())
        .variant(DeliveryText::Branch.role())
        .nowrap(),
    )
    .child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(base).variant(DeliveryText::Branch.role()).nowrap())
    .with_children(synced.map(|synced| {
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(separator())
        .child(Text::new(&synced).variant(ExtraTypography::MonoNote.role()).nowrap())
    }))
    .child(Spacer::new().flex(1.0))
    .with_children(size.map(|size| Text::new(&size).variant(ExtraTypography::MonoNote.role()).nowrap()));
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("delivery.task.review.title")).variant(TaskText::SectionTitle.role()))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&meta)
        .id("task-review-meta")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  Column::new()
    .id("task-review")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(head)
    .child(line)
    .into()
}

/// The merge line's height (`bwHYE`).
const MERGE_LINE_HEIGHT: f32 = 40.0;
/// A separator's height (`rXZ5I`).
const SEPARATOR_HEIGHT: f32 = 14.0;
const SEPARATOR_WIDTH: f32 = 1.0;

fn separator() -> Element {
  Column::new()
    .width(Dimension::Px(SEPARATOR_WIDTH))
    .height(Dimension::Px(SEPARATOR_HEIGHT))
    .background(PaletteColor::Border)
    .into()
}

const fn state_name(state: &QueueState) -> &'static str {
  match state {
    QueueState::Queued => "queued",
    QueueState::Syncing => "syncing",
    QueueState::Rechecking { .. } => "rechecking",
    QueueState::Integrating { .. } => "integrating",
    QueueState::WaitingForApproval => "waiting_for_approval",
    QueueState::Blocked(_) => "blocked",
    QueueState::Left(_) => "left",
    QueueState::Merged(_) => "merged",
    QueueState::Removed => "removed",
  }
}
