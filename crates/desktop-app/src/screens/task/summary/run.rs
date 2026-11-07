//! The run (`Progress` `ee0vh` in `01 · Summary`): *Run* with its pipeline and number in 12 `text_muted`, the rail of
//! its steps (`steps`), and one bordered line with its last event (`Last event` `TGEq8`): where the task stands in a
//! sentence or two in 12 `text_secondary`, after the event's 13 glyph in `text_muted`, and *Conversation →* 12/500 in
//! `text_primary` at its end, in every state (`01`, `01b`, `01c`). 12 apart. A run this computer does not have (a
//! teammate's of a shared task) is its last event alone, without the link. A team run the server fenced, and a team
//! task another computer holds, say so ([`super::team_run`]).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;
use orchester_workspace::tasks::detail::RunView;

use super::{
  browser_line::{BrowserLine, LineProps},
  decisions, section_head, steps, team_run,
};
use crate::{
  navigation::Navigation,
  routes::{Route, TaskTab, TaskView},
  screens::task::{
    TaskContext,
    waits_on::{self, Place},
  },
  services::Services,
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The section of `run`, the task's latest run on this computer, if it has one.
pub fn section(ctx: &mut Ctx, task: &TaskContext, run: Option<&RunView>) -> Element {
  let mut head = section_head(ctx, "task-run-head", "summary.run.title", None);
  let mut section = Column::new()
    .id("task-run")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md);
  if let Some(run) = run {
    let pipeline = ctx.t_args(
      "summary.run.meta",
      [
        ("pipeline", run.pipeline.to_string()),
        ("version", run.version.to_string()),
        ("run", run.run.number().get().to_string()),
      ],
    );
    head = head.child(
      Text::new(&pipeline)
        .id("task-run-meta")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
    section = section.child(head).child(rail(ctx, task, run));
  } else {
    section = section.child(head);
  }
  // The run's tabs in its project's browser, under its last event (`RGeKO`).
  let browser = run.map(|run| {
    ctx.mount_keyed::<BrowserLine>(
      "task-browser-line",
      LineProps {
        root: task.project.open.repo_root.clone(),
        run: run.run.id(),
      },
    )
  });
  // A team run whose authority the server fenced says it stopped, and what became of its reports (03 `hWXxr`); a run
  // that waits on another task, that wait (09 `SLuUW`).
  let event = match run.and_then(|run| team_run::stopped(ctx, task, run)) {
    Some(text) => event_line(ctx, task, (Icon::CornerDownLeft, &text, "stopped"), true),
    None => waits_on::row(ctx, (&task.detail, &task.project.open), Place::Summary)
      .unwrap_or_else(|| last_event(ctx, task, run.is_some())),
  };
  section.child(event).with_children(browser).into()
}

/// The rail of `run`'s steps, the one it stands at marked: in `danger` the step a team run stopped in once the server
/// fenced it.
fn rail(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> Element {
  let lost = task.lost();
  let waiting: Vec<(Arc<str>, Timestamp)> = task
    .waiting_approval()
    .iter()
    .filter_map(|shown| Some((shown.step_key.clone()?, shown.approval.requested_at)))
    .collect();
  let fenced = team_run::fence(task, run).is_some();
  // The step the run stands at, while it has not ended: the one it stopped at, else the one at work or last run; once
  // fenced, the one it stopped in.
  let current = if fenced {
    team_run::stopped_in(run)
  } else {
    (!run.run.status().is_terminal())
      .then(|| run.stopped_step().or_else(|| run.current_step()))
      .flatten()
  };
  let current = current.map(|step| Arc::from(step.key.as_str()));
  let slot_waits = task.slot_waits();
  let now = Services::of(ctx).clock.now();
  let live = steps::LiveSteps {
    lost,
    waiting: &waiting,
    slot_waits: &slot_waits,
    fenced,
    waits_on: waits_on::holding(&task.detail).is_some(),
  };
  steps::rail(ctx, run, &live, current, now)
}

/// Where the task stands, as one line on the page's ground in a `border` stroke (`Last event` `TGEq8`); `here`: the run
/// is on this computer, so its conversation is too. Once the run ended with a pull request, or while its agent works on
/// a Fix CI, the line says how its CI stands (ORC-335).
fn last_event(ctx: &mut Ctx, task: &TaskContext, here: bool) -> Element {
  let said = decisions::said(ctx, task);
  let (glyph, text) = crate::screens::task::ci::header::event(ctx, task).unwrap_or_else(|| {
    let text = ctx.t_args(
      "summary.run.event",
      [("title", said.title.clone()), ("body", said.body.clone())],
    );
    (said.icon, text)
  });
  event_line(ctx, task, (glyph, &text, said.tone.name()), here)
}

/// One bordered line: the event's `glyph`, its `text` and *Conversation →* when `link`; `tone` names it for the MCP
/// tree.
pub(super) fn event_line(
  ctx: &mut Ctx,
  task: &TaskContext,
  (glyph, text, tone): (Icon, &str, &str),
  link: bool,
) -> Element {
  let link = link.then(|| {
    let navigation = Navigation::of(ctx);
    let route = Route::Project(
      task.project.open.project.id(),
      task.page(TaskView::of(TaskTab::Conversation)),
    );
    opening(navigation.to(route))
      .id("task-open-conversation")
      .describe("a11y_name", ctx.t("summary.openConversation"))
      .child(
        Text::new(&ctx.t("summary.openConversation"))
          .variant(TypographyStyle::Label)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      )
  });
  lurq::components::Row::new()
    .id("task-event")
    .describe("tone", tone.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .id("task-event-text")
        .variant(TypographyStyle::Description)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(link)
    .into()
}
