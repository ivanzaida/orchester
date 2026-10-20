//! How the Memory screens say a record: its type's glyph and words, its status, its author and where it was
//! written (the meta line), and when.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  memory::{Confidence, Importance, Memory, MemoryKind, MemoryScope, MemoryStatus},
  time::Timestamp,
};
use orchester_workspace::{
  display,
  memory::{
    handoff::HandoffList,
    listing::{AuthorFilter, StatusFilter},
    origin::{Origins, Workplace},
  },
};

use crate::ui::icon::Icon;

/// The glyph of a record's type (`W0R4Mf` in `J1utD`).
pub const fn glyph(kind: MemoryKind) -> Icon {
  match kind {
    MemoryKind::Handoff => Icon::ArrowRightLeft,
    MemoryKind::Decision => Icon::Signpost,
    MemoryKind::Warning => Icon::TriangleAlert,
    MemoryKind::Fact => Icon::Info,
    MemoryKind::Todo => Icon::ListTodo,
    MemoryKind::Summary => Icon::ScrollText,
    MemoryKind::Observation => Icon::Eye,
    MemoryKind::Artifact => Icon::FileText,
  }
}

/// A record's type as a word: *Decision*.
pub fn kind_name(ctx: &Ctx, kind: MemoryKind) -> Arc<str> {
  ctx.t(match kind {
    MemoryKind::Fact => "memory.kind.fact",
    MemoryKind::Decision => "memory.kind.decision",
    MemoryKind::Observation => "memory.kind.observation",
    MemoryKind::Artifact => "memory.kind.artifact",
    MemoryKind::Handoff => "memory.kind.handoff",
    MemoryKind::Todo => "memory.kind.todo",
    MemoryKind::Warning => "memory.kind.warning",
    MemoryKind::Summary => "memory.kind.summary",
  })
}

/// A record's type in the middle of a sentence: *Edit decision*.
pub fn kind_lower(ctx: &Ctx, kind: MemoryKind) -> Arc<str> {
  ctx.t(match kind {
    MemoryKind::Fact => "memory.kindLower.fact",
    MemoryKind::Decision => "memory.kindLower.decision",
    MemoryKind::Observation => "memory.kindLower.observation",
    MemoryKind::Artifact => "memory.kindLower.artifact",
    MemoryKind::Handoff => "memory.kindLower.handoff",
    MemoryKind::Todo => "memory.kindLower.todo",
    MemoryKind::Warning => "memory.kindLower.warning",
    MemoryKind::Summary => "memory.kindLower.summary",
  })
}

/// A group of records of one type, as its heading and the *Type* filter say it: *Decisions*.
pub fn kind_plural(ctx: &Ctx, kind: MemoryKind) -> Arc<str> {
  ctx.t(match kind {
    MemoryKind::Fact => "memory.kindPlural.fact",
    MemoryKind::Decision => "memory.kindPlural.decision",
    MemoryKind::Observation => "memory.kindPlural.observation",
    MemoryKind::Artifact => "memory.kindPlural.artifact",
    MemoryKind::Handoff => "memory.kindPlural.handoff",
    MemoryKind::Todo => "memory.kindPlural.todo",
    MemoryKind::Warning => "memory.kindPlural.warning",
    MemoryKind::Summary => "memory.kindPlural.summary",
  })
}

pub fn importance(ctx: &Ctx, importance: Importance) -> Arc<str> {
  ctx.t(match importance {
    Importance::Low => "memory.importance.low",
    Importance::Normal => "memory.importance.normal",
    Importance::High => "memory.importance.high",
  })
}

pub fn confidence(ctx: &Ctx, confidence: Confidence) -> Arc<str> {
  ctx.t(match confidence {
    Confidence::Low => "memory.confidence.low",
    Confidence::Medium => "memory.confidence.medium",
    Confidence::High => "memory.confidence.high",
  })
}

/// The agent's confidence at the start of a fact: *High · set by Builder*.
pub fn confidence_title(ctx: &Ctx, confidence: Confidence) -> Arc<str> {
  ctx.t(match confidence {
    Confidence::Low => "memory.confidenceTitle.low",
    Confidence::Medium => "memory.confidenceTitle.medium",
    Confidence::High => "memory.confidenceTitle.high",
  })
}

pub fn status_filter(ctx: &Ctx, status: StatusFilter) -> Arc<str> {
  ctx.t(match status {
    StatusFilter::ActiveAndWaiting => "memory.filters.status.activeAndWaiting",
    StatusFilter::Active => "memory.filters.status.active",
    StatusFilter::Waiting => "memory.filters.status.waiting",
    StatusFilter::Deprecated => "memory.filters.status.deprecated",
    StatusFilter::Superseded => "memory.filters.status.superseded",
    StatusFilter::Rejected => "memory.filters.status.rejected",
    StatusFilter::Withdrawn => "memory.filters.status.withdrawn",
    StatusFilter::All => "memory.filters.status.all",
  })
}

pub fn author_filter(ctx: &Ctx, author: AuthorFilter) -> Arc<str> {
  ctx.t(match author {
    AuthorFilter::Any => "memory.filters.author.any",
    AuthorFilter::Agents => "memory.filters.author.agents",
    AuthorFilter::People => "memory.filters.author.people",
  })
}

pub fn handoff_list(ctx: &Ctx, list: HandoffList) -> Arc<str> {
  ctx.t(match list {
    HandoffList::Completed => "memory.handoff.completed",
    HandoffList::Decisions => "memory.handoff.decisions",
    HandoffList::OpenQuestions => "memory.handoff.openQuestions",
    HandoffList::NextAgentShouldKnow => "memory.handoff.nextAgent",
  })
}

/// The status label of a record no agent is given, with its glyph (`b2J5Cr` in `J1utD`); `None` for an active or
/// a waiting record.
pub fn closed_status(ctx: &Ctx, status: MemoryStatus) -> Option<(Icon, Arc<str>)> {
  let (glyph, key) = match status {
    MemoryStatus::Proposed | MemoryStatus::Active => return None,
    MemoryStatus::Superseded => (Icon::Replace, "memory.status.superseded"),
    MemoryStatus::Deprecated => (Icon::Archive, "memory.status.deprecated"),
    MemoryStatus::Rejected => (Icon::CircleX, "memory.status.rejected"),
    MemoryStatus::Withdrawn => (Icon::Undo2, "memory.status.withdrawn"),
  };
  Some((glyph, ctx.t(key)))
}

/// When `at` was, as a meta line says it: the clock time today (*16:12*), else the day (*Oct 7*).
pub fn when(ctx: &Ctx, at: Timestamp) -> String {
  if at.local_day_start() == Timestamp::now().local_day_start() {
    return display::clock(at);
  }
  let (month, day) = display::month_day(at);
  let month = ctx.t_ns(orchester_workspace::i18n::NAMESPACE, display::MONTH_KEYS[month]);
  ctx
    .t_args("memory.date", [("month", month.to_string()), ("day", day.to_string())])
    .to_string()
}

/// Where an agent worked, as parts of a meta line: its role, its task when `with_task`, the run, the step and the
/// attempt when its run had more than one.
pub fn workplace(ctx: &Ctx, place: &Workplace, with_task: bool) -> Vec<String> {
  let mut parts = Vec::new();
  parts.extend(place.agent.as_deref().map(str::to_owned));
  if with_task {
    parts.extend(place.task.as_ref().map(ToString::to_string));
  }
  if let Some(run) = place.run {
    parts.push(
      ctx
        .t_args("memory.meta.run", [("run", run.get().to_string())])
        .to_string(),
    );
  }
  if let Some((number, name)) = &place.step {
    parts.push(step(ctx, *number, name).to_string());
  }
  if let Some(attempt) = place.attempt {
    parts.push(
      ctx
        .t_args("memory.meta.attempt", [("attempt", attempt.get().to_string())])
        .to_string(),
    );
  }
  parts
}

/// A step as the screens name it: *03 Implement*.
pub fn step(ctx: &Ctx, number: usize, name: &str) -> Arc<str> {
  ctx.t_args(
    "memory.step",
    [("number", format!("{number:02}").as_str()), ("step", name)],
  )
}

/// Who wrote `memory` and where, when, and what happened to it since: the record's meta line (`K7JsB` in `J1utD`).
/// `with_task` names the task an agent worked on, as the project's page does.
pub fn meta(ctx: &Ctx, memory: &Memory, origins: &Origins, with_task: bool) -> Arc<str> {
  let mut parts = match origins.written(memory) {
    Some(place) => workplace(ctx, &place, with_task),
    None => vec![ctx.t("memory.meta.you").to_string()],
  };
  if with_task
    && origins.written(memory).is_none()
    && let MemoryScope::Task(task) = memory.scope()
  {
    parts.extend(origins.task_key(task).map(ToString::to_string));
  }
  parts.push(when(ctx, memory.created_at()));
  parts.extend(change(ctx, memory));
  join(ctx, &parts)
}

/// What happened to `memory` after it was written, for its meta line.
fn change(ctx: &Ctx, memory: &Memory) -> Vec<String> {
  let since = when(ctx, memory.updated_at());
  let key = match memory.status() {
    MemoryStatus::Superseded => Some("memory.meta.superseded"),
    MemoryStatus::Deprecated => Some("memory.meta.deprecated"),
    MemoryStatus::Rejected => Some("memory.meta.rejected"),
    MemoryStatus::Withdrawn => {
      return vec![ctx.t("memory.meta.withdrawn").to_string()];
    }
    MemoryStatus::Proposed | MemoryStatus::Active => None,
  };
  let mut parts = Vec::new();
  if let Some(edited) = memory.edited_at() {
    parts.push(
      ctx
        .t_args("memory.meta.edited", [("time", when(ctx, edited))])
        .to_string(),
    );
  }
  if let Some(key) = key {
    parts.push(ctx.t_args(key, [("time", since)]).to_string());
  }
  parts
}

/// A proposal's meta line: who proposed it, where and when, and the agent's confidence (`PpO9n` in `LdHs2`).
pub fn proposal_meta(ctx: &Ctx, memory: &Memory, origins: &Origins, with_task: bool) -> Arc<str> {
  let mut parts = origins
    .written(memory)
    .map(|place| workplace(ctx, &place, with_task))
    .unwrap_or_default();
  parts.push(when(ctx, memory.created_at()));
  parts.push(
    ctx
      .t_args(
        "memory.meta.confidence",
        [("confidence", confidence(ctx, memory.confidence()).to_string())],
      )
      .to_string(),
  );
  join(ctx, &parts)
}

/// Whose memory a record is: *Project*, or *Task KON-12*.
pub fn scope(ctx: &Ctx, memory: &Memory, origins: &Origins) -> Arc<str> {
  match memory.scope() {
    MemoryScope::Project => ctx.t("memory.scope.project"),
    MemoryScope::Task(task) => ctx.t_args(
      "memory.scope.task",
      [(
        "task",
        origins.task_key(task).map(ToString::to_string).unwrap_or_default(),
      )],
    ),
  }
}

/// `parts` as one line, separated as the screens separate a meta line's parts.
pub fn join(ctx: &Ctx, parts: &[String]) -> Arc<str> {
  Arc::from(parts.join(&ctx.t("memory.separator")))
}

/// A count of tokens as a panel says it: *1.2k* from a thousand on.
pub fn tokens(ctx: &Ctx, tokens: u32) -> Arc<str> {
  if tokens < 1_000 {
    return Arc::from(tokens.to_string());
  }
  let tenths = (tokens + 50) / 100;
  let value = if tenths.is_multiple_of(10) {
    (tenths / 10).to_string()
  } else {
    format!("{}.{}", tenths / 10, tenths % 10)
  };
  ctx.t_args("memory.thousands", [("value", value)])
}
