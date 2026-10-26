//! The run (`Progress` `ee0vh` in `01 · Summary`): *Run* with its pipeline and number in 12 `text_muted`, the rail of
//! its steps (`steps`), and one bordered line with its last event (`Last event` `TGEq8`): where the task stands in a
//! sentence or two in 12 `text_secondary`, after the event's 13 glyph in `text_muted`, and *Open conversation →* where
//! the conversation is where the person looks or answers. 12 apart. A run this computer does not have (a teammate's
//! of a shared task) is its last event alone.

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

use super::{decisions, section_head, steps};
use crate::{
  navigation::Navigation,
  routes::{Route, TaskTab, TaskView},
  screens::task::TaskContext,
  services::Services,
  theme::{ExtraSpacing, IconSize},
  ui::{icon::icon, pressable::opening},
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
  section.child(last_event(ctx, task)).into()
}

/// The rail of `run`'s steps, the one it stands at marked.
fn rail(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> Element {
  let lost = task.lost();
  let waiting: Vec<(Arc<str>, Timestamp)> = task
    .waiting_approval()
    .iter()
    .filter_map(|shown| Some((shown.step_key.clone()?, shown.approval.requested_at)))
    .collect();
  // The step the run stands at, while it has not ended: the one it stopped at, else the one at work or last run.
  let current = (!run.run.status().is_terminal())
    .then(|| run.stopped_step().or_else(|| run.current_step()))
    .flatten()
    .map(|step| Arc::from(step.key.as_str()));
  let now = Services::of(ctx).clock.now();
  steps::rail(ctx, run, (lost, &waiting), current, now)
}

/// Where the task stands, as one line on the page's ground in a `border` stroke (`Last event` `TGEq8`).
fn last_event(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let said = decisions::said(ctx, task);
  let text = ctx.t_args(
    "summary.run.event",
    [("title", said.title.clone()), ("body", said.body.clone())],
  );
  let link = said.conversation.then(|| {
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
    .describe("tone", said.tone.name())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(said.icon, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&text)
        .id("task-event-text")
        .variant(TypographyStyle::Description)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(link)
    .into()
}
