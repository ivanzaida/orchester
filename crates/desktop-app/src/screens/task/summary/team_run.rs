//! What the Summary's run says of a team task's work that is not this computer's to carry on
//! (`docs/design/team-authority.md` §2–3). A team task another computer holds has the run its team server reports in
//! place of *No runs yet* (`Progress` in 02 `nMUwg`, 02b `XbNXU`, 02c `l82OS`, 02d `lpji3`): *Run* with the task's
//! pipeline and whose computer it runs on, and one line with the state of the work and when it last reported, or that
//! none started yet; nothing starts here under that claim. The server names the holder, not their computer. A run of
//! this computer the server fenced stopped in its step, and its reports the server never took stay here to read (03
//! `hWXxr`, 03c `W6Ncf`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_api::v2::ExecutionState;
use orchester_core::run::StepStatus;
use orchester_store::TeamFence;
use orchester_workspace::tasks::detail::{RunView, StepView};

use super::{run::event_line, section_head};
use crate::{
  screens::{
    settings::orchester::moment,
    task::{
      TaskContext,
      team_controls::{held_elsewhere, pipeline_version},
    },
  },
  team::pages::TeamClaim,
  ui::icon::Icon,
};

/// The run of a team task another computer holds, as its team server reports it; `None` while this computer holds the
/// claim or has a run of the task.
pub fn reported(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let claim = held_elsewhere(task)?;
  let team = task.team.as_ref()?;
  let pipeline = pipeline_version(ctx, task);
  let holder = claim.holder.name.as_str();
  let (meta, event) = match team.task.work {
    None if claim.under_module => {
      let event = match &team.module {
        Some(module) => ctx.t_args(
          "summary.team.run.underModule",
          [("holder", holder), ("branch", module.branch.as_str())],
        ),
        None => ctx.t_args("summary.team.run.underModuleNoBranch", [("holder", holder)]),
      };
      (
        ctx.t_args("summary.team.run.notStarted", [("pipeline", pipeline.as_str())]),
        event,
      )
    }
    work => (place(ctx, claim, &pipeline), event(ctx, claim, work)),
  };
  let head = section_head(ctx, "task-run-head", "summary.run.title", None).child(
    Text::new(&meta)
      .id("task-run-meta")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap(),
  );
  let line = event_line(ctx, task, (Icon::MonitorSmartphone, &event, "reported"), true);
  Some(
    Column::new()
      .id("task-run")
      .describe("state", "reported")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(head)
      .child(line)
      .into(),
  )
}

/// *Feature delivery v3 · on mara's computer*, or *· on your other computer* for the person's own claim.
fn place(ctx: &Ctx, claim: &TeamClaim, pipeline: &str) -> Arc<str> {
  if claim.holder.you {
    ctx.t_args("summary.team.run.onYourOther", [("pipeline", pipeline)])
  } else {
    ctx.t_args(
      "summary.team.run.onHolder",
      [("pipeline", pipeline), ("holder", claim.holder.name.as_str())],
    )
  }
}

/// What the team server reports of the work under `claim`: its state and last report, that the claim went stale, or
/// that no work started under it yet.
fn event(ctx: &Ctx, claim: &TeamClaim, work: Option<ExecutionState>) -> Arc<str> {
  let (holder, time) = (claim.holder.name.as_str(), moment(ctx, claim.renewed));
  let Some(work) = work else {
    return if claim.holder.you {
      ctx.t("summary.team.run.yourOtherNotStarted")
    } else {
      ctx.t_args("summary.team.run.holderNotStarted", [("holder", holder)])
    };
  };
  if claim.stale && !claim.holder.you {
    return ctx.t_args("summary.team.run.stale", [("holder", holder), ("time", time.as_str())]);
  }
  let state = ctx.t(match work {
    ExecutionState::Running => "summary.team.run.state.running",
    ExecutionState::Paused => "summary.team.run.state.paused",
    ExecutionState::AwaitingHuman => "summary.team.run.state.awaitingHuman",
    ExecutionState::Succeeded => "summary.team.run.state.succeeded",
    ExecutionState::Failed => "summary.team.run.state.failed",
    ExecutionState::Cancelled => "summary.team.run.state.cancelled",
  });
  if claim.holder.you {
    ctx.t_args(
      "summary.team.run.yourOther",
      [("state", state.as_ref()), ("time", time.as_str())],
    )
  } else {
    ctx.t_args(
      "summary.team.run.holder",
      [("holder", holder), ("state", state.as_ref()), ("time", time.as_str())],
    )
  }
}

/// Why the team server fenced `run`, this computer's run of the task; `None` while it reports.
pub fn fence(task: &TaskContext, run: &RunView) -> Option<TeamFence> {
  let team = task.team.as_ref()?;
  team
    .task
    .fences
    .iter()
    .find(|(fenced, _)| *fenced == run.id())
    .map(|(_, fence)| *fence)
}

/// The step `run` stopped in: its last agent's or command's, else the first of its steps that did not pass, as its row
/// in *Runs* names it (*Canceled at Implement*).
pub fn stopped_in(run: &RunView) -> Option<&StepView> {
  run
    .invocations
    .last()
    .and_then(|invocation| run.step_of(invocation.execution))
    .or_else(|| {
      run
        .steps
        .iter()
        .find(|step| step.status.is_some_and(|status| status != StepStatus::Passed))
    })
}

/// *Run 8 stopped in 03 Implement. The team server refused its 2 unsent reports (claim lost); they stay here to read,
/// not sent again.*, for `run` once the server fenced it; `None` while it reports.
pub fn stopped(ctx: &Ctx, task: &TaskContext, run: &RunView) -> Option<Arc<str>> {
  let fence = fence(task, run)?;
  let team = task.team.as_ref()?;
  let unsent = team
    .task
    .unsent
    .iter()
    .find(|(fenced, _)| *fenced == run.id())
    .map_or(0, |(_, count)| *count);
  let step = stopped_in(run).map_or_else(
    || ctx.t("summary.team.stopped.noStep"),
    |step| {
      ctx.t_args(
        "summary.output.step",
        [
          ("number", format!("{:02}", step.number)),
          ("step", step.name.to_string()),
        ],
      )
    },
  );
  let why = ctx.t(match fence {
    TeamFence::ClaimLost | TeamFence::SessionMismatch => "summary.team.stopped.why.claimLost",
    TeamFence::MembershipRevoked => "summary.team.stopped.why.removed",
    TeamFence::Superseded => "summary.team.stopped.why.superseded",
  });
  let number = run.run.number().get().to_string();
  let key = match unsent {
    0 => "summary.team.stopped.none",
    1 => "summary.team.stopped.one",
    _ => "summary.team.stopped.other",
  };
  Some(ctx.t_args(
    key,
    [
      ("run", number),
      ("step", step.to_string()),
      ("count", unsent.to_string()),
      ("why", why.to_string()),
    ],
  ))
}
