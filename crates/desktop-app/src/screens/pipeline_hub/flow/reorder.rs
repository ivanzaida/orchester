//! What a card's place in the flow can be changed with, through the edit the panel holds ([`Edit`]): the core's moves
//! (`PipelineEdit::move_up`, `move_down`, `move_to`, `run_in_parallel_with`, `run_on_its_own`), adding a step in place,
//! duplicating and removing one. A move is tried on a copy of the edit first: a refused one changes nothing and says
//! why, and one that changes nothing does not start an edit.

use std::{collections::HashMap, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::{
  name::Name,
  resource::{MoveError, PipelineEdit, StepKey},
};
use orchester_workspace::pipelines::flow::{PipelineFlow, Stage};

use super::{
  state::{Lasts, Refused, Reorder},
  words::number,
};
use crate::screens::pipeline_hub::edit::Edit;

/// What the changes of the flow are made with: the edit the panel holds, the state it was drawn from, and the flow's
/// own state, which a refusal and the focus are kept in.
#[derive(Clone)]
pub(super) struct Drive {
  pub reorder: Reorder,
  pub edit: Edit,
  pub current: Arc<PipelineEdit>,
}

impl Drive {
  /// Whether the flow takes no changes now. The cards offer none while it is locked; this stops what a handler made
  /// before the lock (a drag lifted before the save began) from changing the edit under the save.
  pub(super) fn locked(&self) -> bool {
    *self.reorder.locked.lock()
  }

  /// Moves step `key` by `change` from its menu or the keys, which a refusal says why under the card while the pointer
  /// or the keys are on it; `keys` hands the focus back to the card.
  pub(super) fn apply(&self, key: &StepKey, change: &Move, keys: bool) {
    self.make(key, change, Lasts::OnCard);
    if keys && !self.locked() {
      *self.reorder.refocus.lock() = Some(key.clone());
    }
  }

  /// Drops step `key`, whose handle was let go, where `change` puts it; a refusal stays under the card until the next
  /// change, since the pointer was let go away from it.
  pub(super) fn land(&self, key: &StepKey, change: &Move) {
    self.make(key, change, Lasts::NextChange);
  }

  fn make(&self, key: &StepKey, change: &Move, lasts: Lasts) {
    if self.locked() {
      return;
    }
    match make(&self.edit, &self.current, key, change) {
      Ok(_) => self.reorder.refused.set(None),
      Err(error) => self.reorder.refused.set(Some(Refused {
        key: Some(key.clone()),
        error,
        lasts,
      })),
    }
  }

  /// Adds a step after `after`'s stage, or at the end, which `flow` says what follows; returns the step added.
  pub(super) fn add(&self, name: Name, after: Option<&StepKey>, flow: &PipelineFlow) -> Option<StepKey> {
    if self.locked() {
      return None;
    }
    let stage = after.and_then(|after| stage_of(flow, after));
    let together: Vec<StepKey> = stage
      .map(|stage| stage.cards.iter().map(|card| card.key.clone()).collect())
      .unwrap_or_default();
    let next = after.and_then(|after| next_after(flow, after));
    let outcome = add_after(&self.edit, &self.current, (&together, next.as_ref()), name);
    let added = outcome.as_ref().ok().cloned();
    self.settle(outcome.map(|_| ()), after);
    added
  }

  /// Copies step `key`; the copy is called `name` when the step's own name leaves room for it. Returns the copy.
  pub(super) fn duplicate(&self, key: &StepKey, name: Option<Name>) -> Option<StepKey> {
    if self.locked() {
      return None;
    }
    let mut copied = (*self.current).clone();
    let outcome = match copy_of(&mut copied, key, name) {
      Some(copy) => {
        self.edit.change(|held| *held = copied);
        Ok(copy)
      }
      None => Err(MoveError::UnknownStep(key.clone())),
    };
    let copy = outcome.as_ref().ok().cloned();
    self.settle(outcome.map(|_| ()), Some(key));
    copy
  }

  /// Removes step `key`, which no other step returns to on failure ([`returned_to_by`]).
  pub(super) fn remove(&self, key: &StepKey) {
    if self.locked() || returned_to_by(&self.current, key).is_some() {
      return;
    }
    self.reorder.refused.set(None);
    self.edit.change(|held| {
      held.remove_step(key);
    });
  }

  fn settle(&self, outcome: Result<(), MoveError>, key: Option<&StepKey>) {
    match outcome {
      Err(error) => self.reorder.refused.set(Some(Refused {
        key: key.cloned(),
        error,
        lasts: Lasts::OnCard,
      })),
      Ok(()) => self.reorder.refused.set(None),
    }
  }
}

fn stage_of<'a>(flow: &'a PipelineFlow, key: &StepKey) -> Option<&'a Stage> {
  flow
    .stages
    .iter()
    .find(|stage| stage.cards.iter().any(|card| &card.key == key))
}

/// The first step of the stage after the one `key` is in.
fn next_after(flow: &PipelineFlow, key: &StepKey) -> Option<StepKey> {
  let index = flow
    .stages
    .iter()
    .position(|stage| stage.cards.iter().any(|card| &card.key == key))?;
  flow
    .stages
    .get(index + 1)
    .and_then(|stage| stage.cards.first())
    .map(|card| card.key.clone())
}

/// A step that returns to step `key` on failure, which keeps `key` from being removed: the route would point at a step
/// that is gone and the steps could not be drawn as one line. Removing it is refused, not guessed at by dropping the
/// route.
pub(super) fn returned_to_by(current: &PipelineEdit, key: &StepKey) -> Option<StepKey> {
  current
    .steps()
    .iter()
    .find(|step| step.key != *key && step.on_failure.as_ref().is_some_and(|route| &route.return_to == key))
    .map(|step| step.key.clone())
}

/// A change of a step's place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Move {
  Up,
  Down,
  OnItsOwn,
  /// Runs beside this step, and the steps that run beside it.
  Beside(StepKey),
  /// Lands just before this step, or at the end of the flow.
  Before(Option<StepKey>),
}

impl Move {
  fn apply(&self, edit: &mut PipelineEdit, key: &StepKey) -> Result<bool, MoveError> {
    match self {
      Self::Up => edit.move_up(key),
      Self::Down => edit.move_down(key),
      Self::OnItsOwn => edit.run_on_its_own(key),
      Self::Beside(neighbour) => edit.run_in_parallel_with(key, neighbour),
      Self::Before(before) => edit.move_to(key, before.as_ref()),
    }
  }
}

/// Whether `change` would change the steps of `current`, or why it is refused.
pub(super) fn tried(current: &PipelineEdit, key: &StepKey, change: &Move) -> Result<bool, MoveError> {
  change.apply(&mut current.clone(), key)
}

/// Makes `change` to step `key` in the edit, which `current` is the state of; whether anything changed, or why it is
/// refused. A move that changes nothing leaves the edit as it is.
pub(super) fn make(edit: &Edit, current: &PipelineEdit, key: &StepKey, change: &Move) -> Result<bool, MoveError> {
  let mut moved = current.clone();
  if !change.apply(&mut moved, key)? {
    return Ok(false);
  }
  edit.change(|held| *held = moved);
  Ok(true)
}

/// Adds a step after the steps of one stage, `together` (none for the end of the flow), in series with what follows:
/// the core adds it waiting for one step only, which inside a parallel group is a branch no single line shows, so it
/// waits for the whole stage before it is moved. The flow's first step after the stage is `next`.
pub(super) fn add_after(
  edit: &Edit,
  current: &PipelineEdit,
  (together, next): (&[StepKey], Option<&StepKey>),
  name: Name,
) -> Result<StepKey, MoveError> {
  let mut added = current.clone();
  let key = added.add_step(name, together.last());
  for member in together {
    added.set_dependency(&key, member, true);
  }
  added.move_to(&key, next)?;
  edit.change(|held| *held = added);
  Ok(key)
}

/// The copy of step `key`, named `name` when it has one, so that it can be told from the step; `None` when the edit has
/// no such step. The copy runs right after the step, in series (D-023: a copy reads as "do this again after"; the
/// core's `PipelineEdit::duplicate_after`). The card's menu and the open step's head both copy through this.
pub(in crate::screens::pipeline_hub) fn copy_of(
  edit: &mut PipelineEdit,
  key: &StepKey,
  name: Option<Name>,
) -> Option<StepKey> {
  let copy = edit.duplicate_after(key)?;
  if let Some(name) = name {
    edit.update(&copy, |step| step.name = name);
  }
  Some(copy)
}

/// What a copy of the step called `name` is called (*Research copy*), or none when `name` leaves no room for the word,
/// which keeps the step's own name.
pub(in crate::screens::pipeline_hub) fn copy_name(ctx: &Ctx, name: &str) -> Option<Name> {
  // A name at the limit cannot be extended; its copy keeps the name and is told apart by its number.
  Name::new(&ctx.t_args("pipeline_hub.copyName", [("name", name)])).ok()
}

/// The number and name of each step as the flow shows them, for the words that name steps.
#[derive(Clone, Debug, Default)]
pub(super) struct Cards(HashMap<StepKey, (usize, Arc<str>)>);

impl Cards {
  pub(super) fn of(flow: &PipelineFlow) -> Self {
    Self(
      flow
        .stages
        .iter()
        .flat_map(|stage| &stage.cards)
        .map(|card| (card.key.clone(), (card.number, card.name.clone())))
        .collect(),
    )
  }

  pub(super) fn number(&self, key: &StepKey) -> Option<usize> {
    self.0.get(key).map(|(number, _)| *number)
  }

  pub(super) fn name(&self, key: &StepKey) -> Arc<str> {
    self
      .0
      .get(key)
      .map_or_else(|| Arc::from(key.as_str()), |(_, name)| name.clone())
  }

  /// *02 Plan*.
  pub(super) fn label(&self, ctx: &Ctx, key: &StepKey) -> Arc<str> {
    ctx.t_args(
      "pipeline_hub.flow.move.step",
      [
        ("number", self.number(key).map_or_else(String::new, number)),
        ("name", self.name(key).to_string()),
      ],
    )
  }

  /// What steps that run together are called: *02 Plan* for one step, *04 and 05* for several.
  pub(super) fn group(&self, ctx: &Ctx, keys: &[StepKey]) -> Arc<str> {
    let numbers: Vec<String> = keys
      .iter()
      .map(|key| self.number(key).map_or_else(String::new, number))
      .collect();
    match (keys, numbers.as_slice()) {
      ([only], _) => self.label(ctx, only),
      (_, [first, second]) => ctx.t_args(
        "pipeline_hub.flow.move.two",
        [("first", first.as_str()), ("second", second.as_str())],
      ),
      (_, [head @ .., last]) => ctx.t_args(
        "pipeline_hub.flow.move.several",
        [("head", head.join(", ")), ("last", last.clone())],
      ),
      (_, []) => Arc::from(""),
    }
  }

  /// Why `error` refused a move of step `moved` (none for a step added), as a sentence that names the moved step as
  /// *it*, so a step that appears in the move's words once is not named twice.
  pub(super) fn reason(&self, ctx: &Ctx, error: &MoveError, moved: Option<&StepKey>) -> Arc<str> {
    match error {
      MoveError::UnknownStep(_) => ctx.t("pipeline_hub.flow.move.reason.unknownStep"),
      MoveError::WithItself(_) => ctx.t("pipeline_hub.flow.move.reason.withItself"),
      MoveError::Broken(_) => ctx.t("pipeline_hub.flow.move.reason.broken"),
      MoveError::RetryTargetNotUpstream { step, target } => {
        let (key, args) = match moved {
          Some(moved) if moved == target => (
            "pipeline_hub.flow.move.reason.returnsToIt",
            [("step", self.label(ctx, step).to_string())],
          ),
          Some(moved) if moved == step => (
            "pipeline_hub.flow.move.reason.itReturnsTo",
            [("target", self.label(ctx, target).to_string())],
          ),
          _ => {
            return ctx.t_args(
              "pipeline_hub.flow.move.reason.returnsTo",
              [
                ("step", self.label(ctx, step).to_string()),
                ("target", self.label(ctx, target).to_string()),
              ],
            );
          }
        };
        ctx.t_args(key, args)
      }
    }
  }
}
