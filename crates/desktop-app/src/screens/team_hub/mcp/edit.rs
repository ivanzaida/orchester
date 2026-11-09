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
  resource::VersionNumber,
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    save_lane::SaveLane,
    team_mcp_conflict::Based,
    team_mcp_edit::McpEdit,
    team_mcp_merge::{McpField, Pick},
  },
  app_mcp::setup::NewTeamMcpServer,
  messages::Message,
  sharing::team_edit::{EditRefused, Edited},
};
use parking_lot::Mutex;

use super::{
  edit_actions as actions,
  edit_save::{Answer, Save},
};
use crate::{screens::team_hub::create::McpFields, services::Services};

/// A press of *Publish for the team*: the fields as typed over the version the form is edited from, whether a person
/// (or automation they let decide) pressed, and the version in the outbox that the team server refused, which goes back
/// to being the draft first.
pub(super) type Publish = FutureAction<(NewTeamMcpServer, VersionNumber, bool, Option<VersionId>), Edited, EditRefused>;

/// A press of *Rebase my draft*: the version of the person's own in the outbox that met the team's, the team's version
/// the comparison shown stands for, what was picked for the fields both changed, and whether a person (or automation
/// they let decide) pressed. The fields the form shows again, and the version they are edited from.
pub(super) type Rebase = FutureAction<(Option<VersionId>, VersionNumber, Vec<(McpField, Pick)>, bool), Based, Message>;

/// A press of *Restore*: whether a person (or automation they let decide) pressed.
pub(super) type Restore = FutureAction<bool, (), Message>;

/// A press of *Discard draft*: the refused version in the outbox that goes back to being the draft first, if there is
/// one; the version the form shows again and its fields.
pub(super) type Discard = FutureAction<Option<VersionId>, Based, Message>;

/// What the panel keeps of an edit.
pub(super) struct Edit {
  /// The form's fields; none for a server the form cannot hold.
  pub fields: Option<McpFields>,
  /// *Edit* was pressed, or a draft was kept: the team's part is a form.
  pub editing: Signal<bool>,
  /// Why the last *Publish for the team* did not publish, and when: until the next press, a discard or a cancel.
  pub refused: Signal<Option<(EditRefused, Timestamp)>>,
  pub publish: Publish,
  /// *Discard draft*.
  pub discard: Discard,
  /// *Cancel sending*: takes the version waiting in the outbox back as the draft.
  pub cancel: FutureAction<VersionId, Option<Based>, Message>,
  /// *Cancel sending* of a new resource waiting in the outbox: takes it back as if never created.
  pub cancel_created: FutureAction<(), (), Message>,
  /// *Rebase my draft on vN* (A19).
  pub rebase: Rebase,
  /// *Open vN*: the version of the person's own in the outbox that met the team's, if there is one, goes back to
  /// being the draft, and the page shows the team's version.
  pub open_new: FutureAction<Option<VersionId>, Option<Based>, Message>,
  /// *Restore* a resource the team archived (A21).
  pub restore: Restore,
  /// What the person picked for each field both changed, which *Rebase* asks for.
  pub picks: Signal<Vec<(McpField, Pick)>>,
  /// *View changes* was pressed: the page compares the team's version with the draft (A18).
  pub comparing: Signal<bool>,
  /// The modal that asks before *Discard my draft* (A20) is open.
  pub confirm_discard: Signal<bool>,
  /// The modal that asks before *Restore* is open.
  pub confirm_restore: Signal<bool>,
  /// The version the form is edited from, with its fields: what a field is marked *changed in your draft* against, and
  /// what a save says the draft is edited from. It stays the version the form was opened on while the team publishes
  /// newer ones, until a rebase or a discard moves it.
  pub base: Arc<Mutex<Option<Based>>>,
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
    let base = Arc::new(Mutex::new(opened_on(edit)));
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
      rebase: actions::rebase(ctx, &services, &target),
      open_new: actions::open_new(ctx, &services, &target),
      restore: actions::restore(ctx, &services, &target),
      picks: ctx.signal(Vec::new()),
      comparing: ctx.signal(false),
      confirm_discard: ctx.signal(false),
      confirm_restore: ctx.signal(false),
      base,
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
    let (base, reset) = (self.base.clone(), self.reset());
    ctx.watch(&self.discard.state(), move |state| {
      if let Some(published) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        editing.set(false);
        refused.set(None);
        reset();
        // The fields are the version's own again; a later edit starts from it.
        if let Some(fields) = &fields {
          fields.load(&published.fields);
        }
        *base.lock() = Some(published.clone());
      }
    });
    self.follow_conflict(ctx);
    let (editing, refused, base) = (self.editing.clone(), self.refused.clone(), self.base.clone());
    ctx.watch(&self.cancel.state(), move |state| {
      if state.is_fulfilled() {
        refused.set(None);
        editing.set(true);
        keep_base(&base, state.data.as_ref().and_then(Option::as_ref));
      }
    });
    self.follow_failures(ctx);
  }

  /// What a rebase and the pages that follow it do: the form shows what the merge made, edited from the team's version.
  fn follow_conflict(&self, ctx: &mut Ctx) {
    let (editing, refused, fields) = (self.editing.clone(), self.refused.clone(), self.fields.clone());
    let (base, reset) = (self.base.clone(), self.reset());
    ctx.watch(&self.rebase.state(), move |state| {
      if let Some(based) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        editing.set(true);
        refused.set(None);
        reset();
        if let Some(fields) = &fields {
          fields.load(&based.fields);
        }
        *base.lock() = Some(based.clone());
      }
    });
    let (editing, refused, reset) = (self.editing.clone(), self.refused.clone(), self.reset());
    let base = self.base.clone();
    ctx.watch(&self.open_new.state(), move |state| {
      if state.is_fulfilled() {
        editing.set(false);
        refused.set(None);
        reset();
        keep_base(&base, state.data.as_ref().and_then(Option::as_ref));
      }
    });
    let (editing, refused, comparing) = (self.editing.clone(), self.refused.clone(), self.comparing.clone());
    ctx.watch(&self.restore.state(), move |state| {
      if state.is_fulfilled() {
        // The resource is back, and the draft with it: the form shows it again.
        editing.set(true);
        refused.set(None);
        comparing.set(false);
      }
    });
  }

  /// Ends what a comparison keeps: the picks, the modals and the page that compares.
  fn reset(&self) -> impl Fn() + Send + Sync + 'static {
    let (picks, comparing) = (self.picks.clone(), self.comparing.clone());
    let (discard, restore) = (self.confirm_discard.clone(), self.confirm_restore.clone());
    move || {
      picks.set(Vec::new());
      comparing.set(false);
      discard.set(false);
      restore.set(false);
    }
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
    ctx.watch(&self.cancel_created.state(), {
      let failure = failure.clone();
      move |state| settle_failure(&failure, state)
    });
    ctx.watch(&self.rebase.state(), {
      let failure = failure.clone();
      move |state| settle_failure(&failure, state)
    });
    ctx.watch(&self.open_new.state(), {
      let failure = failure.clone();
      move |state| settle_failure(&failure, state)
    });
    ctx.watch(&self.restore.state(), move |state| settle_failure(&failure, state));
  }

  /// Keeps `input` as the draft while the form is open (`Draft saved` in A13), edited from the version the form was
  /// opened on, and tells the screens that read drafts when a draft appears or goes; nothing for `None`. `had_draft`:
  /// the panel's last read held a draft. An input that still equals the version's fields needs no save. Saves go
  /// through the resource's lane ([`SaveLane`]): only the newest one writes, so a save that is late never undoes a
  /// newer one, whichever panel started it. A save is on the blocking pool from the moment it starts ([`Save`]), so it
  /// writes even when this panel is closed first, and one the store refused is told and tried again.
  pub(super) fn autosave(&self, ctx: &mut Ctx, had_draft: bool, input: Option<NewTeamMcpServer>) -> Answer {
    let services = Services::of(ctx);
    let Some(based) = self.base.lock().clone() else {
      return Ok(None);
    };
    // The form opens with nothing to save; once this panel asked for a save, one that goes back to the fields saves, to
    // drop a draft.
    let input = input.filter(|input| had_draft || &based.fields != input || self.asked.load(Ordering::SeqCst));
    self.asked.fetch_or(input.is_some(), Ordering::SeqCst);
    let (lane, target) = (self.lane.clone(), self.target.clone());
    let state = ctx
      .stream(
        (had_draft, input.map(|input| (input, based.number))),
        move |(had_draft, input), emitter| {
          let started = input.map(|(input, base)| {
            let shared = (lane.clone(), services.runtime.clone(), services.changes.clone());
            Save::start(shared, target.clone(), (had_draft, input, base))
          });
          async move {
            match started {
              Some((save, job)) => save.settle(job, &emitter).await,
              None => {
                emitter.emit(None);
              }
            }
          }
        },
      )
      .state()
      .get();
    match state.error {
      Some(error) if !state.is_pending() => Err(error),
      _ => Ok(state.data.flatten()),
    }
  }
}

/// The version a form is edited from when it opens: the one a draft was edited from, which is older than the team's
/// latest when the team published since; else the latest the team server numbered. `None` for a server the form cannot
/// hold.
pub(super) fn opened_on(edit: Option<&McpEdit>) -> Option<Based> {
  let edit = edit?;
  // The latest version here is the person's own while one waits in the outbox or was refused: a draft is not edited
  // from that, but from the latest the team server numbered, which a refused version taken back as the draft is edited
  // from too.
  let confirmed = edit.team.as_ref().unwrap_or(&edit.published);
  Some(match (&edit.newer, &edit.draft) {
    (Some(newer), _) => Based {
      number: newer.base_number,
      fields: newer.base.clone().unwrap_or_else(|| confirmed.clone()),
    },
    (None, Some(draft)) => Based {
      number: draft.based_on,
      fields: confirmed.clone(),
    },
    (None, None) => Based {
      number: edit.team_number.unwrap_or(edit.number),
      fields: confirmed.clone(),
    },
  })
}

/// The form is edited from `based` from now on, when a read of the server said so.
fn keep_base(base: &Mutex<Option<Based>>, based: Option<&Based>) {
  if let Some(based) = based {
    *base.lock() = Some(based.clone());
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
