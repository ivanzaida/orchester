//! Why auto mode's reviewer could not run (ORC-611), in the words a request's card that came to the person and
//! Project settings › Runs › Reviewer and Estimates say it with: its account's plan limit with when it resets, a
//! sign-in that is missing, a CLI that failed, no answer in time, an answer that could not be read. The reviewer's
//! health is read off the UI thread whenever `key` changes.

use std::{path::PathBuf, sync::Arc};

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::{
  run::review::{ReviewerHealth, ReviewerTrouble},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  display::{self, WEEKDAY_KEYS},
  messages::Message,
};

use crate::{services::Services, ui::message::message_text};

/// The reviewer's health as the window holds it.
#[derive(Clone, PartialEq)]
struct Read(ReviewerHealth);

impl DevtoolsInspectable for Read {}

/// What Orchester knows of the reviewer of the project at `root`, read again whenever `key` changes; `None` until it
/// is read, and when it cannot be (logged): nothing is said then.
pub(crate) fn health(ctx: &mut Ctx, root: PathBuf, key: String) -> Option<ReviewerHealth> {
  let workspace = Services::of(ctx).workspace;
  let state = ctx
    .future(key, move |_| {
      let (workspace, root) = (workspace.clone(), root.clone());
      async move {
        background::run(move || workspace.reviewer_health(&root))
          .await?
          .map(Read)
          .map_err(|error| {
            tracing::warn!(%error, "whether the reviewer can run cannot be read");
            error.message()
          })
      }
    })
    .state()
    .get();
  state.data.map(|Read(health)| health)
}

/// Why the reviewer could not run, as the end of a sentence: *its account reached its plan limit, which resets Thu
/// 09:00.*
pub(crate) fn reason(ctx: &Ctx, trouble: ReviewerTrouble, now: Timestamp) -> Arc<str> {
  match trouble {
    ReviewerTrouble::PlanLimit { resets: Some(at) } => {
      ctx.t_args("reviewer.trouble.planLimit", [("time", reset_time(ctx, at, now))])
    }
    ReviewerTrouble::PlanLimit { resets: None } => ctx.t("reviewer.trouble.planLimitNoReset"),
    ReviewerTrouble::NotSignedIn => ctx.t("reviewer.trouble.notSignedIn"),
    ReviewerTrouble::CliFailed => ctx.t("reviewer.trouble.cliFailed"),
    ReviewerTrouble::TimedOut => ctx.t("reviewer.trouble.timedOut"),
    ReviewerTrouble::Unreadable => ctx.t("reviewer.trouble.unreadable"),
  }
}

/// *17:40* today, *Thu 09:00* on another day, as Overview › Limits says a reset.
fn reset_time(ctx: &Ctx, at: Timestamp, now: Timestamp) -> String {
  let time = display::clock(at);
  if display::calendar_date(at) == display::calendar_date(now) {
    return time;
  }
  let weekday = message_text(ctx, &Message::new(WEEKDAY_KEYS[display::weekday(at)]));
  ctx
    .t_args("reviewer.day", [("weekday", weekday.to_string()), ("time", time)])
    .to_string()
}
