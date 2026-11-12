//! What an admin's edit of a team MCP server keeps in its panel (`docs/design/team-configuration-live.md`, rule 6): the
//! team's part becomes a form of the fields the create form has, over the latest version read this session, under the
//! fixed Save/Cancel bar the local panels have. Nothing is kept while it is typed: Save publishes the fields as the
//! next version on the team server at once, made on the version the form was opened on, and Cancel puts that version
//! back in the fields. A
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

/// What a Save published: the version the server numbered, with the fields sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Saved(pub McpEdit);
impl lurq::app::component::DevtoolsInspectable for Saved {}

pub(super) type Save = FutureAction<SaveAsked, Saved, NotSaved>;

/// What the panel keeps of an edit. Cloning shares it.
#[derive(Clone)]
pub(super) struct Edit {
  /// The form's fields; none for a server the form cannot hold.
  pub fields: Option<McpFields>,
  /// Counts the changes of `base`, so the panel draws again.
  pub based: Signal<u64>,
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
      based: ctx.signal(0),
      refused: ctx.signal(None),
      save: save(ctx, target),
      base: Arc::new(Mutex::new(edit.cloned())),
    };
    edit.follow(ctx);
    edit
  }

  /// Takes the answer of a Save: published, the form is made on what was published; a refusal is kept with its time,
  /// and a version someone published first is what the form shows and is made on from now on.
  fn follow(&self, ctx: &mut Ctx) {
    let edit = self.clone();
    let clock = Services::of(ctx).clock;
    ctx.watch(&self.save.state(), move |state| {
      if state.is_pending() {
        edit.refused.set(None);
      } else if let Some(Saved(saved)) = state.data.clone().filter(|_| state.is_fulfilled()) {
        edit.rebase(saved, false);
      } else if let Some(NotSaved { error, reload }) = state.error.clone() {
        if let Some(reload) = reload {
          edit.rebase(reload, true);
        }
        edit.refused.set(Some((error, clock.now())));
      }
    });
  }

  /// The form is made on `latest` from now on, and its fields take what it publishes when `load`.
  fn rebase(&self, latest: McpEdit, load: bool) {
    if let (true, Some(fields)) = (load, &self.fields) {
      fields.load(&latest.published);
    }
    *self.base.lock() = Some(latest);
    self.based.update(|n| *n += 1);
  }

  /// Keeps the form on `latest`, the version the panel reads now, while nothing in it differs from the version it is
  /// made on: a version published elsewhere shows at once. Changed fields stay; Save then says someone published
  /// first.
  pub(super) fn keep_up(&self, latest: Option<&McpEdit>, changed: bool) {
    let Some(latest) = latest else {
      return;
    };
    let behind = self.base.lock().as_ref().is_none_or(|base| base.number < latest.number);
    if behind && !changed && !self.save.state().get_untracked().is_pending() {
      self.rebase(latest.clone(), true);
    }
  }

  /// The version the form is made on.
  pub(super) fn base(&self) -> Option<McpEdit> {
    self.based.get();
    self.base.lock().clone()
  }

  /// Cancel: the fields take the version the form is made on again.
  pub(super) fn cancel(&self) {
    if let (Some(fields), Some(base)) = (&self.fields, self.base.lock().clone()) {
      fields.load(&base.published);
    }
    self.refused.set(None);
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
      saved
        .map(|number| {
          Saved(McpEdit {
            number,
            published: input.clone(),
          })
        })
        .map_err(|error| {
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
