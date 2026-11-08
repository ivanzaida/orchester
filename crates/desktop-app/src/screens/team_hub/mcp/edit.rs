//! What an admin's edit of a team MCP server does in its panel (`Agents · MCP server · admin · draft` A13 `WMtxm`, A14
//! `gTLx6`, A16 `x3ywL`, A17 `rJbPm`): the team's part becomes a form of the fields the create form has, kept as the
//! resource's draft while it is typed, and *Publish for the team* sends the draft to the team server and says its
//! answer in place. A refusal keeps the draft and what was typed; waiting for a server that cannot be reached is not
//! one.

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureState},
  core::Signal,
};
use orchester_core::{
  id::{ProjectId, ResourceId, VersionId},
  time::Timestamp,
};
use orchester_workspace::{
  agents::{save_lane::SaveLane, team_mcp_edit::McpEdit},
  app_mcp::setup::NewTeamMcpServer,
  messages::Message,
  sharing::team_edit::{EditRefused, Edited},
};

use super::{
  edit_actions as actions,
  edit_save::{Answer, Save},
};
use crate::{screens::team_hub::create::McpFields, services::Services};

/// A press of *Publish for the team*: the fields as typed, whether a person (or automation they let decide) pressed,
/// and the version in the outbox that the team server refused, which goes back to being the draft first.
pub(super) type Publish = FutureAction<(NewTeamMcpServer, bool, Option<VersionId>), Edited, EditRefused>;

/// A press of *Discard draft*: the fields of the version the form shows again, and the refused version in the outbox
/// that goes back to being the draft first, in which case the form shows the version the team has.
pub(super) type Discard = FutureAction<(NewTeamMcpServer, Option<VersionId>), NewTeamMcpServer, Message>;

/// What the panel keeps of an edit.
pub(super) struct Edit {
  /// The form's fields; none for a server the form cannot hold.
  pub fields: Option<McpFields>,
  /// *Edit* was pressed, or a draft was kept: the team's part is a form.
  pub editing: Signal<bool>,
  /// Why the last *Publish for the team* did not publish, and when: until the next press, a discard or a cancel.
  pub refused: Signal<Option<(EditRefused, Timestamp)>>,
  pub publish: Publish,
  /// *Discard draft*, given the fields of the version it is edited from, which the form shows again.
  pub discard: Discard,
  /// *Cancel sending*: takes the version waiting in the outbox back as the draft.
  pub cancel: FutureAction<VersionId, (), Message>,
  /// *Cancel sending* of a new resource waiting in the outbox: takes it back as if never created.
  pub cancel_created: FutureAction<(), (), Message>,
  /// Why the last *Discard draft* or *Cancel sending* did nothing (it is being sent right now, the server numbered it
  /// meanwhile, a run uses it, the person is no admin any more): until the next press.
  pub failure: Signal<Option<Message>>,
  /// Orders this resource's saves of the draft ([`SaveLane`]), which *Publish for the team* joins. Shared with every
  /// panel of the resource, so a save of one that was closed is ordered against the next one's.
  lane: Arc<SaveLane>,
  /// This panel asked for a save of its form.
  asked: AtomicBool,
  /// The folder of the project, the project and the resource that are edited.
  target: (PathBuf, ProjectId, ResourceId),
}

impl Edit {
  pub(super) fn new(ctx: &mut Ctx, target: (PathBuf, ProjectId, ResourceId), edit: Option<&McpEdit>) -> Self {
    let services = Services::of(ctx);
    let fields = edit.map(|edit| {
      let start = edit.draft.as_ref().map_or(&edit.published, |draft| &draft.fields);
      McpFields::of(ctx, start)
    });
    let editing = ctx.signal(edit.is_some_and(|edit| edit.draft.is_some()));
    let (root, _, id) = &target;
    let lane = services.servers.save_lane(root, *id);
    let edit = Self {
      fields,
      editing,
      refused: ctx.signal(None),
      publish: actions::publish(ctx, &services, &target, &lane),
      discard: actions::discard(ctx, &services, &target, &lane),
      cancel: actions::cancel(ctx, &services, &target),
      cancel_created: actions::cancel_created(ctx, &services, &target),
      failure: ctx.signal(None),
      lane,
      asked: AtomicBool::new(false),
      target,
    };
    edit.follow(ctx);
    edit
  }

  /// Takes the answer of a press: published or waiting ends the form, a refusal is kept with its time.
  fn follow(&self, ctx: &mut Ctx) {
    let (editing, refused) = (self.editing.clone(), self.refused.clone());
    let clock = Services::of(ctx).clock;
    ctx.watch(&self.publish.state(), move |state| {
      if state.is_pending() {
        refused.set(None);
        // A refused version goes back to being the draft first: the form stays while that happens.
        editing.set(true);
      } else if state.is_fulfilled() {
        editing.set(false);
      } else if let Some(error) = state.error.clone() {
        refused.set(Some((error, clock.now())));
      }
    });
    let (editing, refused, fields) = (self.editing.clone(), self.refused.clone(), self.fields.clone());
    ctx.watch(&self.discard.state(), move |state| {
      if let Some(published) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        editing.set(false);
        refused.set(None);
        // The fields are the version's own again; a later edit starts from it.
        if let Some(fields) = &fields {
          fields.load(published);
        }
      }
    });
    let (editing, refused) = (self.editing.clone(), self.refused.clone());
    ctx.watch(&self.cancel.state(), move |state| {
      if state.is_fulfilled() {
        refused.set(None);
        editing.set(true);
      }
    });
    self.follow_failures(ctx);
  }

  /// Keeps why a *Discard draft* or a *Cancel sending* did nothing, which is told where the press was.
  fn follow_failures(&self, ctx: &mut Ctx) {
    let failure = self.failure.clone();
    ctx.watch(&self.discard.state(), {
      let failure = failure.clone();
      move |state| settle_failure(&failure, state)
    });
    ctx.watch(&self.cancel.state(), {
      let failure = failure.clone();
      move |state| settle_failure(&failure, state)
    });
    ctx.watch(&self.cancel_created.state(), move |state| {
      settle_failure(&failure, state)
    });
  }

  /// Keeps `input` as the draft while the form is open (`Draft saved` in A13), and tells the screens that read drafts
  /// when a draft appears or goes; nothing for `None`. `had_draft`: the panel's last read held a draft. `published`:
  /// the fields the form was opened with, which an input that still equals them needs no save for. Saves go through the
  /// resource's lane ([`SaveLane`]): only the newest one writes, so a save that is late never undoes a newer one,
  /// whichever panel started it. A save is on the blocking pool from the moment it starts ([`Save`]), so it writes
  /// even when this panel is closed first, and one the store refused is told and tried again.
  pub(super) fn autosave(
    &self,
    ctx: &mut Ctx,
    (had_draft, published): (bool, Option<&NewTeamMcpServer>),
    input: Option<NewTeamMcpServer>,
  ) -> Answer {
    let services = Services::of(ctx);
    // The form opens with nothing to save; once this panel asked for a save, one that goes back to the fields saves, to
    // drop a draft.
    let input = input.filter(|input| had_draft || published != Some(input) || self.asked.load(Ordering::SeqCst));
    self.asked.fetch_or(input.is_some(), Ordering::SeqCst);
    let (lane, target) = (self.lane.clone(), self.target.clone());
    let state = ctx
      .stream((had_draft, input), move |(had_draft, input), emitter| {
        let started = input.map(|input| {
          let shared = (lane.clone(), services.runtime.clone(), services.changes.clone());
          Save::start(shared, target.clone(), (had_draft, input))
        });
        async move {
          match started {
            Some((save, job)) => save.settle(job, &emitter).await,
            None => {
              emitter.emit(None);
            }
          }
        }
      })
      .state()
      .get();
    match state.error {
      Some(error) if !state.is_pending() => Err(error),
      _ => Ok(state.data.flatten()),
    }
  }
}

/// Starts a press with no failure to tell, and keeps why it ended in none.
fn settle_failure<T>(failure: &Signal<Option<Message>>, state: &FutureState<T, Message>) {
  if state.is_pending() {
    failure.set(None);
  } else if state.is_rejected() {
    failure.set(state.error.clone());
  }
}
