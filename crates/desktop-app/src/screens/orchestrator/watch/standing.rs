//! The line of an event of a target the conversation leads to the end
//! (`orchester_core::orchestrator::standing::StandingEvent`, `docs/plan/orchestrator-autonomy/OA-13-…`, `OA-15-…`,
//! `OA-20-…`; `Parts · Lines` `e6nsBS` b–f on the board `Orchestrator autonomy · frames to approve`), drawn as the
//! lines about what a conversation watches are; the turn it starts by itself answers under it.
//!
//! - *Work can start in Webhooks v2: KON-41, KON-42 — nothing they depend on is open*, a `circle-play` in `text_muted`,
//!   and its time.
//! - *Webhooks v2 is stuck: 3 left for you — nothing else can start* (*its budget is used* when that is why), a
//!   `circle-alert` in `warning`, and *Needs you ↗* in the time's place.
//! - *Every task of Webhooks v2 is done: its acceptance can start — Module acceptance* (the acceptance pipeline's name)
//!   and *Webhooks v2 is accepted: it can be delivered — module/webhooks-v2 into main*, each a `circle-play` with its
//!   time; what they name is the module as it is now, and a module that is gone adds nothing.
//! - *Every task of Webhooks v2 is done: its delivery waits for you — Deliver when accepted is off* (or why else it is
//!   the person's), a `circle-alert` in `warning`, and *Module ↗*.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::{
  id::ProjectId,
  module::delivery::PersonsDelivery,
  orchestrator::{
    WatchTarget,
    standing::{Raised, StandingEvent},
  },
  time::Timestamp,
};
use orchester_workspace::orchestrator::standing::{ModuleNow, event_line};

use super::{Said, line_row, link, time};
use crate::{
  navigation::Navigation,
  routes::{ModuleTab, OverviewTab, ProjectPage, Route},
  ui::{icon::Icon, message::message_text},
};

/// The line of `event`, of a module that is `now` as read, kept at `at` in `project`; `id` names it.
pub(in crate::screens::orchestrator) fn line(
  ctx: &mut Ctx,
  id: &str,
  (event, now): (&StandingEvent, Option<&ModuleNow>),
  (at, project): (Option<Timestamp>, ProjectId),
) -> Element {
  let starts = (Icon::CirclePlay, PaletteColor::TextMuted);
  let persons = (Icon::CircleAlert, PaletteColor::Warning);
  let (glyph, aside, place) = match &event.raised {
    Raised::CanStart { .. } => (starts, Some(ctx.t("orchestrator.standing.canStart")), None),
    Raised::Stuck { budget_used, .. } => {
      let aside = if *budget_used {
        "orchestrator.standing.stuckBudget"
      } else {
        "orchestrator.standing.stuck"
      };
      let needs_you = ProjectPage::Overview(OverviewTab::NeedsYou);
      let place = ("orchestrator.standing.needsYou", needs_you);
      (persons, Some(ctx.t(aside)), Some(place))
    }
    Raised::Accept => {
      let pipeline = now.and_then(|now| now.acceptance.as_deref());
      let aside = pipeline.map(|pipeline| ctx.t_args("orchestrator.standing.accept", [("pipeline", pipeline)]));
      (starts, aside, None)
    }
    Raised::Deliver => (starts, now.and_then(|now| delivered(ctx, now)), None),
    Raised::DeliveryWaits { why } => {
      let aside = match why {
        PersonsDelivery::NoAcceptance => "orchestrator.standing.noAcceptance",
        PersonsDelivery::AcceptanceCeiling => "orchestrator.standing.acceptanceCeiling",
        PersonsDelivery::NotAllowed => "orchestrator.standing.notAllowed",
      };
      let place = match event.target {
        WatchTarget::Module(module) => Some((
          "orchestrator.standing.module",
          ProjectPage::Module(module, ModuleTab::default()),
        )),
        WatchTarget::Task(_) | WatchTarget::WithoutModule | WatchTarget::Project => None,
      };
      (persons, Some(ctx.t(aside)), place)
    }
  };
  let end = match place {
    Some((words, page)) => {
      let open = Navigation::of(ctx).to(Route::Project(project, page));
      Some(link(&format!("{id}-open"), ctx.t(words), open))
    }
    None => time(id, at),
  };
  let said = Said {
    glyph,
    words: message_text(ctx, &event_line(event)),
    tag: None,
    aside,
  };
  line_row(id, "standing", said, end).into()
}

/// *— module/webhooks-v2 into main*: the branch that is delivered and where it goes; `None` for a module with no
/// branch of its own any more.
fn delivered(ctx: &Ctx, now: &ModuleNow) -> Option<Arc<str>> {
  let branch = now.branch.as_deref()?;
  Some(match now.base.as_deref() {
    Some(base) => ctx.t_args("orchestrator.standing.deliver", [("branch", branch), ("base", base)]),
    None => ctx.t_args("orchestrator.standing.deliverCheckout", [("branch", branch)]),
  })
}
