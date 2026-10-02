//! The models of a native connection (04.H6 `zHowy`): a 720 dialog over Connections that lists them,
//! edits one in place, adds one by hand and removes one no profile uses, then saves them all with
//! *Save models*. Its key, endpoint and last test stay as they are.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, ScrollVertical},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, NativeModels},
  id::ConnectionId,
};

use super::{
  DialogContext, DialogMode,
  dialog_frame::{SaveBar, actions, header, padded, save_reason_of},
  model_editor::{ModelEditor, ModelEditorProps, ModelRows, ModelUse},
};
use crate::{
  activity::{Activity, operations},
  background,
  connections::native::save_models_in,
  messages::Message,
  theme::{ExtraColor, gutter_scrollbar},
  ui::{
    backdrop::backdrop,
    button::SKIPPED_BY_TAB,
    form::{Submit, form},
  },
};

/// Dialog width (`zHowy`), and the tallest its list grows before it scrolls.
const DIALOG_WIDTH: f32 = 720.0;
const LIST_MAX_HEIGHT: f32 = 560.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Saved(ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Saved {}

type SaveAction = FutureAction<(PathBuf, ConnectionId, NativeModels), Saved, Message>;

pub struct ModelsDialog {
  context: DialogContext,
  connection: AgentConnection,
  models: Signal<ModelRows>,
  frame: ElementRef,
}

impl Component for ModelsDialog {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<DialogContext>()
      .expect("the connections screen provides the dialog context");
    let connection = editing(&context).expect("the connections screen mounts the models dialog for a connection");
    let rows = connection
      .api()
      .map(|api| api.models().models().to_vec())
      .unwrap_or_default();
    let saved = context.saved.clone();
    let save = save_action(ctx);
    ctx.watch(&save.state(), move |state| {
      if let (false, Some(Saved(id))) = (state.is_pending(), state.data) {
        saved(id);
      }
    });
    let dialog = Self {
      models: ctx.signal(ModelRows(rows)),
      frame: ElementRef::new(),
      context,
      connection,
    };
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let save = save_action(ctx);
    let models = self.models_to_save();
    let submit = self.save(&save, models.as_ref().ok());
    let current = self.connection.api().map(|api| api.models());
    let uses = self
      .context
      .uses
      .iter()
      .filter(|used| used.connection == self.connection.id())
      .map(|used| ModelUse {
        role: used.role.clone(),
        model: used.model.clone(),
      })
      .collect();
    let editor = ctx.mount_keyed::<ModelEditor>(
      "models-dialog-editor",
      ModelEditorProps {
        models: self.models.clone(),
        default_model: current.and_then(|models| models.default_model().cloned()),
        uses: Arc::new(uses),
        add: "connections.models.add",
      },
    );
    let state = save.state().get();
    let saving = state.is_pending();
    let bar = SaveBar {
      saving,
      reason: save_reason_of(&models, state.error.filter(|_| !saving)),
      submit: submit.clone(),
    };
    let title = ctx.t_args(
      "connections.models.dialogTitle",
      [("connection", self.connection.name().as_str())],
    );
    let panel = Column::new()
      .id("models-dialog")
      .ref_element(self.frame.clone())
      .focusable(true)
      .tab_index(SKIPPED_BY_TAB)
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(header(ctx, &title, &self.context))
      .child(
        ScrollVertical::new(padded(editor))
          .width(Dimension::Pct(100.0))
          .max_height(Dimension::Px(LIST_MAX_HEIGHT))
          .scrollbar(gutter_scrollbar()),
      )
      .child(actions(
        ctx,
        &self.context,
        bar,
        ("models-save", "connections.models.saveAll"),
      ));
    backdrop(ctx, form(submit, panel))
  }
}

impl ModelsDialog {
  /// The models as they would be saved, keeping the default model and effort while the default model
  /// is listed.
  fn models_to_save(&self) -> Result<NativeModels, Message> {
    let rows = self.models.get().0;
    if rows.is_empty() {
      return Err(Message::new("connections.native.save.noModels"));
    }
    let current = self.connection.api().map(|api| api.models());
    let default_model = current
      .and_then(|models| models.default_model().cloned())
      .filter(|id| rows.iter().any(|model| model.id() == id));
    let default_effort = current.and_then(|models| models.default_effort()).filter(|effort| {
      rows
        .iter()
        .any(|model| Some(model.id()) == default_model.as_ref() && model.efforts().contains(effort))
    });
    NativeModels::new(rows, default_model, default_effort)
      .map_err(|_| Message::new("connections.native.save.defaultsInvalid"))
  }

  fn save(&self, save: &SaveAction, models: Option<&NativeModels>) -> Submit {
    if save.state().get_untracked().is_pending() {
      return None;
    }
    let (save, models) = (save.clone(), models?.clone());
    let (repo_root, id) = (self.context.project.repo_root.clone(), self.connection.id());
    Some(Arc::new(move || save.run((repo_root.clone(), id, models.clone()))))
  }
}

/// The connection whose models the dialog edits.
fn editing(context: &DialogContext) -> Option<AgentConnection> {
  match &context.mode {
    DialogMode::NativeModels(connection) => Some(connection.clone()),
    _ => None,
  }
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  let context = ctx
    .use_context::<DialogContext>()
    .expect("the connections screen provides the dialog context");
  let name = editing(&context)
    .map(|connection| connection.name().as_str().to_owned())
    .unwrap_or_default();
  ctx.future_action(move |(repo_root, id, models): (PathBuf, ConnectionId, NativeModels)| {
    let spec = operations::connection_save(&context.project, &name);
    let activity = activity.clone();
    async move {
      let save = async move {
        background::run(move || save_models_in(&repo_root, id, models))
          .await?
          .map(|()| Saved(id))
      };
      activity.track(spec, save).await
    }
  })
}
