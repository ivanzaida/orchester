//! *Stop turn* (ORC-92) as the scheduler takes it: the request goes to the task executing the run
//! (`crate::execution::executor::turn_stop`); a run no task executes has no turn to stop.

use orchester_core::id::RunId;
use orchester_ipc::Rejection;

use super::Scheduler;
use crate::execution::executor::control::Control;

impl Scheduler {
  /// Stops the turn of the agents at work in `run`. Returns once the stop is asked; a run no task executes has no
  /// agent at work.
  pub async fn stop_turn(&self, run: RunId) -> Result<(), Rejection> {
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      let Some(answer) = self.ask(run, Control::StopTurn) else {
        return Err(Rejection::NoTurnToStop);
      };
      drop(one_at_a_time);
      if let Ok(result) = answer.await {
        return result;
      }
    }
  }
}
