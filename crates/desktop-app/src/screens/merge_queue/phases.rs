//! The phases of a run's delivery on one line (`Queue phase` `ED8Ct` in `Detail` of `kcUhy`): *Synced with main ›
//! Check on the merged result › Your approval › Merge into main*, each a 12 glyph and an 11.5 word, 5 apart, with an
//! 11 chevron between them, 6 apart. Done is a `success` check, under way a turning loader in `text_primary` with a
//! 500 word, not yet a dashed circle in `text_muted`, held a `warning` pause. A check under way says how long it has
//! run, counted while it shows. Shared by the Merge queue screen and the task page's delivery card.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::PaletteColor,
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  display::short_revision,
  merge_queue::{PhaseState, RowPhase},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// How often a check under way says again how long it has run.
const TICK: std::time::Duration = std::time::Duration::from_secs(1);

/// What the phases' words name: the base branch the run delivers into and the integrator profile, when one is chosen.
#[derive(Clone, Copy)]
pub(crate) struct Named<'a> {
  pub base: &'a str,
  pub integrator: Option<&'a str>,
}

/// `phases` on one line, or nothing when there are none.
pub(crate) fn phases(ctx: &mut Ctx, id: &str, (phases, named): (&[RowPhase], Named<'_>)) -> Option<Element> {
  if phases.is_empty() {
    return None;
  }
  let mut line = Row::new()
    .id(format!("{id}-phases"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .wrap();
  for (index, phase) in phases.iter().enumerate() {
    if index > 0 {
      line = line.child(icon(Icon::ChevronRight, IconSize::Xs, PaletteColor::TextMuted));
    }
    let around = (
      index.checked_sub(1).map(|before| &phases[before]),
      phases.get(index + 1),
    );
    line = line.child(phase_item(ctx, &format!("{id}-phase-{index}"), named, (phase, around)));
  }
  Some(line.into())
}

/// The phases before and after one, which a re-sync's words name.
type Around<'a> = (Option<&'a RowPhase>, Option<&'a RowPhase>);

fn phase_item(ctx: &mut Ctx, id: &str, named: Named<'_>, (phase, around): (&RowPhase, Around<'_>)) -> Element {
  let base = named.base;
  match phase {
    RowPhase::Sync {
      state,
      base: synced,
      conflicts,
    } => {
      let label = match (state, synced, around) {
        // Syncing again after a check the base's move superseded: with the head it moved to.
        (PhaseState::Active, _, (Some(RowPhase::Superseded { now, .. }), _)) => {
          synced_with(ctx, base, now.as_str(), true)
        }
        (PhaseState::Active, ..) => ctx.t_args("delivery.queue.phase.syncing", [("base", base)]),
        (_, Some(synced), (_, Some(RowPhase::Superseded { .. }))) => synced_with(ctx, base, synced.as_str(), false),
        (_, Some(_), _) if *conflicts > 0 => ctx.t_args(
          "delivery.queue.phase.syncedConflicts",
          [("base", base.to_owned()), ("count", conflicts.to_string())],
        ),
        _ => ctx.t_args("delivery.queue.phase.synced", [("base", base)]),
      };
      item(id, *state, &label)
    }
    RowPhase::Superseded { .. } => superseded(ctx, id),
    RowPhase::Integrate { state } => {
      let label = named.integrator.map_or_else(
        || ctx.t("delivery.queue.phase.integrateStep"),
        |integrator| ctx.t_args("delivery.queue.phase.integrate", [("integrator", integrator)]),
      );
      item(id, *state, &label)
    }
    RowPhase::Check {
      state: PhaseState::Active,
      since: Some(since),
    } => ctx.mount_keyed::<RunningCheck>(
      id,
      RunningProps {
        id: id.into(),
        since: *since,
      },
    ),
    RowPhase::Check { state, .. } => {
      let key = if *state == PhaseState::Done {
        "delivery.queue.phase.checkPassed"
      } else {
        "delivery.queue.phase.check"
      };
      item(id, *state, &ctx.t(key))
    }
    RowPhase::Approval { state, at } => {
      let label = match at {
        Some(at) if *state == PhaseState::Done => ctx.t_args(
          "delivery.queue.phase.approvalAt",
          [("time", orchester_workspace::display::clock(*at))],
        ),
        _ => ctx.t("delivery.queue.phase.approval"),
      };
      item(id, *state, &label)
    }
    RowPhase::Merge { state } => {
      let key = if *state == PhaseState::Blocked {
        "delivery.queue.phase.mergeBlocked"
      } else {
        "delivery.queue.phase.merge"
      };
      item(id, *state, &ctx.t_args(key, [("base", base)]))
    }
  }
}

/// A sync around a re-sync, by the base's head: the one before the superseded check, or the one under way (`active`).
fn synced_with(ctx: &Ctx, base: &str, revision: &str, active: bool) -> Arc<str> {
  let key = if active {
    "delivery.queue.phase.syncingAt"
  } else {
    "delivery.queue.phase.syncedAt"
  };
  ctx.t_args(
    key,
    [
      ("base", base.to_owned()),
      ("revision", short_revision(revision).to_owned()),
    ],
  )
}

fn superseded(ctx: &Ctx, id: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("state", "superseded")
    .child(icon(Icon::CircleSlash, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("delivery.queue.phase.superseded"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// A phase's glyph and word in its state's colors.
fn item(id: &str, state: PhaseState, label: &str) -> Element {
  let (glyph, color, word, strong) = match state {
    PhaseState::Done => (
      Icon::CircleCheck,
      PaletteColor::Success,
      PaletteColor::TextSecondary,
      false,
    ),
    PhaseState::Active => (
      Icon::LoaderCircle,
      PaletteColor::TextPrimary,
      PaletteColor::TextPrimary,
      true,
    ),
    PhaseState::Pending => (
      Icon::CircleDashed,
      PaletteColor::TextMuted,
      PaletteColor::TextMuted,
      false,
    ),
    PhaseState::Blocked => (Icon::CirclePause, PaletteColor::Warning, PaletteColor::Warning, false),
  };
  let style = if strong {
    ExtraTypography::NoteStrong.role()
  } else {
    ExtraTypography::Note.role()
  };
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("state", state_name(state))
    .child(icon(glyph, IconSize::Sm, color))
    .child(Text::new(label).variant(style).color(word))
    .into()
}

const fn state_name(state: PhaseState) -> &'static str {
  match state {
    PhaseState::Done => "done",
    PhaseState::Active => "active",
    PhaseState::Pending => "pending",
    PhaseState::Blocked => "blocked",
  }
}

#[derive(Clone, PartialEq)]
struct RunningProps {
  id: Arc<str>,
  since: Timestamp,
}

impl DevtoolsInspectable for RunningProps {}

/// *Check on the merged result · 1 min 12 s*, counted every second while it shows.
struct RunningCheck {
  tick: Signal<u64>,
  _interval: Interval,
}

impl Component for RunningCheck {
  type Props = RunningProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    interval.start();
    Self {
      tick,
      _interval: interval,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    self.tick.get();
    let RunningProps { id, since } = ctx.props::<RunningProps>().clone();
    let elapsed = super::words::elapsed(ctx, since, Timestamp::now());
    let label = ctx.t_args("delivery.queue.phase.checkRunning", [("elapsed", elapsed)]);
    item(&id, PhaseState::Active, &label)
  }
}
