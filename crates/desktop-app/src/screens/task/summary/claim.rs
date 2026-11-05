//! The claim line at the top of a team task's Summary (`02h · Claim line by state` `xUwYS`,
//! `docs/design/team-authority.md` §2–3): who holds the claim, on which computer, and when it was last renewed, as one
//! strip ([`crate::team::strip`]). Plain for a healthy claim, warning when it is not renewed or stale, danger when the
//! authority of this computer's run is lost. Its action is the one the state calls for, as the server's capabilities
//! allow it: *Claim* when nobody holds it, *Take over* a stale claim of someone else, *Continue* the run of a stale
//! claim of one's own. Only Running work renews a claim (D1): a run that waits for the person or is in review says so.
//! A claim is a computer's: the server names its holder, and this computer's own records say whether it holds it.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::{run::RunStatus, time::Timestamp};
use orchester_store::TeamFence;
use orchester_workspace::{files::Version, sharing::device, tasks::detail::RunView};

use super::Waiting;
use crate::{
  navigation::Navigation,
  routes::{FilesView, ProjectPage, Route},
  screens::{
    settings::orchester::moment,
    task::{
      TaskContext,
      header::controls::{Controls, Offer, RunControls, controls},
      team_controls::{TeamAsk, TeamControls, pipeline_name},
    },
  },
  team::{
    pages::{TeamClaim, minutes},
    strip::{Strip, StripTone},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// Where the task's claim stands for this computer.
enum Line<'a> {
  /// The server fenced this computer's latest run: its authority is lost.
  Lost(TeamFence, &'a RunView, Option<&'a TeamClaim>),
  /// Nobody holds it.
  Nobody,
  /// Its module's claim covers it.
  UnderModule(&'a TeamClaim),
  /// This computer holds it, and it went stale.
  YoursStale,
  /// This computer holds it, and its run waits for the person, or is in review: nothing renews it.
  NotRenewed(&'a TeamClaim, bool),
  /// This computer holds it; `running` while its running step renews it.
  Yours(&'a TeamClaim, bool),
  /// The person holds it on another computer.
  Elsewhere,
  /// Someone else holds it, stale.
  OtherStale(&'a TeamClaim),
  /// Someone else holds it.
  Other(&'a TeamClaim),
}

/// The line of `task`'s claim; `None` for a local task.
pub fn line(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let team = task.team.as_ref()?;
  let claim = team.task.claim.as_ref();
  let line = match (fenced(task), claim) {
    (Some((fence, run)), _) => Line::Lost(fence, run, claim),
    (None, None) => Line::Nobody,
    (None, Some(claim)) if claim.under_module => Line::UnderModule(claim),
    (None, Some(claim)) if claim.holder.you && claim.here => mine(task, claim),
    (None, Some(_)) if claim.is_some_and(|claim| claim.holder.you) => Line::Elsewhere,
    (None, Some(claim)) if claim.stale => Line::OtherStale(claim),
    (None, Some(claim)) => Line::Other(claim),
  };
  Some(strip(ctx, task, &line).into())
}

/// The latest run here, when the server fenced it, with why.
fn fenced(task: &TaskContext) -> Option<(TeamFence, &RunView)> {
  let run = task.detail.run.as_ref()?;
  let team = task.team.as_ref()?;
  let fence = team
    .task
    .fences
    .iter()
    .find(|(fenced, _)| *fenced == run.id())
    .map(|(_, fence)| *fence)?;
  Some((fence, run))
}

/// The line of a claim this computer holds.
fn mine<'a>(task: &'a TaskContext, claim: &'a TeamClaim) -> Line<'a> {
  if claim.stale {
    return Line::YoursStale;
  }
  let run = task.detail.run.as_ref().filter(|_| !task.lost());
  let running = run.is_some_and(|run| *run.run.status() == RunStatus::Running);
  let waits = task.waiting_approval().is_some()
    || run.is_some_and(|run| run.run.waits_for_a_person() || !run.waiting_questions.is_empty());
  match super::waiting(task) {
    Some(Waiting::Plan) => Line::NotRenewed(claim, false),
    Some(_) => Line::NotRenewed(claim, true),
    None if running && waits => Line::NotRenewed(claim, false),
    None => Line::Yours(claim, running),
  }
}

/// Whether this computer holds the task's claim and nothing renews it now, its run waiting for the person or in
/// review (D1): the claim goes stale at its `stale_at`.
pub fn unrenewed(task: &TaskContext) -> bool {
  let Some(claim) = task.team.as_ref().and_then(|team| team.task.claim.as_ref()) else {
    return false;
  };
  let mine_here = claim.holder.you && claim.here && !claim.under_module;
  mine_here && fenced(task).is_none() && matches!(mine(task, claim), Line::NotRenewed(..))
}

fn strip(ctx: &mut Ctx, task: &TaskContext, line: &Line<'_>) -> Strip {
  let key = task.key.to_string();
  let id = "task-team-claim";
  match line {
    Line::Lost(fence, run, claim) => lost(ctx, task, (*fence, run, *claim)),
    Line::Nobody => {
      let pipeline = pipeline_name(task);
      let detail = if pipeline.is_empty() {
        ctx.t("summary.team.claim.nobodyDetailNoPipeline")
      } else {
        ctx.t_args("summary.team.claim.nobodyDetail", [("pipeline", pipeline)])
      };
      let title = ctx.t_args("summary.team.claim.nobodyTitle", [("task", key.as_str())]);
      let strip = Strip::new(id, (Icon::CircleDashed, PaletteColor::TextMuted), title, detail);
      let acquire = task.team.as_ref().is_some_and(|team| team.task.abilities.acquire);
      match ask_button(
        ctx,
        acquire,
        ("task-team-claim-take", "summary.team.claim.claim", Icon::Hand),
        TeamAsk::Claim,
      ) {
        Some(button) => strip.action(button),
        None => strip,
      }
    }
    Line::UnderModule(claim) => {
      let module = task
        .team
        .as_ref()
        .and_then(|team| team.module.as_ref())
        .map(|module| module.module.details().name().as_str().to_owned())
        .unwrap_or_default();
      let holder = claim.holder.name.as_str();
      let title = ctx.t_args(
        "summary.team.claim.moduleTitle",
        [("module", module.as_str()), ("holder", holder)],
      );
      let detail = ctx.t_args(
        "summary.team.claim.moduleDetail",
        [("holder", holder), ("task", key.as_str())],
      );
      Strip::new(id, (Icon::Target, PaletteColor::TextSecondary), title, detail)
    }
    Line::YoursStale => {
      let detail = ctx.t_args("summary.team.claim.yoursStaleDetail", [("task", key.as_str())]);
      let strip = Strip::new(
        id,
        (Icon::ClockAlert, PaletteColor::Warning),
        ctx.t("summary.team.claim.yoursStaleTitle"),
        detail,
      )
      .tone(StripTone::Warning);
      match continue_button(ctx, task) {
        Some(button) => strip.action(button),
        None => strip,
      }
    }
    Line::NotRenewed(claim, review) => not_renewed(ctx, (&key, claim), *review),
    Line::Yours(claim, running) => yours(ctx, claim, *running),
    Line::Elsewhere => Strip::new(
      id,
      (Icon::Monitor, PaletteColor::TextSecondary),
      ctx.t("summary.team.claim.elsewhereTitle"),
      ctx.t("summary.team.claim.elsewhereDetail"),
    ),
    Line::OtherStale(claim) => other_stale(ctx, task, claim),
    Line::Other(claim) => {
      let (time, threshold) = (moment(ctx, claim.renewed), minutes(claim.renewed, claim.stale_at));
      let title = ctx.t_args(
        "summary.team.claim.otherTitle",
        [("holder", claim.holder.name.as_str())],
      );
      let detail = ctx.t_args(
        "summary.team.claim.otherDetail",
        [("time", time.as_str()), ("minutes", threshold.as_str())],
      );
      Strip::new(id, (Icon::User, PaletteColor::TextSecondary), title, detail)
    }
  }
}

/// This computer's claim while its work renews it, or before any run does.
fn yours(ctx: &Ctx, claim: &TeamClaim, running: bool) -> Strip {
  let id = "task-team-claim";
  let (time, threshold) = (moment(ctx, claim.renewed), minutes(claim.renewed, claim.stale_at));
  let (title, detail) = if running {
    let computer = device::computer_name();
    (
      ctx.t("summary.team.claim.hereTitle"),
      ctx.t_args(
        "summary.team.claim.hereDetail",
        [
          ("computer", computer.as_str()),
          ("time", &time),
          ("minutes", &threshold),
        ],
      ),
    )
  } else {
    (
      ctx.t("summary.team.claim.yoursTitle"),
      ctx.t_args(
        "summary.team.claim.detail",
        [("time", time.as_str()), ("minutes", &threshold)],
      ),
    )
  };
  Strip::new(id, (Icon::UserRoundCheck, PaletteColor::Success), title, detail)
}

/// This computer's claim that nothing renews while its run waits for the person or is in review (D1).
fn not_renewed(ctx: &Ctx, (task, claim): (&str, &TeamClaim), review: bool) -> Strip {
  let (stale_at, left) = (moment(ctx, claim.stale_at), minutes(Timestamp::now(), claim.stale_at));
  let (title, detail) = if review {
    (
      ctx.t("summary.team.claim.reviewTitle"),
      ctx.t_args(
        "summary.team.claim.reviewDetail",
        [("staleAt", stale_at.as_str()), ("left", left.as_str())],
      ),
    )
  } else {
    let renewed = moment(ctx, claim.renewed);
    (
      ctx.t("summary.team.claim.waitingTitle"),
      ctx.t_args(
        "summary.team.claim.waitingDetail",
        [
          ("time", renewed.as_str()),
          ("staleAt", &stale_at),
          ("left", &left),
          ("task", task),
        ],
      ),
    )
  };
  Strip::new(
    "task-team-claim",
    (Icon::Hourglass, PaletteColor::Warning),
    title,
    detail,
  )
  .tone(StripTone::Warning)
}

/// Someone else's stale claim, with *Take over* while the server allows it.
fn other_stale(ctx: &mut Ctx, task: &TaskContext, claim: &TeamClaim) -> Strip {
  let holder = claim.holder.name.as_str();
  let (age, threshold) = (
    minutes(claim.renewed, Timestamp::now()),
    minutes(claim.renewed, claim.stale_at),
  );
  let title = ctx.t_args("summary.team.claim.staleTitle", [("holder", holder)]);
  let detail = ctx.t_args(
    "summary.team.claim.staleDetail",
    [("age", age.as_str()), ("minutes", &threshold), ("holder", holder)],
  );
  let strip = Strip::new(
    "task-team-claim",
    (Icon::ClockAlert, PaletteColor::Warning),
    title,
    detail,
  )
  .tone(StripTone::Warning);
  let allowed = task.team.as_ref().is_some_and(|team| team.task.abilities.take_over);
  match take_over_button(ctx, allowed) {
    Some(button) => strip.action(button),
    None => strip,
  }
}

/// The authority of this computer's run is gone: taken over, moved to the person's other computer, ended by an Admin,
/// or the person is no longer a member. The run stopped; its worktree stays here.
fn lost(ctx: &mut Ctx, task: &TaskContext, (fence, run, claim): (TeamFence, &RunView, Option<&TeamClaim>)) -> Strip {
  let key = task.key.to_string();
  let number = run.run.number().get().to_string();
  let (glyph, title, detail) = match (fence, claim) {
    (TeamFence::MembershipRevoked, _) => {
      let team = task
        .team
        .as_ref()
        .and_then(|team| team.server.clone())
        .unwrap_or_default();
      (
        Icon::UserX,
        ctx.t_args("summary.team.claim.removedTitle", [("team", team)]),
        ctx.t_args("summary.team.claim.removedDetail", [("run", number.as_str())]),
      )
    }
    (_, claim) if fence == TeamFence::SessionMismatch || claim.is_some_and(|claim| claim.holder.you) => {
      let time = claim.map(|claim| moment(ctx, claim.acquired)).unwrap_or_default();
      (
        Icon::Monitor,
        ctx.t_args(
          "summary.team.claim.movedTitle",
          [("task", key.as_str()), ("time", time.as_ref())],
        ),
        ctx.t_args(
          "summary.team.claim.movedDetail",
          [("run", number.as_str()), ("task", key.as_str())],
        ),
      )
    }
    (_, Some(claim)) => {
      let (holder, time) = (claim.holder.name.as_str(), moment(ctx, claim.acquired));
      (
        Icon::UserX,
        ctx.t_args(
          "summary.team.claim.takenOverTitle",
          [("holder", holder), ("task", key.as_str()), ("time", time.as_ref())],
        ),
        ctx.t_args(
          "summary.team.claim.takenOverDetail",
          [("run", number.as_str()), ("task", key.as_str()), ("holder", holder)],
        ),
      )
    }
    (_, None) => (
      Icon::UserX,
      ctx.t("summary.team.claim.unassignedTitle"),
      ctx.t_args("summary.team.claim.unassignedDetail", [("run", number.as_str())]),
    ),
  };
  let strip = Strip::new("task-team-claim", (glyph, PaletteColor::Danger), title, detail).tone(StripTone::Danger);
  match open_worktree(ctx, task, run) {
    Some(button) => strip.action(button),
    None => strip,
  }
}

/// A strip's button that asks `ask` of the task, while `allowed`; under way while an ask is.
fn ask_button(
  ctx: &mut Ctx,
  allowed: bool,
  (id, label, glyph): (&'static str, &'static str, Icon),
  ask: TeamAsk,
) -> Option<ActionButton> {
  let controls = TeamControls::of(ctx).filter(|_| allowed)?;
  let button = ActionButton::new(id, ButtonKind::Secondary, ctx.t(label))
    .icon(glyph)
    .small();
  Some(if controls.pending() {
    button.loading()
  } else {
    button.on_click(move || controls.run(ask))
  })
}

/// *Take over*, which asks first (`02e`).
pub fn take_over_button(ctx: &mut Ctx, allowed: bool) -> Option<ActionButton> {
  let controls = TeamControls::of(ctx).filter(|_| allowed)?;
  let button = ActionButton::new(
    "task-team-take-over",
    ButtonKind::Secondary,
    ctx.t("summary.team.claim.takeOver"),
  )
  .icon(Icon::HandHelping)
  .small();
  Some(if controls.pending() {
    button.loading()
  } else {
    let open = controls.taking_over;
    button.on_click(move || open.set(true))
  })
}

/// *Continue*: the run control that moves this computer's run on, which renews the claim once it runs.
fn continue_button(ctx: &mut Ctx, task: &TaskContext) -> Option<ActionButton> {
  let run_controls = RunControls::of(ctx);
  let shown = controls(
    ctx,
    &Controls {
      detail: &task.detail,
      online: task.online,
      action: &run_controls.action,
      pending: run_controls.pending(),
      requested: &run_controls.requested,
      approval_waits: task.waiting_approval().is_some(),
      fenced: fenced(task).is_some(),
    },
  );
  let moves = shown.moves?;
  let button = ActionButton::new(
    "task-team-continue",
    ButtonKind::Secondary,
    ctx.t("summary.team.claim.continue"),
  )
  .icon(Icon::Play)
  .small();
  Some(match moves.offer {
    Offer::Ready(press) => button.press(press),
    Offer::Pending => button.loading(),
    Offer::Disabled => button.disabled(),
    Offer::Unavailable(reason) => button.unavailable(reason.full),
  })
}

/// *Open worktree* of the stopped run, whose worktree stays here as detached work: the code viewer at it, as *Open
/// worktree in Files* opens it, while it is there.
fn open_worktree(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> Option<ActionButton> {
  run.run.current_attempt()?;
  if run.worktree_removed.is_some() {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let files = FilesView::new(None, Version::Run(run.id()));
  let route = Route::Project(task.project.open.project.id(), ProjectPage::Files(files));
  let words: Arc<str> = ctx.t("summary.team.claim.openWorktree");
  Some(
    ActionButton::new("task-team-open-worktree", ButtonKind::Secondary, words)
      .icon(Icon::FolderOpen)
      .small()
      .opens(navigation.to(route)),
  )
}
