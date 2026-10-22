//! What the feedback parts say (`seN1p`): a thread's state as its icon, color and word (`State status` in
//! `ZtcOK`), its kind, where its lines are, why Orchester holds a request, when the agent reads it, what happened to
//! it, and the run and budget lines. Every word is a key of `pull_requests.feedback.*`; what a thread is about is
//! the workspace's ([`orchester_workspace::pull_requests::feedback`]).

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  connection::AgentProvider,
  pull_request::feedback::{DiffSide, FeedbackAnchor, FeedbackKind, LineAnchor, LineSpan, Located},
  revision::Revision,
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_workspace::{
  display::{clock, short_revision, waited},
  pull_requests::feedback::{BoardTask, Budget, Hold, NotDelivered, ThreadEvent, ThreadState, ThreadView},
  tasks::conversation::{PanelState, Queue},
};

use crate::ui::{icon::Icon, message::message_text};

/// A state's icon, color and word.
pub struct Look {
  pub icon: Icon,
  pub color: PaletteColor,
  pub word: Arc<str>,
}

/// How a thread's state is drawn: `text_secondary` icon and `text_primary` word, Ready to verify both in `warning`
/// (`xyIJD`); an outdated thread reads Outdated whatever its conversation says (`chva5`).
pub fn look(ctx: &Ctx, thread: &ThreadView) -> Look {
  if matches!(thread.at_head, Some(Located::Outdated)) && thread.state.is_open() {
    return Look {
      icon: Icon::History,
      color: PaletteColor::TextSecondary,
      word: ctx.t("pull_requests.feedback.state.outdated"),
    };
  }
  state_look(ctx, &thread.state)
}

pub fn state_look(ctx: &Ctx, state: &ThreadState) -> Look {
  let (icon, key) = match state {
    ThreadState::Queued(Hold::Closed(_)) => (Icon::CircleSlash, "runEnded"),
    ThreadState::Queued(_) => (Icon::Hourglass, "queued"),
    ThreadState::Delivered(_) => (Icon::Send, "delivered"),
    ThreadState::Working { .. } => (Icon::LoaderCircle, "working"),
    ThreadState::Answered { .. } => (Icon::MessageSquareReply, "answered"),
    ThreadState::ReadyToVerify { .. } => (Icon::Eye, "readyToVerify"),
    ThreadState::NotDelivered { .. } => (Icon::TriangleAlert, "notDelivered"),
    ThreadState::Resolved { .. } => (Icon::CircleCheck, "resolved"),
    ThreadState::Withdrawn { .. } => (Icon::CircleSlash, "withdrawn"),
  };
  let color = match state {
    ThreadState::ReadyToVerify { .. } => PaletteColor::Warning,
    _ => PaletteColor::TextSecondary,
  };
  Look {
    icon,
    color,
    word: ctx.t(&format!("pull_requests.feedback.state.{key}")),
  }
}

/// The word's own color: `warning` with its icon for Ready to verify, `text_primary` otherwise.
pub fn word_color(look: &Look) -> PaletteColor {
  if look.color == PaletteColor::Warning {
    PaletteColor::Warning
  } else {
    PaletteColor::TextPrimary
  }
}

pub const fn kind_icon(kind: FeedbackKind) -> Icon {
  match kind {
    FeedbackKind::Question => Icon::MessageCircleQuestion,
    FeedbackKind::Fix => Icon::Wrench,
  }
}

pub fn kind_word(ctx: &Ctx, kind: FeedbackKind) -> Arc<str> {
  match kind {
    FeedbackKind::Question => ctx.t("pull_requests.feedback.kind.question"),
    FeedbackKind::Fix => ctx.t("pull_requests.feedback.kind.fix"),
  }
}

/// `lines 43–45`, `line 40`; with the side's word (`new lines 43–45`) where `side` says so.
pub fn lines(ctx: &Ctx, (side, span): (Option<DiffSide>, LineSpan)) -> Arc<str> {
  let prefix = match side {
    None => "",
    Some(DiffSide::New) => "new",
    Some(DiffSide::Old) => "old",
  };
  let key = |one: &str, many: &str| {
    let word = if span.len() == 1 { one } else { many };
    if prefix.is_empty() {
      format!("pull_requests.feedback.anchor.{}", word.to_lowercase())
    } else {
      format!("pull_requests.feedback.anchor.{prefix}{word}")
    }
  };
  if span.len() == 1 {
    ctx.t_args(&key("Line", "Lines"), [("line", span.start().to_string())])
  } else {
    ctx.t_args(
      &key("Line", "Lines"),
      [("start", span.start().to_string()), ("end", span.end().to_string())],
    )
  }
}

/// Where a thread is, as its row says it: `lines 43–45 · 7c1e9f2`, `lines 9–14 → 9–16 · 7c1e9f2`, `no line ·
/// 7c1e9f2`, or its old place and the head it is gone at.
pub fn row_anchor(ctx: &Ctx, thread: &ThreadView, head: Option<&Revision>) -> Arc<str> {
  let revision = short_revision(thread.revision.as_str()).to_owned();
  let Some(anchor) = thread.anchor.lines() else {
    return ctx.t_args("pull_requests.feedback.anchor.whole", [("revision", revision)]);
  };
  let at = lines(ctx, (side_word(anchor), anchor.span()));
  match (thread.at_head, head) {
    (Some(Located::At(moved)), _) if moved != anchor.span() => ctx.t_args(
      "pull_requests.feedback.anchor.moved",
      [
        ("lines", at.to_string()),
        ("moved", span_text(moved)),
        ("revision", revision),
      ],
    ),
    (Some(Located::Outdated), Some(head)) => ctx.t_args(
      "pull_requests.feedback.anchor.outdated",
      [
        ("lines", at.to_string()),
        ("revision", revision),
        ("head", short_revision(head.as_str()).to_owned()),
      ],
    ),
    _ => ctx.t_args(
      "pull_requests.feedback.anchor.at",
      [("lines", at.to_string()), ("revision", revision)],
    ),
  }
}

/// Where a thread is, as its card's head says it: with the side, its move to the head, the agent and the task.
pub fn card_anchor(ctx: &Ctx, thread: &ThreadView, (head, task): (Option<&Revision>, &str)) -> Arc<str> {
  let revision = short_revision(thread.revision.as_str()).to_owned();
  let anchor = match (thread.anchor.lines(), thread.at_head, head) {
    (None, ..) => ctx.t_args("pull_requests.feedback.anchor.whole", [("revision", revision)]),
    (Some(lines), Some(Located::At(moved)), Some(head)) if moved != lines.span() => ctx.t_args(
      "pull_requests.feedback.anchor.movedAt",
      [
        ("lines", self::lines(ctx, (None, lines.span())).to_string()),
        ("revision", revision),
        ("moved", span_text(moved)),
        ("head", short_revision(head.as_str()).to_owned()),
      ],
    ),
    (Some(lines), Some(Located::Outdated), Some(head)) => ctx.t_args(
      "pull_requests.feedback.anchor.outdated",
      [
        ("lines", self::lines(ctx, (None, lines.span())).to_string()),
        ("revision", revision),
        ("head", short_revision(head.as_str()).to_owned()),
      ],
    ),
    (Some(lines), ..) => ctx.t_args(
      "pull_requests.feedback.anchor.at",
      [
        (
          "lines",
          self::lines(ctx, (Some(lines.side()), lines.span())).to_string(),
        ),
        ("revision", revision),
      ],
    ),
  };
  ctx.t_args(
    "pull_requests.feedback.anchor.to",
    [
      ("anchor", anchor.to_string()),
      ("agent", thread.recipient.to_string()),
      ("task", task.to_owned()),
    ],
  )
}

/// `sender.rs 43–45`, `whole PR`: a thread in the THREADS list, where its lines are now.
pub fn short_anchor(ctx: &Ctx, thread: &ThreadView) -> Arc<str> {
  let Some(lines) = thread.anchor.lines() else {
    return ctx.t("pull_requests.feedback.anchor.wholeShort");
  };
  let span = match thread.at_head {
    Some(Located::At(moved)) => moved,
    _ => lines.span(),
  };
  let file = lines.path().rsplit('/').next().unwrap_or(lines.path()).to_owned();
  ctx.t_args(
    "pull_requests.feedback.anchor.short",
    [("file", file), ("lines", span_text(span))],
  )
}

/// `43–45`, `40`.
pub fn span_text(span: LineSpan) -> String {
  if span.len() == 1 {
    span.start().to_string()
  } else {
    format!("{}–{}", span.start(), span.end())
  }
}

/// The side a row names: only the old side's lines say so.
fn side_word(anchor: &LineAnchor) -> Option<DiffSide> {
  (anchor.side() == DiffSide::Old).then_some(DiffSide::Old)
}

/// Why Orchester holds a request.
pub fn hold(ctx: &Ctx, hold: &Hold, (task, agent): (&str, &str), board: &[ThreadView]) -> Arc<str> {
  let task_only = |key: &str| ctx.t_args(key, [("task", task)]);
  match hold {
    Hold::AfterFix(thread) => {
      let anchor = board.iter().find(|other| other.id == *thread).map_or_else(
        || ctx.t("pull_requests.feedback.anchor.wholeShort"),
        |other| short_anchor(ctx, other),
      );
      ctx.t_args("pull_requests.feedback.hold.afterFix", [("anchor", anchor)])
    }
    Hold::NoSlot { working, slots } => ctx.t_args(
      "pull_requests.feedback.hold.noSlot",
      [
        ("slots", slots.to_string()),
        ("working", working.to_string()),
        ("agent", agent.to_owned()),
      ],
    ),
    Hold::Limit => task_only("pull_requests.feedback.hold.limit"),
    Hold::SharedLimit => task_only("pull_requests.feedback.hold.sharedLimit"),
    Hold::Question(asker) => ctx.t_args(
      "pull_requests.feedback.hold.question",
      [("agent", &**asker), ("task", task)],
    ),
    Hold::Plan(asker) => ctx.t_args("pull_requests.feedback.hold.plan", [("agent", &**asker)]),
    Hold::Closed(state) => task_only(closed_key(*state)),
    Hold::Next => task_only("pull_requests.feedback.hold.next"),
  }
}

/// Why no agent of the task reads messages any more.
pub const fn closed_key(state: PanelState) -> &'static str {
  match state {
    PanelState::Merged => "pull_requests.feedback.hold.merged",
    PanelState::WorktreeRemoved | PanelState::FailedWorktreeRemoved => "pull_requests.feedback.hold.worktreeRemoved",
    PanelState::FailedNoAgent => "pull_requests.feedback.hold.noAgent",
    _ => "pull_requests.feedback.hold.ended",
  }
}

/// When the agent reads a request in the conversation.
pub fn queue(ctx: &Ctx, queue: &Queue, (task, agent): (&str, &str)) -> Arc<str> {
  let key = match queue {
    Queue::AfterTurn(_) => "afterTurn",
    Queue::SendingNow(_) => "sendingNow",
    Queue::AgentNextTurn(_) => "agentNextTurn",
    Queue::RunStarts => "runStarts",
    Queue::RunResumes(_) | Queue::RunContinues(_) => "runGoesOn",
    Queue::NextRun => "nextRun",
    Queue::NextCheckpoint | Queue::RunEnded | Queue::WorktreeRemoved => "nextCheckpoint",
  };
  ctx.t_args(
    &format!("pull_requests.feedback.queue.{key}"),
    [("agent", agent), ("task", task)],
  )
}

/// An event's icon, word and what it says.
pub fn event(
  ctx: &Ctx,
  event: &ThreadEvent,
  thread: &ThreadView,
  (task, board): (&str, &[ThreadView]),
) -> (Icon, Arc<str>, Arc<str>, Timestamp) {
  let agent = &*thread.recipient;
  let revision = short_revision(thread.current().revision.as_str()).to_owned();
  let args = |key: &str| ctx.t_args(key, [("agent", agent), ("task", task), ("revision", &revision)]);
  let word = |key: &str| ctx.t(&format!("pull_requests.feedback.state.{key}"));
  match event {
    ThreadEvent::Queued { at, hold } => {
      let detail = match hold {
        Hold::Next => args("pull_requests.feedback.hold.waited"),
        hold => self::hold(ctx, hold, (task, agent), board),
      };
      (Icon::Hourglass, word("queued"), detail, *at)
    }
    ThreadEvent::Delivered { at } => {
      let key = if thread.anchor == FeedbackAnchor::PullRequest {
        "pull_requests.feedback.event.deliveredWhole"
      } else {
        "pull_requests.feedback.event.delivered"
      };
      (Icon::Send, word("delivered"), args(key), *at)
    }
    ThreadEvent::Working { at } => (
      Icon::LoaderCircle,
      word("working"),
      args("pull_requests.feedback.event.working"),
      *at,
    ),
    ThreadEvent::Answered { at } => (
      Icon::MessageSquareReply,
      word("answered"),
      args("pull_requests.feedback.event.answered"),
      *at,
    ),
    ThreadEvent::Committed { at, commit } => (
      Icon::Eye,
      word("readyToVerify"),
      ctx.t_args(
        "pull_requests.feedback.event.committed",
        [("agent", agent), ("commit", short_revision(commit.as_str()))],
      ),
      *at,
    ),
    ThreadEvent::NotDelivered { at, why } => {
      let key = match why {
        NotDelivered::Cli(_) => "pull_requests.feedback.event.cli",
        NotDelivered::RunEnded => "pull_requests.feedback.event.runEnded",
      };
      (Icon::TriangleAlert, word("notDelivered"), args(key), *at)
    }
  }
}

/// `Codex / GPT-5`: the target's connection and model, when they are known.
pub fn model(ctx: &Ctx, provider: Option<AgentProvider>, model: Option<&str>) -> Option<Arc<str>> {
  let model = model?;
  let provider = match provider? {
    AgentProvider::CodexCli => "codex",
    AgentProvider::ClaudeCodeCli => "claudeCode",
    AgentProvider::AnthropicApi => "anthropic",
    AgentProvider::OpenAiApi => "openAi",
  };
  Some(ctx.t_args(
    "pull_requests.feedback.strip.model",
    [
      (
        "provider",
        ctx.t(&format!("pull_requests.feedback.provider.{provider}")),
      ),
      ("model", Arc::from(model)),
    ],
  ))
}

/// `380k of 1.4M` as `(left, budget)`, compact.
pub fn budget(budget: Budget) -> (String, String) {
  (compact_tokens(budget.left()), compact_tokens(budget.limit))
}

/// The time of day a thing happened.
pub fn time(at: Timestamp) -> String {
  clock(at)
}

/// How long since `since`: `4 min`.
pub fn elapsed(ctx: &Ctx, since: Timestamp) -> Arc<str> {
  message_text(ctx, &waited(Timestamp::now(), since))
}

/// The task's key, or nothing while the pull request has no task.
pub fn task_key(task: Option<&BoardTask>) -> String {
  task.map(|task| task.key.to_string()).unwrap_or_default()
}
