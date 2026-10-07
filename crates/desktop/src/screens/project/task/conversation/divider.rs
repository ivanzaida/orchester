//! Thin dividers of the chat (`kpnmP`, `FeiRG`, `o0KDAq`, `uDqzB`): a hairline at each side of a
//! label of a 12 glyph and 11 text, `$text` for what is live or ended the run and `$muted` otherwise.
//! A step starts with one (number, step, agent, engine, times, tokens; while searching its matches)
//! and a step that finished ends with one (*06 Fix notes done · 16:30 · Changes · 3 files ↗*); the
//! step the run ended on closes with its result card instead ([`super::cards::result`]). Finished
//! steps in a row fold into one line (*01–05 · 5 steps done · Show them*), the steps a failure kept
//! from starting share one muted line, returns and restarts are dividers (*Review returned the work
//! to 03 Implement · attempt 2 · 15:12*), and so is the run's end.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  run::{StepStatus, WaitReason},
  time::Timestamp,
  tokens::compact_tokens,
};

use super::{
  super::activity::{engine, header_tokens},
  Timeline,
  chat::{Avatar, chat_row, chat_text},
  nav,
};
use crate::{
  display,
  navigation::route::{Page, TaskPageTab},
  tasks::timeline::{
    ResultCard, RunEnd, StepHead,
    mark::{Mark, MarkKind},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    markdown::plain::plain_text,
    pressable::{pressable, surface},
  },
};

/// The rules' thickness (`u69QQ`).
const RULE_HEIGHT: f32 = 1.0;

/// A divider's label: its glyph, its text and what follows it (a link), in `$text` when `strong`.
fn divider(id: String, (glyph, strong): (Icon, bool), label: Vec<Element>) -> Element {
  let rule = || {
    Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Px(RULE_HEIGHT))
      .background(ExtraColor::Hairline.role())
  };
  let tone = tone(strong);
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .describe("strong", strong.to_string())
    .child(rule())
    .child(
      Row::new()
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Small, tone))
        .with_children(label),
    )
    .child(rule())
    .into()
}

fn tone(strong: bool) -> PaletteColor {
  if strong {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  }
}

/// A divider's text: 11, giving way with an ellipsis in a narrow column.
fn label(text: &str, strong: bool) -> Element {
  Text::new(text)
    .id("conversation-divider-text")
    .variant(TypographyStyle::Caption)
    .color(tone(strong))
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0))
    .flex_shrink(1.0)
    .into()
}

/// "03 Implement".
pub(super) fn step_label(ctx: &Ctx, number: usize, step: &str) -> Arc<str> {
  ctx.t_args(
    "runs.conversation.stepLabel",
    [("number", format!("{number:02}").as_str()), ("step", step)],
  )
}

/// A step's divider (`rx1p9`): pressing a finished step's folds it; pressing its tokens opens Details
/// on it.
pub(super) fn step_divider(
  ctx: &Ctx,
  timeline: &Timeline,
  (head, key): (&StepHead, &Arc<str>),
  (strong, foldable, matches): (bool, bool, Option<usize>),
) -> Element {
  let text = match matches {
    Some(count) => {
      let key = if count == 1 {
        "runs.conversation.divider.matches.one"
      } else {
        "runs.conversation.divider.matches.other"
      };
      ctx.t_args(
        key,
        [
          ("step", &*step_label(ctx, head.number, &head.step)),
          ("agent", &*head.agent),
          ("count", count.to_string().as_str()),
        ],
      )
    }
    None => step_text(ctx, head),
  };
  let tokens =
    (matches.is_none() && !head.command && head.usage.counted() > 0).then(|| tokens(ctx, timeline, head, strong));
  let line = divider(
    format!("conversation-step-{key}"),
    (step_glyph(head), strong),
    std::iter::once(label(&text, strong)).chain(tokens).collect(),
  );
  let line: Element = if foldable {
    let (toggles, key) = (timeline.toggles.clone(), key.clone());
    pressable(surface().id("activity-step-header"), move || toggles.flip(&key, true))
      .width(Dimension::Pct(100.0))
      .child(line)
      .into()
  } else {
    Column::new()
      .id("activity-step-header")
      .width(Dimension::Pct(100.0))
      .child(line)
      .into()
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .describe(
      "execution",
      head
        .execution
        .map(|execution| execution.to_string())
        .unwrap_or_default(),
    )
    .child(line)
    .into()
}

/// The glyph of a step's divider: a tool step's terminal, what it waits for, else its state.
fn step_glyph(head: &StepHead) -> Icon {
  if head.command {
    return Icon::Terminal;
  }
  match head.status {
    Some(StepStatus::Waiting(WaitReason::Plan)) => Icon::ListChecks,
    Some(StepStatus::Waiting(WaitReason::Message)) => Icon::MessageCircle,
    Some(StepStatus::Waiting(WaitReason::Approval)) => Icon::Hand,
    Some(StepStatus::Failed) => Icon::CircleX,
    Some(StepStatus::Passed) => Icon::Check,
    Some(StepStatus::Skipped) => Icon::CircleMinus,
    _ => Icon::Loader,
  }
}

/// "07 Docs & cleanup · Builder · Codex / Sol · started 16:31", "03 Implement · Builder · Codex / Sol
/// · attempt 2 · 12:04–12:38", "04 Run checks · tool · 12:38–12:40".
fn step_text(ctx: &Ctx, head: &StepHead) -> Arc<str> {
  let step = step_label(ctx, head.number, &head.step);
  let start = head.started.map(display::clock).unwrap_or_default();
  let time = match head.ended {
    Some(ended) => span(ctx, head.started, ended),
    None => ctx.t_args("runs.conversation.divider.started", [("start", start.as_str())]),
  };
  if head.command {
    return ctx.t_args("runs.conversation.divider.tool", [("step", &*step), ("time", &*time)]);
  }
  let engine = engine(ctx, head.vendor.as_ref(), &head.model);
  // Orchester runs a native agent itself (04.H10 `rL1aI`).
  let engine = match head.provider {
    Some(provider) if provider.is_native() => ctx.t_args("runs.activity.engineNative", [("engine", &*engine)]),
    _ => engine,
  };
  let engine = match head.attempt.filter(|attempt| *attempt > 1) {
    Some(attempt) => ctx.t_args(
      "runs.conversation.divider.engineAttempt",
      [("engine", &*engine), ("attempt", attempt.to_string().as_str())],
    ),
    None => engine,
  };
  ctx.t_args(
    "runs.conversation.divider.agent",
    [
      ("step", &*step),
      ("agent", &*head.agent),
      ("engine", &*engine),
      ("time", &*time),
    ],
  )
}

/// "12:01–12:02", or one time when both fall on the same minute.
fn span(ctx: &Ctx, started: Option<Timestamp>, ended: Timestamp) -> Arc<str> {
  let end = display::clock(ended);
  match started.map(display::clock) {
    Some(start) if start != end => ctx.t_args(
      "runs.activity.timeSpan",
      [("start", start.as_str()), ("end", end.as_str())],
    ),
    _ => end.as_str().into(),
  }
}

/// "· 38k tokens", with those before the first edit; pressed, Details opens on the step.
fn tokens(ctx: &Ctx, timeline: &Timeline, head: &StepHead, strong: bool) -> Element {
  let used = ctx.t_args(
    "runs.tokens",
    [("count", compact_tokens(head.usage.counted()).as_str())],
  );
  let used = header_tokens(ctx, used, head.before_edit.as_ref(), head.ended.is_some());
  let text = Text::new(&ctx.t_args("runs.conversation.divider.tokens", [("tokens", &*used)]))
    .variant(TypographyStyle::Caption)
    .color(tone(strong))
    .nowrap()
    .flex_shrink(0.0);
  match head.execution {
    Some(execution) => pressable(
      surface().id("activity-step-tokens"),
      timeline.details.open_on(execution),
    )
    .child(text)
    .into(),
    None => text.into(),
  }
}

/// Finished steps folded into one line, with *Show them*.
pub(super) fn folded(ctx: &Ctx, timeline: &Timeline, key: &Arc<str>, steps: &[(Arc<str>, StepHead)]) -> Element {
  let numbers = match (steps.first(), steps.last()) {
    (Some((_, first)), Some((_, last))) if first.number != last.number => ctx.t_args(
      "runs.conversation.range",
      [
        ("first", format!("{:02}", first.number).as_str()),
        ("last", format!("{:02}", last.number).as_str()),
      ],
    ),
    (Some((_, only)), _) => format!("{:02}", only.number).into(),
    _ => "".into(),
  };
  let (text_key, link_key) = if steps.len() == 1 {
    ("runs.conversation.fold.one", "runs.conversation.fold.showIt")
  } else {
    ("runs.conversation.fold.other", "runs.conversation.fold.showThem")
  };
  let text = ctx.t_args(
    text_key,
    [("steps", &*numbers), ("count", steps.len().to_string().as_str())],
  );
  let (toggles, keys) = (
    timeline.toggles.clone(),
    steps.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>(),
  );
  let show = pressable(surface().id("conversation-fold-show"), move || toggles.open_all(&keys)).child(
    Text::new(&ctx.t(link_key))
      .variant(TypographyStyle::Link)
      .color(PaletteColor::TextPrimary)
      .nowrap(),
  );
  divider(
    format!("conversation-{key}"),
    (Icon::CircleCheck, false),
    vec![label(&text, false), show.into()],
  )
}

/// "05 Review, 06 Fix notes and 07 Docs & cleanup did not start" (`bbW23`).
pub(super) fn not_started(ctx: &Ctx, steps: &[StepHead]) -> Element {
  let labels: Vec<Arc<str>> = steps
    .iter()
    .map(|head| step_label(ctx, head.number, &head.step))
    .collect();
  let key = if steps.len() == 1 {
    "runs.conversation.notStarted.one"
  } else {
    "runs.conversation.notStarted.other"
  };
  let text = ctx.t_args(key, [("steps", list(ctx, &labels).as_str())]);
  divider(
    "conversation-not-started".into(),
    (Icon::CircleDashed, false),
    vec![label(&text, false)],
  )
}

/// "02 Plan and 03 Implement": `labels` joined as a list.
pub(super) fn list(ctx: &Ctx, labels: &[Arc<str>]) -> String {
  match labels {
    [] => String::new(),
    [one] => one.to_string(),
    [rest @ .., last] => {
      let rest: Vec<&str> = rest.iter().map(|label| &**label).collect();
      ctx
        .t_args(
          "common.listAnd",
          [
            ("rest", rest.join(&ctx.t("common.listSeparator")).as_str()),
            ("last", last),
          ],
        )
        .to_string()
    }
  }
}

/// A finished step's end (`oDAin`): *06 Fix notes done · 16:30 ·* and *Changes · 3 files ↗*, or
/// *Summary ↗* when it changed no file; a failed one with what it reported (`IYNp8`).
pub(super) fn result(ctx: &mut Ctx, timeline: &Timeline, card: &ResultCard, at: Timestamp) -> Element {
  let step = step_label(ctx, card.number, &card.step);
  let title_key = match (card.passed, card.verdict) {
    (true, true) => "runs.conversation.result.passed",
    (true, false) => "runs.conversation.result.done",
    (false, _) => "runs.conversation.result.failed",
  };
  let title = ctx.t_args(title_key, [("step", &*step)]);
  let time = display::clock(at);
  let id = format!("conversation-result-{}", card.execution);
  if !card.passed {
    let text = match card.reported.as_deref() {
      Some(reported) => ctx.t_args(
        "runs.conversation.divider.failed",
        [
          ("title", &*title),
          ("time", time.as_str()),
          ("reported", plain_text(reported).as_str()),
        ],
      ),
      None => ctx.t_args(
        "runs.conversation.divider.ended",
        [("title", &*title), ("time", time.as_str())],
      ),
    };
    return divider(id, (Icon::CircleX, true), vec![label(&text, true)]);
  }
  let text = ctx.t_args(
    "runs.conversation.divider.done",
    [("title", &*title), ("time", time.as_str())],
  );
  let link = result_link(ctx, timeline, card.edited);
  divider(id, (Icon::CircleCheck, false), vec![label(&text, false), link])
}

/// *Changes · 3 files ↗* when the step edited files, else *Summary ↗*.
fn result_link(ctx: &mut Ctx, timeline: &Timeline, edited: usize) -> Element {
  if edited > 0 {
    return changes_link(ctx, timeline, edited);
  }
  link(
    ctx,
    timeline,
    ("conversation-result-summary", ctx.t("runs.conversation.result.summary")),
    TaskPageTab::Summary,
  )
}

/// *Changes · 3 files ↗*, or *Changes ↗* when the step's transcript names no edited file.
pub(super) fn changes_link(ctx: &mut Ctx, timeline: &Timeline, edited: usize) -> Element {
  let text = match edited {
    0 => ctx.t("runs.conversation.result.changes"),
    1 => ctx.t("runs.conversation.result.changesFiles.one"),
    count => ctx.t_args(
      "runs.conversation.result.changesFiles.other",
      [("count", count.to_string().as_str())],
    ),
  };
  link(
    ctx,
    timeline,
    ("conversation-result-changes", text),
    TaskPageTab::Changes,
  )
}

/// A link (11/500 `$text`) to a tab of the task.
pub(super) fn link(
  ctx: &mut Ctx,
  timeline: &Timeline,
  (id, text): (&'static str, Arc<str>),
  tab: TaskPageTab,
) -> Element {
  nav(ctx)
    .link(Page::Task(timeline.task, tab))
    .on(surface().id(id))
    .child(
      Text::new(&text)
        .variant(TypographyStyle::Link)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .into()
}

/// A return or restart (`j0qLQz`): a divider that says what failed and where the work goes, what
/// follows from it under the line, or the person's note as their message.
pub(super) fn mark(ctx: &mut Ctx, timeline: &Timeline, mark: &Mark) -> Element {
  let (glyph, text) = mark_text(ctx, timeline, mark);
  let line = divider(
    format!("activity-mark-{}", mark.attempt),
    (glyph, true),
    vec![label(&text, true)],
  );
  let below = match &mark.kind {
    MarkKind::ReturnedWithNote { note, .. } => {
      let meta = mark.at.map(display::clock).unwrap_or_default();
      chat_row(
        format!("activity-mark-detail-{}", mark.attempt),
        Avatar::Person,
        (&ctx.t("runs.conversation.you"), &meta),
        vec![chat_text(ctx, "activity-mark-note", note)],
      )
    }
    _ => Text::new(&mark_detail(ctx, mark))
      .id(format!("activity-mark-detail-{}", mark.attempt))
      .variant(ExtraTypography::CellLabel.role())
      .width(Dimension::Pct(100.0))
      .selectable(true)
      .into(),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Group.role())
    .child(line)
    .child(below)
    .into()
}

/// "Review failed in attempt 2 · the work goes back to 03 Implement · automatic · retry 2 of 2 ·
/// 14:39", "Restarted 03 Implement · it stopped without a result · automatic · attempt 2 · 12:16",
/// "You returned the work to 03 Implement · 14:52".
fn mark_text(ctx: &Ctx, timeline: &Timeline, mark: &Mark) -> (Icon, Arc<str>) {
  let time = mark.at.map(display::clock).unwrap_or_default();
  let attempt = mark.attempt.to_string();
  match &mark.kind {
    MarkKind::Returned {
      to,
      failed,
      failed_attempt,
      retry: (taken, allowed),
      ..
    } => {
      let key = if mark.by_person {
        "runs.conversation.mark.returnedByYou"
      } else {
        "runs.conversation.mark.returned"
      };
      let text = ctx.t_args(
        key,
        [
          ("failed", &**failed),
          ("failedAttempt", failed_attempt.to_string().as_str()),
          ("step", &*timeline.labelled(ctx, to)),
          ("retry", taken.to_string().as_str()),
          ("max", allowed.to_string().as_str()),
          ("time", time.as_str()),
        ],
      );
      (Icon::Undo2, text)
    }
    MarkKind::Restarted { step } => {
      let key = if mark.by_person {
        "runs.conversation.mark.restartedByYou"
      } else {
        "runs.conversation.mark.restarted"
      };
      let text = ctx.t_args(
        key,
        [
          ("step", &*timeline.labelled(ctx, step)),
          ("attempt", attempt.as_str()),
          ("time", time.as_str()),
        ],
      );
      (Icon::RotateCcw, text)
    }
    MarkKind::ReturnedWithNote { to, .. } => (
      Icon::Undo2,
      ctx.t_args(
        "runs.conversation.mark.returnedWithNote",
        [("step", &*timeline.labelled(ctx, to)), ("time", time.as_str())],
      ),
    ),
  }
}

/// What follows from an automatic or decided return or restart.
fn mark_detail(ctx: &Ctx, mark: &Mark) -> Arc<str> {
  match &mark.kind {
    MarkKind::Returned {
      agent, failed, after, ..
    } if after.is_empty() => ctx.t_args(
      "runs.conversation.mark.returnedDetailAlone",
      [("agent", &**agent), ("failed", &**failed)],
    ),
    MarkKind::Returned {
      agent, failed, after, ..
    } => {
      let steps = after.join(&*ctx.t("runs.conversation.mark.and"));
      ctx.t_args(
        "runs.conversation.mark.returnedDetail",
        [("agent", &**agent), ("failed", &**failed), ("steps", steps.as_str())],
      )
    }
    MarkKind::Restarted { .. } | MarkKind::ReturnedWithNote { .. } => ctx.t("runs.conversation.mark.restartedDetail"),
  }
}

/// The run's end (`k9DqeL` in `uDqzB`).
pub(super) fn end(ctx: &Ctx, end: &RunEnd, (run, at): (u64, Timestamp)) -> Element {
  let (number, time) = (run.to_string(), display::clock(at));
  let (glyph, text) = match end {
    RunEnd::Ready => (
      Icon::GitPullRequest,
      ctx.t_args(
        "runs.conversation.end.ready",
        [("run", number.as_str()), ("time", time.as_str())],
      ),
    ),
    RunEnd::Merged(branch) => (
      Icon::GitMerge,
      ctx.t_args(
        "runs.conversation.end.merged",
        [("run", number.as_str()), ("branch", &**branch), ("time", time.as_str())],
      ),
    ),
    RunEnd::Failed(step) => (
      Icon::CircleX,
      ctx.t_args(
        "runs.conversation.end.failed",
        [("run", number.as_str()), ("step", &**step), ("time", time.as_str())],
      ),
    ),
    RunEnd::Cancelled => (
      Icon::Square,
      ctx.t_args(
        "runs.conversation.end.cancelled",
        [("run", number.as_str()), ("time", time.as_str())],
      ),
    ),
  };
  divider("conversation-end".into(), (glyph, true), vec![label(&text, true)])
}
