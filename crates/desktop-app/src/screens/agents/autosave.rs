//! Autosave of a profile's page (ORC-107): an edit saves the draft once the fields have rested, as instructions and
//! skills do. A save is held while a write is under way, one failed (its notice offers the retry), the page reads the
//! profiles again, or a question waits for the person; a widening is asked before anything widens is saved.
use std::time::Duration;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::{Ref, Signal},
};
use orchester_workspace::{
  agents::{
    data::ProjectAgents,
    editor::{Action, Request},
  },
  messages::Message,
};

use super::{
  fields::Fields,
  work::{Allowed, Pending, Saved, Work},
};

/// How long the fields rest after an edit before it is saved.
const REST: Duration = Duration::from_millis(700);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rested;
impl DevtoolsInspectable for Rested {}

/// What a rested save reads.
#[derive(Clone)]
pub struct Source {
  pub fields: Fields,
  pub saved: Signal<Saved>,
  pub pending: Signal<Pending>,
  pub allowed: Signal<Allowed>,
  pub work: Work,
}

#[derive(Clone)]
pub struct Autosave {
  rest: FutureAction<(), Rested, Message>,
  /// Whether the page reads the profiles again, or cannot write now: the save waits.
  pub held: Ref<bool>,
  /// The project's profiles and connections as last read, which the fields make a profile with.
  pub listed: Ref<Option<ProjectAgents>>,
}

impl Autosave {
  pub fn new(ctx: &mut Ctx, source: Source) -> Self {
    let held = ctx.create_ref(false);
    let listed = ctx.create_ref(None::<ProjectAgents>);
    let waiting = held.clone();
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
    source.fields.on_edit(ctx, {
      let rest = rest.clone();
      move || rest.run(())
    });
    let reading = listed.clone();
    ctx.watch(&rest.state(), move |state| {
      if state.is_fulfilled()
        && let Some(listed) = reading.get()
      {
        save(&source, &listed);
      }
    });
    Self { rest, held, listed }
  }

  /// Rests again, so that what was edited while a write was under way is saved after it.
  pub fn again(&self) {
    self.rest.run(());
  }
}

/// Saves the fields as the draft of the saved profile, unless they are invalid or unchanged, a write is under way or
/// failed, or a question waits.
fn save(source: &Source, listed: &ProjectAgents) {
  let writing = source.work.state().get_untracked();
  if writing.is_pending() || writing.is_rejected() || source.pending.get_untracked().0.is_some() {
    return;
  }
  let (Ok(profile), Some(entry)) = (source.fields.profile_untracked(listed), source.saved.get_untracked().0) else {
    return;
  };
  if profile != entry.profile {
    source.work.run(Request {
      existing: Some(entry),
      profile,
      action: Action::Save,
      allowed: source.allowed.get_untracked().0,
      prompt_decided: false,
    });
  }
}
