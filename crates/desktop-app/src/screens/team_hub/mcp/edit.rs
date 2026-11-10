//! What an admin's edit of a team MCP server keeps in its panel (`docs/design/team-configuration-live.md`, rule 6): the
//! team's part becomes a form of the fields the create form has, over the latest version read this session, under the
//! fixed Save/Cancel bar the local panels have. Nothing is kept while it is typed: Save publishes the fields as the
//! next version on the team server at once, made on the version the form was opened on, and Cancel closes the form. A
//! refusal is said in place and keeps what was typed; a version someone published first is loaded into the form, and
//! the refusal says so.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::VersionNumber,
  time::Timestamp,
};
use orchester_workspace::{
  agents::team_mcp_edit::{self, EditError, McpEdit},
  app_mcp::setup::NewTeamMcpServer,
  messages::Message,
  sharing::team_edit::EditRefused,
  tasks::outside::Outside,
};
use parking_lot::Mutex;

use crate::{screens::team_hub::create::McpFields, services::Services};

/// Why a Save did not publish, with what the form starts from again when someone published first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NotSaved {
  pub error: EditError,
  pub reload: Option<McpEdit>,
}
impl lurq::app::component::DevtoolsInspectable for NotSaved {}

/// A press of Save: the fields as typed and the version they are made on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SaveAsked {
  pub input: NewTeamMcpServer,
  pub base: VersionNumber,
}
impl lurq::app::component::DevtoolsInspectable for SaveAsked {}

/// The version a Save published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Saved(pub VersionNumber);
impl lurq::app::component::DevtoolsInspectable for Saved {}

pub(super) type Save = FutureAction<SaveAsked, Saved, NotSaved>;

/// What the panel keeps of an edit. Cloning shares it.
#[derive(Clone)]
pub(super) struct Edit {
  /// The form's fields; none for a server the form cannot hold.
  pub fields: Option<McpFields>,
  /// *Edit* was pressed: the team's part is a form.
  pub editing: Signal<bool>,
  /// Why the last Save did not publish, and when: until the next press or a cancel.
  pub refused: Signal<Option<(EditError, Timestamp)>>,
  pub save: Save,
  /// The version the form was opened on: a Save is made on it, and the fields are compared with its own.
  pub base: Arc<Mutex<Option<McpEdit>>>,
}

impl Edit {
  pub(super) fn new(ctx: &mut Ctx, target: (PathBuf, ProjectId, ResourceId), edit: Option<&McpEdit>) -> Self {
    let fields = edit.map(|edit| McpFields::of(ctx, &edit.published));
    let edit = Self {
      fields,
      editing: ctx.signal(false),
      refused: ctx.signal(None),
      save: save(ctx, target),
      base: Arc::new(Mutex::new(edit.cloned())),
    };
    edit.follow(ctx);
    edit
  }

  /// Takes the answer of a Save: published closes the form; a refusal is kept with its time, and a version someone
  /// published first is what the form shows and is made on from now on.
  fn follow(&self, ctx: &mut Ctx) {
    let (editing, refused) = (self.editing.clone(), self.refused.clone());
    let (fields, base) = (self.fields.clone(), self.base.clone());
    let clock = Services::of(ctx).clock;
    ctx.watch(&self.save.state(), move |state| {
      if state.is_pending() {
        refused.set(None);
      } else if state.is_fulfilled() {
        editing.set(false);
      } else if let Some(NotSaved { error, reload }) = state.error.clone() {
        if let Some(reload) = reload {
          if let Some(fields) = &fields {
            fields.load(&reload.published);
          }
          *base.lock() = Some(reload);
        }
        refused.set(Some((error, clock.now())));
      }
    });
  }

  /// *Edit*: the form opens on `latest`, the version the panel shows.
  pub(super) fn open(&self, latest: &McpEdit) {
    if let Some(fields) = &self.fields {
      fields.load(&latest.published);
    }
    *self.base.lock() = Some(latest.clone());
    self.refused.set(None);
    self.editing.set(true);
  }

  /// Cancel: the form closes and what was typed goes.
  pub(super) fn cancel(&self) {
    self.refused.set(None);
    self.editing.set(false);
  }
}

/// Save's action: the fields go to the team server as the next version of server `id`, made on the version given.
fn save(ctx: &mut Ctx, (root, project, id): (PathBuf, ProjectId, ResourceId)) -> Save {
  let services = Services::of(ctx);
  ctx.future_action(move |SaveAsked { input, base }: SaveAsked| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      let not_saved = |error| NotSaved { error, reload: None };
      let Some(config) = services.workspace.team_read(&root).config().cloned() else {
        let reading = EditRefused::Failed(Message::new("teamConfig.reading"));
        return Err(not_saved(EditError::Refused(reading)));
      };
      let saved = team_mcp_edit::save(&services.servers, (&root, &config), (id, base), &input).await;
      services.changes.record(project, Outside::Connections);
      saved.map(Saved).map_err(|error| {
        let reload = matches!(error, EditError::Refused(EditRefused::Changed { .. }))
          .then(|| {
            services
              .workspace
              .team_read(&root)
              .config()
              .and_then(|now| team_mcp_edit::latest(now, id))
          })
          .flatten();
        NotSaved { error, reload }
      })
    }
  })
}
