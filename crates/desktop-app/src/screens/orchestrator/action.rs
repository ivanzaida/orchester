//! An action the Orchestrator took through one of Orchester's tools, in its answer (`Action card` `nQIhx`): on
//! `surface_raised` in a `border` stroke, radius 10, 12 over and under and 14 at its sides, 10 apart: a 14 icon and
//! what was done (12.5/600), and at its end whether it was done (`circle-check` in `success`, *Confirmed* and when)
//! or refused (`ban`, *Refused* and when); what it did, or the tool's own reason it refused (13/1.5); that it was
//! checked like the same action in the app (a 12 `shield-check` and 11.5 `text_muted`); and *Open* the task it
//! names.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{ActionOutcome, ChatAction, change::Publishing},
  project::TaskKey,
  time::Timestamp,
};
use orchester_workspace::display;

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ConversationText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// What the card says of a tool, by its name: its icon and keys.
struct Words {
  glyph: Icon,
  done: &'static str,
  refused: &'static str,
  effect: Option<&'static str>,
}

fn words(tool: &str) -> Words {
  let (glyph, done, refused, effect) = match tool {
    "create_task" => (
      Icon::FilePlus,
      "orchestrator.action.createTask.done",
      "orchestrator.action.createTask.refused",
      Some("orchestrator.action.createTask.effect"),
    ),
    "update_task" => (
      Icon::Pencil,
      "orchestrator.action.updateTask.done",
      "orchestrator.action.updateTask.refused",
      Some("orchestrator.action.updateTask.effect"),
    ),
    "run_task" => (
      Icon::Play,
      "orchestrator.action.runTask.done",
      "orchestrator.action.runTask.refused",
      Some("orchestrator.action.runTask.effect"),
    ),
    "pause_run" => (
      Icon::CirclePause,
      "orchestrator.action.pauseRun.done",
      "orchestrator.action.pauseRun.refused",
      Some("orchestrator.action.pauseRun.effect"),
    ),
    "resume_run" => (
      Icon::Play,
      "orchestrator.action.resumeRun.done",
      "orchestrator.action.resumeRun.refused",
      Some("orchestrator.action.resumeRun.effect"),
    ),
    "hold_queue" => (
      Icon::CirclePause,
      "orchestrator.action.holdQueue.done",
      "orchestrator.action.holdQueue.refused",
      Some("orchestrator.action.holdQueue.effect"),
    ),
    "release_queue" => (
      Icon::Play,
      "orchestrator.action.releaseQueue.done",
      "orchestrator.action.releaseQueue.refused",
      Some("orchestrator.action.releaseQueue.effect"),
    ),
    "cancel_run" => (
      Icon::CircleStop,
      "orchestrator.action.cancelRun.done",
      "orchestrator.action.cancelRun.refused",
      Some("orchestrator.action.cancelRun.effect"),
    ),
    "merge_run" => (
      Icon::GitMerge,
      "orchestrator.action.mergeRun.done",
      "orchestrator.action.mergeRun.refused",
      Some("orchestrator.action.mergeRun.effect"),
    ),
    other => return publish_words(other),
  };
  Words {
    glyph,
    done,
    refused,
    effect,
  }
}

/// The words of a tool that publishes a draft, or of any other tool.
fn publish_words(tool: &str) -> Words {
  let (glyph, done, refused, effect) = match Publishing::of_tool(tool) {
    Some(Publishing::Pipeline) => (
      Icon::Upload,
      "orchestrator.action.publishPipeline.done",
      "orchestrator.action.publishPipeline.refused",
      Some("orchestrator.action.publishPipeline.effect"),
    ),
    Some(Publishing::Profile) => (
      Icon::Upload,
      "orchestrator.action.publishProfile.done",
      "orchestrator.action.publishProfile.refused",
      Some("orchestrator.action.publishProfile.effect"),
    ),
    Some(Publishing::Instruction) => (
      Icon::Upload,
      "orchestrator.action.publishInstruction.done",
      "orchestrator.action.publishInstruction.refused",
      Some("orchestrator.action.publishInstruction.effect"),
    ),
    Some(Publishing::Skill) => (
      Icon::Upload,
      "orchestrator.action.publishSkill.done",
      "orchestrator.action.publishSkill.refused",
      Some("orchestrator.action.publishSkill.effect"),
    ),
    Some(Publishing::Tool) => (
      Icon::Upload,
      "orchestrator.action.publishTool.done",
      "orchestrator.action.publishTool.refused",
      Some("orchestrator.action.publishTool.effect"),
    ),
    None => (
      Icon::Wrench,
      "orchestrator.action.other.done",
      "orchestrator.action.other.refused",
      None,
    ),
  };
  Words {
    glyph,
    done,
    refused,
    effect,
  }
}

/// The card of `action`, taken at `at` in project `project` (`name`); `id` names it.
pub(super) fn card(
  ctx: &mut Ctx,
  id: &str,
  (action, at): (&ChatAction, Timestamp),
  (project, name): (ProjectId, &str),
) -> Element {
  let Shown {
    title,
    glyph,
    state,
    state_glyph,
    state_color,
    effect,
  } = shown(ctx, action, at);
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&title)
        .id(format!("{id}-title"))
        .variant(ConversationText::ResultTitle.role())
        .flex_shrink(1.0),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .id(format!("{id}-state"))
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(state_glyph, IconSize::Sm, state_color))
        .child(
          Text::new(&state)
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        ),
    );
  // A proposal waiting or dismissed did nothing yet: no check to tell of.
  let ran = !matches!(
    action.outcome,
    ActionOutcome::Proposed(_) | ActionOutcome::ProposedChange(_) | ActionOutcome::Dismissed
  );
  let checked = ran.then(|| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Start)
      .child(icon(Icon::ShieldCheck, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t_args("orchestrator.action.checked", [("project", name)]))
          .variant(ExtraTypography::Note.role())
          .flex(1.0),
      )
  });
  let open = open_task(ctx, id, project, action.task.as_deref());
  Column::new()
    .id(id.to_owned())
    .describe("outcome", outcome_name(&action.outcome))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Section.role(), SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(head)
    .with_children(effect.map(|effect| {
      Text::new(&effect)
        .id(format!("{id}-effect"))
        .variant(TypographyStyle::Body)
        .width(Dimension::Pct(100.0))
    }))
    .with_children(checked)
    .with_children(open)
    .into()
}

/// What the card of `action`, taken at `at`, says and shows.
struct Shown {
  title: Arc<str>,
  glyph: Icon,
  state: Arc<str>,
  state_glyph: Icon,
  state_color: PaletteColor,
  effect: Option<Arc<str>>,
}

fn shown(ctx: &Ctx, action: &ChatAction, at: Timestamp) -> Shown {
  let words = words(&action.tool);
  let task = action.task.as_deref().unwrap_or_default();
  let args = [("task", task), ("tool", action.tool.as_str())];
  let time = display::clock(at);
  let (title, glyph, state, state_glyph, state_color, effect) = match &action.outcome {
    ActionOutcome::Done => (
      ctx.t_args(words.done, args),
      words.glyph,
      ctx.t_args("orchestrator.action.confirmed", [("time", time.as_str())]),
      Icon::CircleCheck,
      PaletteColor::Success,
      words.effect.map(|key| ctx.t(key)),
    ),
    ActionOutcome::Refused { reason, .. } => (
      ctx.t_args(words.refused, args),
      Icon::Ban,
      ctx.t_args("orchestrator.action.refusedState", [("time", time.as_str())]),
      Icon::Ban,
      PaletteColor::TextSecondary,
      Some(Arc::from(reason.as_str())),
    ),
    // Shown with its Start and Dismiss in a kept conversation (`super::proposal`); this is its card without them.
    ActionOutcome::Proposed(start) => (
      ctx.t_args(
        "orchestrator.proposal.title",
        [("task", task), ("pipeline", start.pipeline.as_str())],
      ),
      words.glyph,
      ctx.t_args("orchestrator.proposal.waits", [("time", time.as_str())]),
      Icon::Hourglass,
      PaletteColor::Warning,
      Some(ctx.t("orchestrator.proposal.effect")),
    ),
    ActionOutcome::ProposedChange(change) => (
      super::proposal::change_title(ctx, change, task),
      super::proposal::change_glyph(change),
      ctx.t_args("orchestrator.proposal.waits", [("time", time.as_str())]),
      Icon::Hourglass,
      PaletteColor::Warning,
      Some(ctx.t("orchestrator.proposal.changeEffect")),
    ),
    ActionOutcome::Dismissed => (
      ctx.t_args(dismissed_title(&action.tool), args),
      words.glyph,
      ctx.t_args("orchestrator.proposal.dismissedState", [("time", time.as_str())]),
      Icon::CircleX,
      PaletteColor::TextSecondary,
      Some(ctx.t("orchestrator.proposal.dismissedEffect")),
    ),
  };
  Shown {
    title,
    glyph,
    state,
    state_glyph,
    state_color,
    effect,
  }
}

/// What the card of a dismissed proposal of `tool` is titled.
fn dismissed_title(tool: &str) -> &'static str {
  match tool {
    "cancel_run" => "orchestrator.proposal.cancel.dismissed",
    "merge_run" => "orchestrator.proposal.merge.dismissed",
    other if Publishing::of_tool(other).is_some() => "orchestrator.proposal.publish.dismissed",
    _ => "orchestrator.proposal.dismissedTitle",
  }
}

/// *Open* the task the action names, when it names one people can read.
pub(super) fn open_task(ctx: &mut Ctx, id: &str, project: ProjectId, task: Option<&str>) -> Option<Element> {
  let key: TaskKey = task?.parse().ok()?;
  let navigation = Navigation::of(ctx);
  let label = ctx.t_args("orchestrator.action.openTask", [("task", key.to_string().as_str())]);
  let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary)));
  Some(
    Row::new()
      .child(
        ActionButton::new(format!("{id}-open"), ButtonKind::Ghost, label)
          .icon(Icon::ArrowUpRight)
          .on_click(move || navigation.go(route.clone())),
      )
      .into(),
  )
}

const fn outcome_name(outcome: &ActionOutcome) -> &'static str {
  match outcome {
    ActionOutcome::Done => "done",
    ActionOutcome::Refused { .. } => "refused",
    ActionOutcome::Proposed(_) | ActionOutcome::ProposedChange(_) => "proposed",
    ActionOutcome::Dismissed => "dismissed",
  }
}
