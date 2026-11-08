//! What an admin's edit of a team MCP server does in its panel (`Agents · MCP server · admin · draft` A13 `WMtxm`, A14
//! `gTLx6`, A16 `x3ywL`, A17 `rJbPm`): the team's part becomes a form of the fields the create form has, kept as the
//! resource's draft while it is typed, and *Publish for the team* sends the draft to the team server and says its
//! answer in place. A refusal keeps the draft and what was typed; waiting for a server that cannot be reached is not
//! one.

use std::path::PathBuf;

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureState},
  core::Signal,
};
use orchester_core::{
  id::{ProjectId, ResourceId, VersionId},
  resource::ResourceKind,
  time::Timestamp,
};
use orchester_workspace::{
  agents::team_mcp_edit::{self, EditError, McpEdit, Saved},
  app_mcp::setup::NewTeamMcpServer,
  background,
  messages::Message,
  sharing::team_edit::{EditRefused, Edited},
  tasks::outside::Outside,
};

use crate::{screens::team_hub::create::McpFields, services::Services};

/// A press of *Publish for the team*: the fields as typed, and whether a person (or automation they let decide)
/// pressed.
pub(super) type Publish = FutureAction<(NewTeamMcpServer, bool), Edited, EditRefused>;

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
  pub discard: FutureAction<NewTeamMcpServer, NewTeamMcpServer, Message>,
  /// *Cancel sending*: takes the version waiting in the outbox back as the draft.
  pub cancel: FutureAction<VersionId, (), Message>,
  /// *Cancel sending* of a new resource waiting in the outbox: takes it back as if never created.
  pub cancel_created: FutureAction<(), (), Message>,
  /// Why the last *Discard draft* or *Cancel sending* did nothing (it is being sent right now, the server numbered it
  /// meanwhile, a run uses it, the person is no admin any more): until the next press.
  pub failure: Signal<Option<Message>>,
}

impl Edit {
  pub(super) fn new(
    ctx: &mut Ctx,
    (root, project, id): (PathBuf, ProjectId, ResourceId),
    edit: Option<&McpEdit>,
  ) -> Self {
    let services = Services::of(ctx);
    let fields = edit.map(|edit| {
      let start = edit.draft.as_ref().map_or(&edit.published, |draft| &draft.fields);
      McpFields::of(ctx, start)
    });
    let editing = ctx.signal(edit.is_some_and(|edit| edit.draft.is_some()));
    let refused = ctx.signal(None);
    let publish = {
      let (services, root) = (services.clone(), root.clone());
      ctx.future_action(move |(fields, decided): (NewTeamMcpServer, bool)| {
        let (services, root) = (services.clone(), root.clone());
        async move {
          if !decided {
            return Err(EditRefused::person_only());
          }
          let kept = root.clone();
          background::run(move || team_mcp_edit::save(&kept, id, &fields))
            .await
            .map_err(EditRefused::Failed)?
            .map_err(|error| EditRefused::Failed(failed(error)))?;
          let settled = services.servers.publish_mcp_edit(&root, id).await;
          services.changes.record(project, Outside::Connections);
          settled
        }
      })
    };
    let discard = {
      let (services, root) = (services.clone(), root.clone());
      ctx.future_action(move |published: NewTeamMcpServer| {
        let (services, root) = (services.clone(), root.clone());
        async move {
          let result = background::run(move || team_mcp_edit::discard(&root, id))
            .await?
            .map_err(failed);
          services.changes.record(project, Outside::Connections);
          result.map(|()| published)
        }
      })
    };
    let cancel = {
      let (services, root) = (services.clone(), root.clone());
      ctx.future_action(move |version: VersionId| {
        let (services, root) = (services.clone(), root.clone());
        async move {
          let result = services
            .servers
            .reopen_draft(&root, ResourceKind::McpServer, version)
            .await;
          services.changes.record(project, Outside::Connections);
          result
        }
      })
    };
    let cancel_created = ctx.future_action(move |(): ()| {
      let (services, root) = (services.clone(), root.clone());
      async move {
        let result = services.servers.cancel_created(&root, id).await;
        services.changes.record(project, Outside::Connections);
        result
      }
    });
    let edit = Self {
      fields,
      editing,
      refused,
      publish,
      discard,
      cancel,
      cancel_created,
      failure: ctx.signal(None),
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
  /// when a draft appears or goes; nothing for `None`. `had_draft`: the panel's last read held a draft.
  pub(super) fn autosave(
    ctx: &mut Ctx,
    (root, project, id): (PathBuf, ProjectId, ResourceId),
    had_draft: bool,
    input: Option<NewTeamMcpServer>,
  ) -> Result<Option<Saved>, EditError> {
    let services = Services::of(ctx);
    let state = ctx
      .future((root, had_draft, input), move |(root, had_draft, input)| {
        let services = services.clone();
        async move {
          let Some(input) = input else {
            return Ok(None);
          };
          let saved = background::run(move || team_mcp_edit::save(&root, id, &input))
            .await
            .map_err(EditError::Store)??;
          if matches!(saved, Saved::Draft(_)) != had_draft {
            services.changes.record(project, Outside::Connections);
          }
          Ok(Some(saved))
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

/// What a draft that could not be read or saved says.
fn failed(error: EditError) -> Message {
  match error {
    EditError::Store(message) => message,
    EditError::Invalid(invalid) => invalid.message(),
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
