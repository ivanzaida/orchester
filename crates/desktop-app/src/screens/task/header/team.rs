//! What a team task's header offers and says (`Task header` `j8DakG` in 01c `Tb55G`, 02 `nMUwg`, 02d `lpji3`, 03
//! `hWXxr`): Run, which starts through the team server, before the task has a run here; *Pause run* as the primary
//! action while its run works here; *Take over* as the primary action of someone
//! else's stale claim, asked first; *Release claim* in *More actions* while this computer holds the claim and no run of
//! it works; and in place of the run's status line who has the task now once this computer's run lost its authority
//! (*Taken over by mara · Run 8 stopped · output kept on this computer*), or, while this computer has no run of the
//! task, what the server says of the work on another computer (*Running on mara's computer · last report 15:41*) or of
//! a stale claim (*Stale claim · mara · last renewed 15:20*). Each as the server's capabilities allow it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::v2::ExecutionState;
use orchester_store::TeamFence;

use super::controls::{Control, Offer};
use crate::{
  screens::{
    settings::orchester::moment,
    task::{
      TaskContext,
      team::TeamTaskShown,
      team_controls::{TeamAsk, TeamControls, can_run},
    },
  },
  team::pages::TeamClaim,
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::ButtonKind,
    icon::{Icon, icon},
    pressable::Press,
    status::{Status, StatusChip},
  },
};

/// *Run* of a team task that has no run here yet, as the control `id` saying `label`, which takes the claim when nobody
/// holds it; `None` for a local task and while it does not apply.
pub fn run(ctx: &mut Ctx, task: &TaskContext, (id, label): (&str, &str)) -> Option<Control> {
  if !can_run(task) {
    return None;
  }
  let controls = TeamControls::of(ctx)?;
  let offer = if controls.pending() {
    Offer::Pending
  } else if !task.online {
    Offer::Unavailable(super::controls::Reason {
      short: ctx.t("task.reasons.offline"),
      full: ctx.t("task.controls.offline"),
    })
  } else {
    Offer::Ready(Press::run(move || controls.run(TeamAsk::Run)))
  };
  Some(Control::new(ctx, (id, label, Icon::Play), ButtonKind::Primary, offer))
}

/// *Take over* someone else's stale claim, which asks first; `None` while it does not apply.
pub fn take_over(ctx: &mut Ctx, task: &TaskContext) -> Option<Control> {
  let team = task.team.as_ref()?;
  let claim = team.task.claim.as_ref()?;
  if !claim.stale || claim.holder.you || claim.under_module || !team.task.abilities.take_over {
    return None;
  }
  let controls = TeamControls::of(ctx)?;
  let offer = if controls.pending() {
    Offer::Pending
  } else {
    let open = controls.taking_over;
    Offer::Ready(Press::run(move || open.set(true)))
  };
  Some(Control::new(
    ctx,
    ("task-take-over", "summary.team.claim.takeOver", Icon::HandHelping),
    ButtonKind::Primary,
    offer,
  ))
}

/// *Pause run* as the primary action while a team task's run works here (`Primary action` in 01c `Tb55G`, a `pause`
/// on `accent`); `None` for a local task, whose Pause run stays in *More actions*, and while nothing pauses.
pub fn pause(task: &TaskContext, pause: Option<&Control>) -> Option<Control> {
  task.team.as_ref()?;
  let mut pause = pause?.clone();
  pause.kind = ButtonKind::Primary;
  pause.icon = Icon::Pause;
  Some(pause)
}

/// *Release claim* while this computer holds the task's own claim and no run of it works here.
pub fn release(ctx: &mut Ctx, task: &TaskContext) -> Option<Control> {
  let team = task.team.as_ref()?;
  let claim = team.task.claim.as_ref()?;
  let working = task
    .detail
    .run
    .as_ref()
    .is_some_and(|run| run.run.status().has_live_work());
  if !claim.here || claim.under_module || working || !team.task.abilities.release {
    return None;
  }
  let controls = TeamControls::of(ctx)?;
  let offer = if controls.pending() {
    Offer::Pending
  } else {
    Offer::Ready(Press::run(move || controls.run(TeamAsk::Release)))
  };
  Some(Control::new(
    ctx,
    ("task-release-claim", "task.team.release", Icon::LogOut),
    ButtonKind::Ghost,
    offer,
  ))
}

/// The status line while this computer's run lost its authority (`03` `hWXxr`), or while this computer has no run of
/// the task and someone else's claim holds it; `None` otherwise.
pub fn status_line(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let team = task.team.as_ref()?;
  let claim = team.task.claim.as_ref();
  let (status, words, about) = match (lost(ctx, task), claim) {
    (Some(lost), _) => lost,
    (None, Some(claim)) if task.detail.run.is_none() && !claim.here => elsewhere(ctx, team, claim)?,
    _ => return None,
  };
  let chip = match status {
    Some(status) => StatusChip::new("task-status", status).words(words).render(ctx),
    None => stopped_chip(&words),
  };
  Some(
    Row::new()
      .id("task-status-row")
      .width(Dimension::Pct(100.0))
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(chip)
      .child(
        Text::new(&about)
          .id("task-run-line")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .into(),
  )
}

/// What a status line says: the chip's status (`None` for a run stopped for good by the team server) and words, and
/// the line after it.
type Said = (Option<Status>, Arc<str>, Arc<str>);

/// The chip of a run the team server stopped for good: a 13 `circle-stop` and its `words` in 12/500, both in `danger`
/// (`Status line` in 03 `hWXxr`), which no run status names.
fn stopped_chip(words: &str) -> Element {
  Row::new()
    .id("task-status")
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("status", "stopped")
    .child(icon(Icon::CircleStop, IconSize::Base, PaletteColor::Danger).id("task-status-icon"))
    .child(
      Text::new(words)
        .id("task-status-label")
        .variant(TypographyStyle::Label)
        .color(PaletteColor::Danger)
        .nowrap(),
    )
    .into()
}

/// Why the team server fenced this computer's latest run of the task; `None` while it reports, and for a local task.
pub fn fence(task: &TaskContext) -> Option<TeamFence> {
  let team = task.team.as_ref()?;
  let run = task.detail.run.as_ref()?;
  team
    .task
    .fences
    .iter()
    .find(|(fenced, _)| *fenced == run.id())
    .map(|(_, fence)| *fence)
}

/// The status of this computer's run whose authority the server fenced: who has the task now, and that the run stopped
/// with its output kept here.
fn lost(ctx: &Ctx, task: &TaskContext) -> Option<Said> {
  let team = task.team.as_ref()?;
  let run = task.detail.run.as_ref()?;
  let fence = fence(task)?;
  let claim = team.task.claim.as_ref();
  let words = match (fence, claim) {
    (TeamFence::MembershipRevoked, _) => ctx.t("task.team.removed"),
    (_, Some(claim)) if fence == TeamFence::SessionMismatch || claim.holder.you => ctx.t("task.team.moved"),
    (_, Some(claim)) => ctx.t_args("task.team.takenOver", [("holder", claim.holder.name.as_str())]),
    (_, None) => ctx.t("task.team.unassigned"),
  };
  let about = ctx.t_args("task.team.stopped", [("run", run.run.number().get().to_string())]);
  Some((None, words, about))
}

/// What the server says of the work on the task while another computer holds its claim: Running there with when it
/// last reported, or that the claim went stale.
fn elsewhere(ctx: &Ctx, team: &TeamTaskShown, claim: &TeamClaim) -> Option<Said> {
  let holder = claim.holder.name.as_str();
  let time = moment(ctx, claim.renewed);
  if claim.stale {
    return Some((
      Some(Status::Interrupted),
      ctx.t("task.team.staleClaim"),
      ctx.t_args("task.team.lastRenewed", [("holder", holder), ("time", time.as_str())]),
    ));
  }
  if team.task.work != Some(ExecutionState::Running) {
    return None;
  }
  let words = if claim.holder.you {
    ctx.t("task.team.runningElsewhere")
  } else {
    ctx.t_args("task.team.runningOn", [("holder", holder)])
  };
  Some((
    Some(Status::Running),
    words,
    ctx.t_args("task.team.lastReport", [("time", time.as_str())]),
  ))
}
