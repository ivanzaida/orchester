//! Ready for review (02.13 `vpgrJ`): the Summary tab of a run whose every step passed. The
//! readiness card (`IBuWf`), the run's stages (`XHMgH`, `i9rVFu`), what its steps reported
//! (`Wegul`), what verified the revision its work ended at (`pWiFM`: the steps, at the latest
//! checkpoint `q91Jf8`), and the review actions with the size of the change (`ksjis`, `T2AAq`);
//! beside them the run's details (`cB4GN`, see [`details`]). What the run did scrolls in a viewport
//! with the scroll gutter, so a long pipeline never pushes the actions out of the window; the
//! actions stay at the bottom.
//!
//! Reviewing the pull request (`o5Txa1`) needs a pull request, which no pipeline step opens yet, so
//! it is drawn unavailable and says why. Asking the agent (`QzsSV`) and Open conversation open the
//! task conversation; Ask agent also puts the focus in its composer. Once the run is merged its
//! conversation takes no messages, so Ask agent is not offered (02.P3 `bMJUA`, `OP7dt`); once its
//! worktree was removed its agent cannot continue, so Ask agent is unavailable and says why, as
//! Merge does (not drawn). Evidence
//! records (acceptance criteria, tests, browser checks) and artifacts do not exist yet, so their
//! rows and the artifact buttons are left out. *Merge into <branch>* ([`merge`], 02.P `GIiCb`)
//! sits between Ask agent and Review PR.

mod details;
mod merge;
mod not_enforced;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::TaskId, integration::Leftovers, run::StepStatus};

pub(super) use self::merge::merged_notice;
use self::merge::{MergeAction, merge_action, merge_button, merge_problem, merge_unavailable, readiness};
use super::{super::ProjectNav, changes::CheckpointedLoad, conversation::ComposerFocus, progress::progress_list};
use crate::{
  display,
  messages::Message,
  navigation::route::{Page, TaskPageTab},
  tasks::{
    changes::Checkpointed,
    detail::{RunView, TaskDetail},
    integration_jobs::{JobState, RunIntegration},
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    link::Link,
  },
  workspace::OpenProject,
};

/// Height of the stage row and of a stage, the stage line's thickness, and a bullet's size
/// (`XHMgH`, `i9rVFu`, `nK1Sl`, `gqIxn`).
const STAGES_HEIGHT: f32 = 44.0;
const STAGE_HEIGHT: f32 = 24.0;
const STAGE_LINE: f32 = 2.0;
const BULLET: f32 = 3.0;
/// Height of the review actions row (`ksjis`).
const ACTIONS_HEIGHT: f32 = 44.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What Ready for review shows, compared to render it again when the task or its run changes.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ReadyProps {
  /// The task, whose latest run succeeded.
  pub(super) detail: Arc<TaskDetail>,
  pub(super) checkpointed: Option<CheckpointedLoad>,
  pub(super) open: OpenProject,
  /// What the run left in the repository, once read.
  pub(super) leftovers: Option<Result<Leftovers, Message>>,
}

impl lurq::app::component::DevtoolsInspectable for ReadyProps {}

/// The Summary tab of a run that succeeded, with its merge; mounted per run.
pub(super) struct ReadyForReview {
  merge_ref: ElementRef,
  integration: RunIntegration,
}

impl Component for ReadyForReview {
  type Props = ReadyProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      merge_ref: ctx.element_ref(),
      integration: ctx
        .use_context::<RunIntegration>()
        .expect("the task page provides its run integration"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReadyProps>().clone();
    let action = merge_action(ctx, &props.open, &props.detail);
    let detail = &props.detail;
    let run = detail
      .run
      .as_ref()
      .expect("Ready for review shows a task whose run succeeded");
    let checkpointed = match &props.checkpointed {
      Some(CheckpointedLoad::Ready(checkpointed)) => Some(checkpointed),
      _ => None,
    };
    let leftovers = props.leftovers.as_ref().and_then(|leftovers| leftovers.as_ref().ok());
    let (ask, open) = conversation(ctx, detail.task.id());
    // Its agent continues in the run's worktree: once that was removed, it cannot be asked.
    let ask = leftovers.is_none_or(Leftovers::keeps_run_worktree).then_some(ask);
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(readiness(ctx, detail.merge.as_ref(), leftovers))
      .with_children(not_enforced::not_enforced(ctx, &nav, run))
      .child(stages(run))
      .with_children(what_changed(ctx, run))
      .child(verification(ctx, run));
    let summary = Column::new()
      .id("task-ready")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Section)
      .padding_left(SpacingSize::Section)
      .padding_right(ExtraSpacing::BesideGutter.role())
      .spacing(SpacingSize::Lg)
      .child(
        ScrollVertical::new(content)
          .id("task-ready-scroll")
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .scrollbar(gutter_scrollbar()),
      )
      .with_children(merge_problem(ctx, detail, &action).map(beside_gutter))
      .child(beside_gutter(actions(
        ctx,
        (detail, &action),
        checkpointed,
        ask,
        &self.merge_ref,
      )));
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(summary)
      .child(details::run_details(
        ctx,
        run,
        props.open.project.name().as_str(),
        props.leftovers.as_ref(),
        detail.merge.is_some(),
        open,
      ))
  }

  fn after_layout(&self) {
    if matches!(self.integration.merge().get_untracked(), JobState::Running) || !self.merge_ref.is_attached() {
      return;
    }
    let width = self.merge_ref.width();
    let saved = self.integration.merge_width();
    if width > 0.0 && (saved.get_untracked() - width).abs() > 0.5 {
      saved.set(width);
    }
  }
}

/// Ask agent and Open conversation: the task's Conversation tab; Ask agent also puts the focus in
/// its composer, and Open conversation is a link.
fn conversation(ctx: &mut Ctx, task: TaskId) -> (Handler, Link) {
  let nav = ctx
    .use_context::<ProjectNav>()
    .expect("the project shell provides ProjectNav");
  let ComposerFocus(focus) = ctx
    .use_context::<ComposerFocus>()
    .expect("the task page provides ComposerFocus");
  let page = Page::Task(task, TaskPageTab::Conversation);
  let open = nav.link(page);
  (
    Arc::new(move || {
      focus.set(true);
      nav.show(page);
    }),
    open,
  )
}

/// A part below the scrolled summary, ending where the viewport's gutter begins.
fn beside_gutter(child: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_right(ExtraSpacing::Stack.role())
    .child(child)
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

/// What the agent steps reported when they finished (`Wegul`); nothing when none did. A tool step's
/// output says how its command ended, which is not a change.
fn what_changed(ctx: &Ctx, run: &RunView) -> Option<Element> {
  let reported: Vec<&str> = run
    .steps
    .iter()
    .filter(|step| !step.tool_step)
    .filter_map(|step| run.reported(step.execution?))
    .map(str::trim)
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

/// The size of the change, then Ask agent, Merge and Review PR, and why the unavailable ones are
/// (`ksjis`); a merged run offers neither Ask agent nor Merge. Ask agent is unavailable without
/// `ask`, once the run's worktree was removed.
fn actions(
  ctx: &Ctx,
  (detail, merge): (&TaskDetail, &MergeAction),
  checkpointed: Option<&Checkpointed>,
  ask: Option<Handler>,
  merge_ref: &ElementRef,
) -> Element {
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
  let ask_unavailable = (detail.merge.is_none() && ask.is_none()).then(|| ctx.t("runs.ready.askUnavailable"));
  let ask = detail.merge.is_none().then(|| {
    let button = ActionButton::new(ctx.t("runs.ready.askAgent"))
      .icon(Icon::MessageSquare)
      .id("task-ask-agent");
    match ask {
      Some(ask) => button.on_click(move || ask()),
      None => button,
    }
    .render(ctx)
  });
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
        .with_children(ask)
        .with_children(merge_button(ctx, detail, merge, merge_ref))
        .child(review),
    )
    .child(hint("task-ready-unavailable", &ctx.t("runs.ready.unavailable")))
    .with_children(ask_unavailable.map(|text| hint("task-ask-unavailable", &text)))
    .with_children(merge_unavailable(ctx, detail).map(|text| hint("task-merge-unavailable", &text)))
    .into()
}

/// A right-aligned line under the actions.
fn hint(id: &'static str, text: &str) -> Element {
  Text::new(text)
    .id(id)
    .variant(ExtraTypography::Hint.role())
    .text_align(TextAlign::Right)
    .width(Dimension::Pct(100.0))
    .into()
}
