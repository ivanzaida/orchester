//! The run's last event as a plan limit stopped it, named after the account (`Stopped at Work's plan limit ·
//! checkpoint 7c1e9f2`) and saying when it resets: the card reads both, so it hands them to the Summary's run.

use lurq::app::ctx::Ctx;
use orchester_core::{account::AgentAccount, connection::plan_limits::LimitReading, time::Timestamp};
use orchester_workspace::{display, tasks::detail::RunView};

use super::card;
use crate::{
  screens::task::{PageState, TaskContext, summary::EventWords},
  ui::plan_window::when,
};

/// Hands the run's last event its words for `account`, whose CLI last reported `reading`, unless it has them.
pub fn publish(
  ctx: &mut Ctx,
  props: &TaskContext,
  run: &RunView,
  (account, reading): (&AgentAccount, Option<&LimitReading>),
) {
  let words = words(ctx, props, run, (account, reading));
  let event = PageState::of(ctx).event;
  if event.get().as_ref() != Some(&words) {
    event.set(Some(words));
  }
}

fn words(
  ctx: &Ctx,
  props: &TaskContext,
  run: &RunView,
  (account, reading): (&AgentAccount, Option<&LimitReading>),
) -> EventWords {
  let name = card::account_name(ctx, account);
  let checkpoint = run
    .run
    .last_checkpoint()
    .filter(|_| run.saved_at_pause && !props.lost())
    .map(|checkpoint| display::short_revision(checkpoint.revision().as_str()).to_owned());
  let title = match checkpoint {
    Some(checkpoint) => ctx.t_args(
      "decisions.planLimit.noticeTitle",
      [("account", name.as_str()), ("checkpoint", checkpoint.as_str())],
    ),
    None => ctx.t_args("decisions.planLimit.noticeTitleUnsaved", [("account", name.as_str())]),
  };
  let now = Timestamp::now();
  let step = run.stopped_step().map(|step| step.name.to_string()).unwrap_or_default();
  let agent = run
    .stopped_step()
    .and_then(|step| step.agent.as_deref())
    .unwrap_or_default()
    .to_owned();
  let body = match reading.and_then(|reading| reading.resets_at(now)) {
    Some(resets) => ctx.t_args(
      "decisions.planLimit.noticeBody",
      [
        ("step", step),
        ("account", name),
        ("time", when(ctx, resets, now, card::TIMES)),
        ("agent", agent),
      ],
    ),
    None => ctx.t_args(
      "decisions.planLimit.noticeBodyNoReset",
      [("step", step), ("account", name), ("agent", agent)],
    ),
  };
  EventWords {
    run: run.id(),
    sequence: run.run.last_sequence().get(),
    title,
    body,
  }
}
