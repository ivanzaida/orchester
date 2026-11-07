//! The lines about the conversation's subscriptions (`avDZO` a–c and the subscription line `yJHNj` in `04` `OKtX2`):
//! *Leading Webhooks v2 · 12 tasks and Search index · 5 tasks* after a /watch, with its `eye`; *Billing is now led in
//! «Webhooks v2»* in the conversation that lost a target, and the variant for a task inside its module, with an
//! `arrow-right-left` and *Open ↗* to the conversation that leads it now; and *Stopped watching Search index* with an
//! `eye-off`, its time, and under it, 27 in and 4 apart, what the stop left open as links to the tasks. The lines of
//! a target taken here and led here again read as *now led in* does, with their time.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::{
    WatchTarget,
    watch::{NamedTarget, OpenTask, OpenWork, WatchLine},
  },
  project::TaskKey,
  time::Timestamp,
};

use super::{Said, WatchFacts, joined, line_row, link, target_name, time};
use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route, TaskTab, TaskView},
  screens::orchestrator::timeline::TurnEnv,
  theme::{ExtraSpacing, OrchestratorSpacing},
  ui::icon::Icon,
};

/// The row of `line`, kept at `at`, named `id`; `None` for a /watch that leads nothing new here (what it found led
/// elsewhere or gone is the command's card's to show).
pub(super) fn line(
  ctx: &mut Ctx,
  id: &str,
  line: &WatchLine,
  (at, env): (Option<Timestamp>, &TurnEnv),
) -> Option<Element> {
  let element = match line {
    WatchLine::Watching {
      started, already_here, ..
    } => {
      let led: Vec<&NamedTarget> = started.iter().chain(already_here).collect();
      if led.is_empty() {
        return None;
      }
      leading(ctx, id, &led, &env.watch)
    }
    WatchLine::NowLedIn {
      target,
      chat,
      title,
      taken_over,
    } => now_led_in(ctx, id, (target, *taken_over, &env.watch), (env.project, *chat, title)),
    WatchLine::TookOver { target, title, .. } => {
      let said = moved(
        ctx,
        target,
        ("orchestrator.watch.tookOver", "orchestrator.watch.tookOverAside"),
        title,
      );
      line_row(id, "took_over", said, time(id, at)).into()
    }
    WatchLine::LedHereAgain { target, title, .. } => {
      let keys = (
        "orchestrator.watch.ledHereAgain",
        "orchestrator.watch.ledHereAgainAside",
      );
      line_row(id, "led_here_again", moved(ctx, target, keys, title), time(id, at)).into()
    }
    WatchLine::Stopped { targets, left_open } => stopped(ctx, id, (targets, left_open), (at, env.project)),
    // A target left where another conversation leads it shows in its card's place (`super::super::led`).
    WatchLine::LeftThere { .. } => return None,
  };
  Some(element)
}

/// *Leading* the targets `led`, each module with how many tasks it holds now.
fn leading(ctx: &Ctx, id: &str, led: &[&NamedTarget], facts: &WatchFacts) -> Element {
  let names: Vec<Arc<str>> = led
    .iter()
    .map(|named| {
      let name = target_name(ctx, named);
      match facts.tasks_of(named.target) {
        Some(count) => {
          let key = if count == 1 {
            "orchestrator.watch.counted.one"
          } else {
            "orchestrator.watch.counted.other"
          };
          ctx.t_args(key, [("target", name.to_string()), ("count", count.to_string())])
        }
        None => name,
      }
    })
    .collect();
  let said = Said {
    glyph: (Icon::Eye, PaletteColor::TextMuted),
    words: ctx.t_args("orchestrator.watch.leading", [("targets", joined(ctx, &names))]),
    aside: Some(ctx.t("orchestrator.watch.leadingAside")),
  };
  line_row(id, "leading", said, None).into()
}

/// *X is now led in «title»*, conversation `chat` of `project`, with *Open ↗* to it. A target the person took there
/// with *Take it here* says its events start turns there; a task a task subscription took out of what this
/// conversation watches says the rest of its module stays here.
fn now_led_in(
  ctx: &mut Ctx,
  id: &str,
  (target, taken_over, facts): (&NamedTarget, bool, &WatchFacts),
  (project, chat, title): (ProjectId, ChatId, &str),
) -> Element {
  let name = target_name(ctx, target);
  let aside = match (taken_over, target.target) {
    (false, WatchTarget::Task(task)) => match facts.module_of(task) {
      Some(module) => ctx.t_args("orchestrator.watch.nowLedInTask", [("module", module)]),
      None => ctx.t("orchestrator.watch.nowLedInTaskBare"),
    },
    _ => ctx.t("orchestrator.watch.nowLedInAside"),
  };
  let said = Said {
    glyph: (Icon::ArrowRightLeft, PaletteColor::TextMuted),
    words: ctx.t_args("orchestrator.watch.nowLedIn", [("target", &*name), ("title", title)]),
    aside: Some(aside),
  };
  let route = Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)));
  let open = link(
    &format!("{id}-open"),
    ctx.t("orchestrator.line.open"),
    Navigation::of(ctx).to(route),
  );
  line_row(id, "now_led_in", said, Some(open)).into()
}

/// A target that moved between conversations, `target`, as the keys `(words, aside)` say it, naming the other
/// conversation by its `title`.
fn moved(ctx: &Ctx, target: &NamedTarget, (words, aside): (&str, &str), title: &str) -> Said {
  let name = target_name(ctx, target);
  Said {
    glyph: (Icon::ArrowRightLeft, PaletteColor::TextMuted),
    words: ctx.t_args(words, [("target", &*name)]),
    aside: Some(ctx.t_args(aside, [("title", title)])),
  }
}

/// *Stopped watching* `targets` (all of them when it names none), kept at `at`, with what it left open under it, each
/// a link to its task in `project`.
fn stopped(
  ctx: &mut Ctx,
  id: &str,
  (targets, left_open): (&[NamedTarget], &[OpenTask]),
  (at, project): (Option<Timestamp>, ProjectId),
) -> Element {
  let names: Vec<Arc<str>> = targets.iter().map(|named| target_name(ctx, named)).collect();
  let words = if names.is_empty() {
    ctx.t("orchestrator.watch.stoppedAll")
  } else {
    ctx.t_args("orchestrator.watch.stopped", [("targets", joined(ctx, &names))])
  };
  let aside = match left_open.len() {
    0 => ctx.t("orchestrator.watch.nothingOpen"),
    1 => ctx.t_args("orchestrator.watch.leftOpen.one", [("count", "1")]),
    count => ctx.t_args("orchestrator.watch.leftOpen.other", [("count", count.to_string())]),
  };
  let said = Said {
    glyph: (Icon::EyeOff, PaletteColor::TextMuted),
    words,
    aside: Some(aside),
  };
  let row = line_row(&format!("{id}-line"), "stopped", said, time(id, at));
  let open: Vec<Element> = left_open
    .iter()
    .enumerate()
    .filter_map(|(index, open)| left_open_link(ctx, &format!("{id}-left-{index}"), open, project))
    .collect();
  let left = (!open.is_empty()).then(|| {
    Column::new()
      .id(format!("{id}-left"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(OrchestratorSpacing::LeftOpen.role()))
      .spacing(SpacingSize::Xs)
      .with_children(open)
  });
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(row)
    .with_children(left)
    .into()
}

/// What a stop left open of a task, `open`, as a link to its Summary in `project`, named `id`; `None` for a key that
/// does not read as one.
fn left_open_link(ctx: &mut Ctx, id: &str, open: &OpenTask, project: ProjectId) -> Option<Element> {
  let key: TaskKey = open.task.parse().ok()?;
  let words_key = match open.work {
    OpenWork::Waits => "orchestrator.watch.open.waits",
    OpenWork::Runs => "orchestrator.watch.open.runs",
    OpenWork::Queued => "orchestrator.watch.open.queued",
    OpenWork::StoppedAtLimit => "orchestrator.watch.open.stoppedAtLimit",
  };
  let words = ctx.t_args(words_key, [("task", open.task.as_str())]);
  let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary)));
  Some(link(id, words, Navigation::of(ctx).to(route)))
}
