//! Autosave of the editor ilieH: an edit saves the draft once the fields have rested, started from the
//! edit and the end of the rest rather than from a render.
use std::time::Duration;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::{Ref, Signal},
};
use orchester_workspace::messages::Message;

use super::{
  fields::Fields,
  state::Saved,
  work::{Change, Work},
};

/// How long the fields rest after an edit before it is saved.
const REST: Duration = Duration::from_millis(700);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rested;
impl DevtoolsInspectable for Rested {}

#[derive(Clone)]
pub struct Autosave {
  rest: FutureAction<(), Rested, Message>,
  /// Whether the screen is reading the project's instructions again: the save waits until it has.
  pub reading: Ref<bool>,
}

impl Autosave {
  pub fn new(ctx: &mut Ctx, fields: &Fields, saved: Signal<Saved>, work: Work, published: Signal<Option<u32>>) -> Self {
    let reading = ctx.create_ref(false);
    let waiting = reading.clone();
    let rest = ctx.future_action(move |()| {
      let waiting = waiting.clone();
      async move {
        tokio::time::sleep(REST).await;
        while waiting.get() {
          tokio::time::sleep(REST).await;
        }
        Ok::<_, Message>(Rested)
      }
    });
    // Every edit of what the draft is made of starts the rest again.
    let restart = rest.clone();
    ctx.watch(&fields.text, move |_| restart.run(()));
    let restart = rest.clone();
    ctx.watch(&fields.targets, move |_| restart.run(()));
    let restart = rest.clone();
    ctx.watch(&fields.events, move |_| restart.run(()));
    let restart = rest.clone();
    ctx.watch(&fields.scope, move |_| restart.run(()));
    let fields = fields.clone();
    ctx.watch(&rest.state(), move |state| {
      if state.is_fulfilled() {
        save(&fields, &saved, &work, &published);
      }
    });
    Self { rest, reading }
  }

  /// Rests again, so that what was edited while a write was in flight is saved after it.
  pub fn again(&self) {
    self.rest.run(());
  }
}

/// Saves the fields as the draft, unless they are invalid, unchanged, or a write is in flight or failed
/// (its notice offers the retry).
fn save(fields: &Fields, saved: &Signal<Saved>, work: &Work, published: &Signal<Option<u32>>) {
  let writing = work.action.state().get_untracked();
  if writing.is_pending() || writing.is_rejected() {
    return;
  }
  let (Ok(body), Some(entry)) = (fields.instruction(), saved.get_untracked().0) else {
    return;
  };
  if body != entry.body {
    published.set(None);
    work.run(Change::Save(entry, body).into());
  }
}
