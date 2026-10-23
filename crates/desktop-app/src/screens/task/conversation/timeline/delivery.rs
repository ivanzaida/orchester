//! What the merge queue did with the run, in the chat (`Task · Conversation · Deliver and Integrate` `Urfxj`,
//! `orchester_workspace::tasks::timeline::delivery`): event dividers for joining the queue (*Open merge queue*), its
//! conflicts going to the Integrate step, a check the base superseded, the person's approval, a block, the merge and
//! leaving the queue; the Deliver step's own dividers (*07 Deliver · sync with main*, *07 Deliver · Check on the merged
//! result*) over tool rows for the sync and the check (`Tool row` `uFWI7`, 20 in); what the clean-up removed as tool
//! rows; and, while the run waits for the person's approval, that it does with *Review & merge*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  integration::MergeKind,
  merge_queue::{Blocked, QueueEvent},
  time::Timestamp,
};
use orchester_workspace::{
  display::{clock, short_revision},
  tasks::timeline::delivery::{DeliveryKind, DeliveryLine},
};

use super::{
  Timeline,
  divider::{event, phase, step_label},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  theme::{ConversationSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
  },
};

/// A tool row's height (`uFWI7`).
const ROW_HEIGHT: f32 = 26.0;

/// The names a line's words take: the base branch, the run's branch, the Deliver step's label and the integrator.
struct Names {
  base: Arc<str>,
  branch: Arc<str>,
  deliver: Arc<str>,
  integrator: Arc<str>,
}

impl Names {
  fn of(ctx: &Ctx, timeline: &Timeline) -> Self {
    let run = timeline.detail.run.as_ref();
    let steps = run.map_or(0, |run| run.steps.len());
    Self {
      base: run
        .and_then(|run| run.run.base_branch())
        .map_or_else(|| ctx.t("delivery.conversation.theBase"), |base| base.as_str().into()),
      branch: run.map_or_else(|| "".into(), |run| run.run.branch().as_str().into()),
      deliver: step_label(ctx, steps + 1, &ctx.t("delivery.task.stage.deliver")),
      integrator: timeline
        .detail
        .queue
        .as_ref()
        .and_then(|queue| queue.integrator.as_deref())
        .map_or_else(|| ctx.t("delivery.task.theIntegrator"), Arc::from),
    }
  }
}

/// `line`'s row in the chat.
pub(super) fn line(ctx: &mut Ctx, timeline: &Timeline, line: &DeliveryLine) -> Element {
  let names = Names::of(ctx, timeline);
  let id = format!("conversation-{}", line.key);
  let time = clock(line.at);
  match &line.kind {
    DeliveryKind::WaitsForApproval => {
      let text = ctx.t_args(
        "delivery.conversation.waitsForApproval",
        [("base", &*names.base), ("time", time.as_str())],
      );
      let review = timeline.open(ctx, TaskView::Summary);
      let link = (ctx.t("delivery.queue.action.reviewAndMerge"), Press::Open(review));
      event(id, Icon::Hand, &text, Some(link))
    }
    DeliveryKind::Event(queued) => queue_event(ctx, timeline, (&id, line.at), &names, queued),
  }
}

/// What a line says, with the base branch and its time among its words.
struct Said<'a> {
  names: &'a Names,
  time: String,
}

impl Said<'_> {
  fn say(&self, ctx: &Ctx, key: &str, args: &[(&str, &str)]) -> Arc<str> {
    let mut all: Vec<(&str, &str)> = vec![("base", &*self.names.base), ("time", self.time.as_str())];
    all.extend_from_slice(args);
    ctx.t_args(key, all)
  }
}

fn queue_event(
  ctx: &mut Ctx,
  timeline: &Timeline,
  (id, at): (&str, Timestamp),
  names: &Names,
  queued: &QueueEvent,
) -> Element {
  let said = Said { names, time: clock(at) };
  let id = id.to_owned();
  let (glyph, text) = match queued {
    QueueEvent::Joined => {
      let queue = Navigation::of(ctx).to(Route::Project(timeline.project, ProjectPage::MergeQueue));
      let link = (ctx.t("delivery.task.openQueue"), Press::Open(queue));
      let text = said.say(ctx, "delivery.conversation.joined", &[]);
      return event(id, Icon::ListOrdered, &text, Some(link));
    }
    QueueEvent::Approved { .. } => (Icon::ShieldCheck, said.say(ctx, "delivery.conversation.approved", &[])),
    QueueEvent::Blocked(blocked) => {
      let key = match blocked {
        Blocked::Uncommitted { .. } => "delivery.conversation.blocked.uncommitted",
        Blocked::OtherBranch { .. } => "delivery.conversation.blocked.otherBranch",
        Blocked::InProgress => "delivery.conversation.blocked.inProgress",
        Blocked::HeldLocally => "delivery.conversation.blocked.held",
      };
      (Icon::TriangleAlert, said.say(ctx, key, &[]))
    }
    QueueEvent::Merged { revision, kind } => {
      let key = if *kind == MergeKind::Squash {
        "delivery.conversation.squashed"
      } else {
        "delivery.conversation.merged"
      };
      let revision = short_revision(revision.as_str());
      (Icon::GitMerge, said.say(ctx, key, &[("revision", revision)]))
    }
    QueueEvent::CleanedUp {
      worktree_removed,
      branch_deleted,
    } => return cleaned(ctx, &id, (*worktree_removed, *branch_deleted), names, &said.time),
    QueueEvent::Left(left) => {
      let reason = crate::screens::merge_queue::words::left_reason(ctx, left);
      let text = said.say(ctx, "delivery.conversation.left", &[("reason", &*reason)]);
      (Icon::CircleX, text)
    }
    QueueEvent::Removed => (Icon::X, said.say(ctx, "delivery.conversation.removed", &[])),
    QueueEvent::Requeued => (Icon::ListOrdered, said.say(ctx, "delivery.conversation.requeued", &[])),
    _ => return delivering(ctx, &id, &said, queued),
  };
  event(id, glyph, &text, None)
}

/// A line of the delivery itself: the sync, its conflicts going to Integrate, and Check on the merged result.
fn delivering(ctx: &Ctx, id: &str, said: &Said<'_>, queued: &QueueEvent) -> Element {
  let names = said.names;
  match queued {
    QueueEvent::Synced { base, files } => {
      let args = [("revision", short_revision(base.as_str())), ("branch", &*names.branch)];
      let title = said.say(ctx, "delivery.conversation.syncCommand", &args);
      let (result, tone) = if files.is_empty() {
        (ctx.t("delivery.conversation.clean"), PaletteColor::TextSecondary)
      } else {
        let count = files.len().to_string();
        (
          ctx.t_args("delivery.conversation.conflicts", [("count", count.as_str())]),
          PaletteColor::Warning,
        )
      };
      let divider = said.say(ctx, "delivery.conversation.syncPhase", &[("step", &*names.deliver)]);
      let row = tool(id, (Icon::GitMerge, &title), (&result, tone), &said.time);
      phased(id, &divider, row)
    }
    QueueEvent::IntegrateStarted { files, .. } => {
      let count = files.len().to_string();
      let args = [("count", count.as_str()), ("integrator", &*names.integrator)];
      let text = said.say(ctx, "delivery.conversation.integrate", &args);
      event(id.to_owned(), Icon::GitCompareArrows, &text, None)
    }
    QueueEvent::CheckStarted { base } => {
      let args = [("step", &*names.deliver), ("revision", short_revision(base.as_str()))];
      phase(id.to_owned(), &said.say(ctx, "delivery.conversation.checkPhase", &args))
    }
    QueueEvent::CheckPassed { base, tool: name } | QueueEvent::CheckFailed { base, tool: name } => {
      let (key, tone) = if matches!(queued, QueueEvent::CheckPassed { .. }) {
        ("delivery.conversation.checkPassed", PaletteColor::Success)
      } else {
        ("delivery.conversation.checkFailed", PaletteColor::Danger)
      };
      let result = said.say(ctx, key, &[("revision", short_revision(base.as_str()))]);
      tool(id, (Icon::Terminal, name), (&result, tone), &said.time)
    }
    QueueEvent::CheckSuperseded { now, .. } => {
      let args = [("revision", short_revision(now.as_str()))];
      let text = said.say(ctx, "delivery.conversation.superseded", &args);
      event(id.to_owned(), Icon::RefreshCw, &text, None)
    }
    // The queue's other events are lines of their own (`queue_event`).
    _ => Column::new().into(),
  }
}

/// What the clean-up removed, a tool row each: the worktree, the code index that went with it, the branch.
fn cleaned(ctx: &Ctx, id: &str, (worktree, branch): (bool, bool), names: &Names, time: &str) -> Element {
  let done = ctx.t("delivery.conversation.done");
  let mut rows = Vec::new();
  if worktree {
    let worktree_title = ctx.t("delivery.conversation.removedWorktree");
    let index_title = ctx.t("delivery.conversation.removedIndex");
    rows.push(tool(
      &format!("{id}-worktree"),
      (Icon::FolderX, &worktree_title),
      (&done, PaletteColor::TextMuted),
      time,
    ));
    rows.push(tool(
      &format!("{id}-index"),
      (Icon::FolderX, &index_title),
      (&done, PaletteColor::TextMuted),
      time,
    ));
  }
  if branch {
    let title = ctx.t_args("delivery.conversation.deletedBranch", [("branch", &*names.branch)]);
    rows.push(tool(
      &format!("{id}-branch"),
      (Icon::GitBranch, &title),
      (&done, PaletteColor::TextMuted),
      time,
    ));
  }
  if rows.is_empty() {
    let text = ctx.t_args("delivery.conversation.notCleared", [("time", time)]);
    return event(id.to_owned(), Icon::FolderOpen, &text, None);
  }
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .with_children(rows)
    .into()
}

/// The Deliver step's `divider` over `under`.
fn phased(id: &str, divider: &str, under: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(phase(format!("{id}-phase"), divider))
    .child(under)
    .into()
}

/// A tool row 20 in: its glyph, what ran, how it ended in `tone` and its time (`uFWI7`).
fn tool(id: &str, (glyph, title): (Icon, &str), (result, tone): (&str, PaletteColor), time: &str) -> Element {
  Row::new()
    .id(format!("{id}-tool"))
    .width(Dimension::Pct(100.0))
    .padding_left(ConversationSpacing::ToolRows.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(ROW_HEIGHT))
        .padding_horizontal(SpacingSize::Sm)
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(TypographyStyle::Mono)
            .color(PaletteColor::TextSecondary)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .min_width(Dimension::Px(0.0))
            .flex(1.0),
        )
        .child(
          Text::new(result)
            .id(format!("{id}-result"))
            .variant(TypographyStyle::Caption)
            .color(tone)
            .nowrap(),
        )
        .child(Text::new(time).variant(ExtraTypography::MonoNote.role()).nowrap()),
    )
    .into()
}
