//! The cards of a run's services in its Conversation, from the runner's notes (`Agent testing · redesign` `eWydp`;
//! `ui::service_card` draws their parts): a service that crashed after it was ready ([`crash`], `FwHrv`), a start
//! that failed because Docker was not available or a service was not ready in time ([`failed`], 10a and 10c of
//! `oXVuM`), and a start that waits for *Services at once* slots ([`slots`], 10d). Each says what happened and what
//! Orchester did, and offers what the person can do next ([`actions`]).

mod actions;
mod crash;
mod failed;
mod slots;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::connection::stream::RunnerProblem;
use orchester_workspace::tasks::transcript::{Entry, EntryKind};

use super::Timeline;

/// The card of `entry`, the row `key`, when it is a note about the run's services.
pub(super) fn card(ctx: &mut Ctx, timeline: &Timeline, key: &Arc<str>, entry: &Entry) -> Option<Element> {
  let EntryKind::Problem(problem) = &entry.kind else {
    return None;
  };
  let id = format!("conversation-service-{key}");
  match problem {
    RunnerProblem::ServiceCrashed(crash) => Some(crash::card(ctx, timeline, &id, (crash, entry.at))),
    RunnerProblem::ServiceStartFailed(failure) => Some(failed::card(ctx, timeline, &id, (&**failure, entry.at))),
    RunnerProblem::WaitingForServiceSlots(wait) => Some(slots::card(ctx, timeline, &id, wait, entry)),
    _ => None,
  }
}

/// `names` as one list, as the conversation lists names.
fn listed(ctx: &Ctx, names: &[String]) -> String {
  names.join(&*ctx.t("conversation.list.separator"))
}

/// The actions of the card `id` about `service`, saying `labels`, while the timeline has a run.
fn actions(
  ctx: &mut Ctx,
  timeline: &Timeline,
  (id, service): (&str, &str),
  (labels, log): ((Arc<str>, Arc<str>), actions::LogPlace),
) -> Option<Element> {
  let run = timeline.detail.run.as_ref()?.id();
  let props = actions::ActionsProps {
    id: id.into(),
    root: timeline.chat_project.root.clone(),
    run,
    service: service.into(),
    labels,
    log,
  };
  Some(actions::actions(ctx, props))
}
