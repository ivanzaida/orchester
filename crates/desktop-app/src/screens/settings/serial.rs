//! The saves of a row that saves as the person types or presses: one at a time, the newest last. A save that runs is
//! never cancelled (its write goes on off the UI thread whatever happens to its result), so a second save started
//! beside it could reach the database first and leave the older value stored. A value asked for while a save runs
//! waits, replacing any older one that waits, and is saved when the running save ends.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureState},
  core::{Signal, signal::SignalValue},
};
use parking_lot::Mutex;

use super::save_state::Outcome;

/// Clones share the saves.
pub struct Serial<A, T: SignalValue, E: SignalValue> {
  action: FutureAction<A, T, E>,
  /// The newest value asked for while a save ran.
  waiting: Arc<Mutex<Option<A>>>,
}

impl<A, T: SignalValue, E: SignalValue> Clone for Serial<A, T, E> {
  fn clone(&self) -> Self {
    Self {
      action: self.action.clone(),
      waiting: Arc::clone(&self.waiting),
    }
  }
}

impl<A, T, E> Serial<A, T, E>
where
  A: Clone + Send + Sync + 'static,
  T: SignalValue + Clone + PartialEq + Send + Sync + 'static,
  E: SignalValue + Clone + PartialEq + Send + Sync + 'static,
{
  /// Saves through `action`, which only this saver may run.
  pub fn new(ctx: &mut Ctx, action: FutureAction<A, T, E>) -> Self {
    let waiting = Arc::<Mutex<Option<A>>>::default();
    let (runner, next) = (action.clone(), Arc::clone(&waiting));
    ctx.watch(&action.state(), move |state: &FutureState<T, E>| {
      if state.is_pending() {
        return;
      }
      let Some(value) = next.lock().take() else {
        return;
      };
      if !runner.run_if_idle(value.clone()) {
        // A newer value asked for meanwhile goes first.
        next.lock().get_or_insert(value);
      }
    });
    Self { action, waiting }
  }

  /// Saves `value` now, or once the running save ends.
  pub fn save(&self, value: A) {
    if !self.action.run_if_idle(value.clone()) {
      *self.waiting.lock() = Some(value);
    }
  }

  /// The last save's state; reading it follows its changes.
  pub fn state(&self) -> Signal<FutureState<T, E>> {
    self.action.state()
  }

  /// What the saves did, as of `state`: a value still waiting is being saved.
  pub fn outcome(&self, state: &FutureState<T, E>) -> Outcome {
    if self.waiting.lock().is_some() {
      Outcome::Saving
    } else {
      Outcome::of(state)
    }
  }

  /// Whether a save runs or a value waits for one.
  pub fn busy(&self, state: &FutureState<T, E>) -> bool {
    state.is_pending() || self.waiting.lock().is_some()
  }
}
