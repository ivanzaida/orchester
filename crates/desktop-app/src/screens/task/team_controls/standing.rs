//! What the page tells of a team task's start, and why (see [`start_standing`]).

use lurq::app::ctx::Ctx;

use super::{TeamAnswer, TeamAsk, TeamControls, can_run};
use crate::{
  screens::task::TaskContext,
  team::start::{Recorded, Started, Telling},
};

/// What the page tells of the task's start, and why.
pub struct StartStanding {
  pub ask: TeamAsk,
  pub started: Started,
  /// The notice tells a start that was settled for good and that the person has not dismissed: it offers *Dismiss*.
  pub dismiss: bool,
}

impl StartStanding {
  fn of(ask: TeamAsk, recorded: &Recorded) -> Self {
    Self {
      ask,
      started: recorded.started.clone(),
      dismiss: recorded.telling != Telling::Standing && matches!(recorded.started, Started::Stopped(_)),
    }
  }
}

/// What the page tells of the task's start: the person's last ask in this window, unless that said the start waits,
/// which recovery may have settled since, or there is none, as in a window opened after it did. Then the project's
/// record of the task's newest start tells ([`TeamTaskShown::last_start`]), for a task this computer can still run: a
/// start that waits, or that the runner refused, for as long as it does; one settled for good once, until the person
/// dismisses it, runs the task again or updates what made it fail ([`Telling`]).
pub fn start_standing(ctx: &mut Ctx, task: &TaskContext) -> Option<StartStanding> {
  let controls = TeamControls::of(ctx)?;
  if controls.pending() {
    return None;
  }
  let recorded = task.team.as_ref()?.last_start.as_ref();
  match controls.told() {
    Some(TeamAnswer::Acted(..)) => None,
    Some(TeamAnswer::Started(ask, Started::Waiting)) => match recorded {
      Some(Recorded {
        started: Started::Run(_),
        ..
      }) => None,
      Some(
        settled @ Recorded {
          started: Started::Stopped(_),
          ..
        },
      ) => Some(StartStanding::of(ask, settled)),
      Some(Recorded {
        started: Started::Waiting,
        ..
      })
      | None => Some(StartStanding {
        ask,
        started: Started::Waiting,
        dismiss: false,
      }),
    },
    Some(TeamAnswer::Started(ask, started)) => Some(StartStanding {
      ask,
      started,
      dismiss: false,
    }),
    None => recorded
      .filter(|recorded| {
        !matches!(recorded.started, Started::Run(_)) && recorded.telling != Telling::Told && can_run(task)
      })
      .map(|recorded| StartStanding::of(TeamAsk::Run, recorded)),
  }
}
