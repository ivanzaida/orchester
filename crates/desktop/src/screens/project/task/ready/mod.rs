//! Ready for review (02.13 `vpgrJ`): the Summary tab of a run whose every step passed. The
//! readiness card (`IBuWf`), the run's stages (`XHMgH`, `i9rVFu`), what its steps reported
//! (`Wegul`), what verified the revision its work ended at (`pWiFM`: the steps, at the latest
//! checkpoint `q91Jf8`), and the review actions with the size of the change (`ksjis`, `T2AAq`);
//! beside them the run's details (`cB4GN`, see [`details`]).
//!
//! Reviewing the pull request (`o5Txa1`) needs a pull request, which no pipeline step opens yet;
//! asking the agent (`QzsSV`) needs the task conversation. Both are drawn unavailable and say why.
//! Evidence records (acceptance criteria, tests, browser checks) and artifacts do not exist yet, so
//! their rows and the artifact buttons are left out.

mod details;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};
use orchester_core::run::StepStatus;

use super::{changes::CheckpointedLoad, progress::progress_list};
use crate::{
  display,
  tasks::{changes::Checkpointed, detail::RunView},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
  },
};

/// Height of the stage row and of a stage, the stage line's thickness, and a bullet's size
/// (`XHMgH`, `i9rVFu`, `nK1Sl`, `gqIxn`).
const STAGES_HEIGHT: f32 = 44.0;
const STAGE_HEIGHT: f32 = 24.0;
const STAGE_LINE: f32 = 2.0;
const BULLET: f32 = 3.0;
/// Height of the review actions row (`ksjis`).
const ACTIONS_HEIGHT: f32 = 44.0;

/// The Summary tab of `run`, which succeeded, in project `project`.
pub(super) fn ready_for_review(
  ctx: &mut Ctx,
  run: &RunView,
  checkpointed: Option<&CheckpointedLoad>,
  project: &str,
) -> Element {
  let checkpointed = match checkpointed {
    Some(CheckpointedLoad::Ready(checkpointed)) => Some(checkpointed),
    _ => None,
  };
  let summary = Column::new()
    .id("task-ready")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(SpacingSize::Lg)
    .child(readiness(ctx))
    .child(stages(run))
    .with_children(what_changed(ctx, run))
    .child(verification(ctx, run))
    .child(Spacer::new().flex(1.0))
    .child(actions(ctx, checkpointed));
  Row::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(summary)
    .child(details::run_details(ctx, run, project))
    .into()
}

/// "Ready for your review" (`IBuWf`).
fn readiness(ctx: &Ctx) -> Element {
  Row::new()
    .id("task-ready-notice")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Group.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(Icon::CircleCheck, IconSize::Panel, PaletteColor::TextPrimary))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Indicator.role())
        .child(Text::new(&ctx.t("runs.ready.title")).variant(ExtraTypography::CardTitle.role()))
        .child(Text::new(&ctx.t("runs.ready.detail")).variant(TypographyStyle::FieldLabel)),
    )
    .into()
}

/// The pipeline's steps, each passed (`XHMgH`).
fn stages(run: &RunView) -> Element {
  let stage = |name: &str, status: Option<StepStatus>| {
    let glyph = match status {
      Some(StepStatus::Skipped) => Icon::CircleMinus,
      _ => Icon::Check,
    };
    Column::new()
      .flex(1.0)
      .height(Dimension::Px(STAGE_HEIGHT))
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Tight.role())
          .align_items(Alignment::Center)
          .child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary))
          .child(Text::new(name).variant(ExtraTypography::Footnote.role()).nowrap()),
      )
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .height(Dimension::Px(STAGE_LINE))
          .rounded(ExtraRadius::Mark.role())
          .background(PaletteColor::TextSecondary),
      )
  };
  Row::new()
    .id("task-ready-stages")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STAGES_HEIGHT))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .with_children(run.steps.iter().map(|step| stage(&step.name, step.status)))
    .into()
}

/// What the steps reported when they finished (`Wegul`); nothing when none did.
fn what_changed(ctx: &Ctx, run: &RunView) -> Option<Element> {
  let reported: Vec<&str> = run
    .steps
    .iter()
    .filter_map(|step| run.output(step.execution?))
    .map(|output| output.text.trim())
    .filter(|text| !text.is_empty())
    .collect();
  if reported.is_empty() {
    return None;
  }
  let bullet = |text: &str| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .child(
        Column::new()
          .size(Dimension::Px(BULLET), Dimension::Px(BULLET))
          .rounded(ExtraRadius::Pill.role())
          .background(PaletteColor::TextSecondary),
      )
      .child(
        Text::new(text)
          .variant(ExtraTypography::Explanation.role())
          .flex(1.0)
          .selectable(true),
      )
  };
  Some(
    Column::new()
      .id("task-ready-changes")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .child(Text::new(&ctx.t("runs.ready.whatChanged")).variant(ExtraTypography::SectionHeading.role()))
      .with_children(reported.into_iter().map(bullet))
      .into(),
  )
}

/// The steps that passed, at the revision the run's work ended at (`pWiFM`, `q91Jf8`).
fn verification(ctx: &Ctx, run: &RunView) -> Element {
  let revision = run.run.last_checkpoint().map_or_else(
    || ctx.t("runs.ready.noCommit"),
    |checkpoint| {
      ctx.t_args(
        "runs.ready.commit",
        [("revision", display::short_revision(checkpoint.revision().as_str()))],
      )
    },
  );
  Column::new()
    .id("task-ready-verification")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .align_items(Alignment::Center)
        .justify(Justify::SpaceBetween)
        .child(Text::new(&ctx.t("runs.ready.verification")).variant(ExtraTypography::SectionHeading.role()))
        .child(
          Text::new(&revision)
            .id("task-ready-revision")
            .variant(TypographyStyle::Mono),
        ),
    )
    .child(progress_list(ctx, run, false))
    .into()
}

/// The size of the change, then Ask agent and Review PR, unavailable and why (`ksjis`).
fn actions(ctx: &Ctx, checkpointed: Option<&Checkpointed>) -> Element {
  let scope = match checkpointed {
    Some(Checkpointed::Read { changes, .. }) => {
      let files = changes.diff.files().len();
      let (additions, deletions) = (
        changes.diff.additions().to_string(),
        changes.diff.deletions().to_string(),
      );
      let key = if files == 1 {
        "runs.ready.scope.one"
      } else {
        "runs.ready.scope.other"
      };
      ctx.t_args(
        key,
        [
          ("files", files.to_string().as_str()),
          ("additions", additions.as_str()),
          ("deletions", deletions.as_str()),
        ],
      )
    }
    Some(Checkpointed::TooLarge { size, .. }) => ctx.t_args(
      "runs.ready.scopeTooLarge",
      [
        ("files", size.files.to_string().as_str()),
        (
          "lines",
          size.additions.saturating_add(size.deletions).to_string().as_str(),
        ),
      ],
    ),
    Some(Checkpointed::NoCheckpoint) => ctx.t("runs.ready.scopeNone"),
    None => ctx.t("runs.ready.scopeReading"),
  };
  let ask = ActionButton::new(ctx.t("runs.ready.askAgent"))
    .icon(Icon::MessageSquare)
    .id("task-ask-agent")
    .render(ctx);
  let review = ActionButton::new(ctx.t("runs.ready.reviewPullRequest"))
    .icon(Icon::GitCompareArrows)
    .primary()
    .id("task-review-pull-request")
    .render(ctx);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .id("task-ready-actions")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(ACTIONS_HEIGHT))
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&scope)
            .id("task-ready-scope")
            .variant(TypographyStyle::Mono)
            .flex(1.0)
            .nowrap(),
        )
        .child(ask)
        .child(review),
    )
    .child(
      Text::new(&ctx.t("runs.ready.unavailable"))
        .id("task-ready-unavailable")
        .variant(ExtraTypography::Hint.role())
        .text_align(TextAlign::Right)
        .width(Dimension::Pct(100.0)),
    )
    .into()
}
