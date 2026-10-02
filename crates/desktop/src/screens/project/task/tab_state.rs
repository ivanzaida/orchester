//! A task page tab without its content (`hig8O`): an icon, a title and an explanation centered in
//! the tab, e.g. a run that has not started, loading, couldn't load; the Changes tab's states are
//! drawn alike.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};
use orchester_core::run::RunStatus;

use crate::{
  tasks::{detail::RunView, unreadable::Unreadable},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// Width of the explanation (`yUyVC`).
const TEXT_WIDTH: f32 = 420.0;

/// An icon, a title and an explanation centered in the tab, with optional actions.
pub(super) fn state(id: &'static str, glyph: Icon, title: &str, text: &str, actions: Vec<Element>) -> Element {
  let actions = (!actions.is_empty()).then(|| Row::new().spacing(SpacingSize::Sm).with_children(actions));
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::State, PaletteColor::TextSecondary))
    .child(Text::new(title).variant(ExtraTypography::CardTitle.role()))
    .child(
      Text::new(text)
        .variant(ExtraTypography::Explanation.role())
        .text_align(TextAlign::Center)
        .width(Dimension::Px(TEXT_WIDTH)),
    )
    .with_children(actions)
    .into()
}

/// A tab of a task whose latest run cannot be read, but its Summary: nothing the run did can be
/// shown (02.R2 `lmlkQ`; 02.R draws the Summary).
pub(super) fn unreadable(ctx: &Ctx, id: &'static str, run: &Unreadable) -> Element {
  state(
    id,
    Icon::AlertCircle,
    &message_text(ctx, &run.title()),
    &ctx.t("runs.unreadable.tabDetail"),
    Vec::new(),
  )
}

/// The timeline has nothing yet, with why: the task has no run, its run has not started (with the
/// steps it will run, `zxIMy`), or its agents have reported nothing so far.
pub(super) fn not_started(ctx: &Ctx, run: Option<&RunView>) -> Element {
  let Some(run) = run else {
    return state(
      "conversation-empty",
      Icon::Clock3,
      &ctx.t("runs.conversation.empty.noRun"),
      &ctx.t("runs.activity.emptyNoRun"),
      Vec::new(),
    );
  };
  if run.run.status() != &RunStatus::Pending {
    let key = if run.run.status().is_terminal() {
      "runs.activity.emptyNone"
    } else {
      "runs.activity.emptyStarting"
    };
    return state(
      "activity-empty",
      Icon::Activity,
      &ctx.t("runs.activity.emptyTitle"),
      &ctx.t(key),
      Vec::new(),
    );
  }
  let number = run.run.number().get().to_string();
  let steps = Column::new()
    .id("conversation-empty-steps")
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .with_children(run.steps.iter().map(|step| {
      Text::new(&ctx.t_args(
        "runs.activity.stepOption",
        [
          ("number", format!("{:02}", step.number).as_str()),
          ("step", &*step.name),
          ("agent", step.agent.as_deref().unwrap_or_default()),
        ],
      ))
      .variant(lurq::app::theme::TypographyStyle::Caption)
    }));
  state(
    "conversation-empty",
    Icon::Clock3,
    &ctx.t_args("runs.conversation.empty.notStartedTitle", [("run", number.as_str())]),
    &ctx.t("runs.conversation.empty.notStarted"),
    vec![steps.into()],
  )
}
