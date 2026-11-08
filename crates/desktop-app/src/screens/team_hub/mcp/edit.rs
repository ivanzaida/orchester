//! What an admin's edit of a team MCP server does in its panel (`Agents · MCP server · admin · draft` A13 `WMtxm`, A14
//! `gTLx6`, A16 `x3ywL`, A17 `rJbPm`): the team's part becomes a form of the fields the create form has, kept as the
//! resource's draft while it is typed, and *Publish for the team* sends the draft to the team server and says its
//! answer in place. A refusal keeps the draft and what was typed; waiting for a server that cannot be reached is not
//! one.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureState},
  core::Signal,
};
use orchester_core::{
  id::{ProjectId, ResourceId, VersionId},
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    save_lane::SaveLane,
    team_mcp_edit::{self, EditError, McpEdit, Saved},
  },
  app_mcp::setup::NewTeamMcpServer,
  background,
  messages::Message,
  sharing::team_edit::{EditRefused, Edited},
  tasks::outside::Outside,
};

use super::edit_actions as actions;
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
  /// Orders this resource's saves of the draft ([`SaveLane`]), which *Publish for the team* joins.
  lane: Arc<SaveLane>,
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
    let lane = Arc::new(SaveLane::default());
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
  /// lane: only the newest one writes, so a save that is late never undoes a newer one.
  pub(super) fn autosave(
    &self,
    ctx: &mut Ctx,
    (had_draft, published): (bool, Option<&NewTeamMcpServer>),
    input: Option<NewTeamMcpServer>,
  ) -> Result<Option<Saved>, EditError> {
    let services = Services::of(ctx);
    let (root, project, id) = self.target.clone();
    // The form opens with nothing to save; once a save started, one that goes back to the fields saves, to drop a
    // draft.
    let input = input.filter(|input| had_draft || published != Some(input) || self.lane.used());
    let state = ctx
      .future((root, had_draft, input), {
        let lane = self.lane.clone();
        move |(root, had_draft, input)| {
          let (services, lane) = (services.clone(), lane.clone());
          let ticket = input.as_ref().map(|_| lane.start());
          async move {
            let (Some(input), Some(ticket)) = (input, ticket) else {
              return Ok(None);
            };
            let saved = background::run(move || lane.run_if_newest(ticket, || team_mcp_edit::save(&root, id, &input)))
              .await
              .map_err(EditError::Store)?;
            // A newer save started meanwhile and writes what is typed now.
            let Some(saved) = saved.transpose()? else {
              return Ok(None);
            };
            if matches!(saved, Saved::Draft(_)) != had_draft {
              services.changes.record(project, Outside::Connections);
            }
            Ok(Some(saved))
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
