//! Add MCP server, also used to edit one (not drawn yet: built from the connection dialog frame
//! `IAN4P` and its fields). A server needs a name the agent CLIs know it by and a transport: a
//! program the agent's CLI starts, with its arguments, folder and variables, or a URL with an
//! optional bearer token from a project secret. Nothing is started or reached while it is set up;
//! the agent's CLI starts or reaches it when an agent that gets it starts. The fields are in
//! `mcp_form`; a new connection's type can be switched to the other kinds.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_core::id::{ConnectionId, SecretId};

use super::{
  DialogContext, DialogMode, KindChoice,
  dialog_frame::{SaveBar, dialog, save_reason_of},
};
use crate::{
  activity::{Activity, operations},
  background,
  connections::mcp::{McpServerDraft, McpServerErrors, McpServerText, TransportChoice, save_in, secret_reference},
  messages::Message,
  ui::form::Submit,
};

/// The bearer token select: none, or a project secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BearerChoice(pub Option<SecretId>);

impl lurq::app::component::DevtoolsInspectable for BearerChoice {}

/// *Add a variable from a secret*: set when a secret is picked, then cleared once its line is added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InsertSecret(pub Option<SecretId>);

impl lurq::app::component::DevtoolsInspectable for InsertSecret {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Saved(ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Saved {}

type SaveAction = FutureAction<(PathBuf, Option<ConnectionId>, McpServerDraft), Saved, Message>;

pub struct McpDialog {
  pub(super) context: DialogContext,
  pub(super) kind: Signal<KindChoice>,
  pub(super) name: Signal<String>,
  pub(super) transport: Signal<TransportChoice>,
  pub(super) program: Signal<String>,
  pub(super) arguments: Signal<String>,
  pub(super) working_directory: Signal<String>,
  pub(super) variables: Signal<String>,
  pub(super) url: Signal<String>,
  pub(super) bearer: Signal<BearerChoice>,
  pub(super) insert: Signal<InsertSecret>,
  /// Focused when the dialog opens so that Tab starts at its first control.
  frame: ElementRef,
}

impl Component for McpDialog {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<DialogContext>()
      .expect("the connections screen provides the dialog context");
    let text = match &context.mode {
      DialogMode::EditMcp(server) => McpServerText::of(server, &context.secrets),
      _ => McpServerText::new_server(),
    };
    let dialog = Self {
      kind: ctx.signal(KindChoice::Mcp),
      name: ctx.signal(text.name),
      transport: ctx.signal(text.transport),
      program: ctx.signal(text.program),
      arguments: ctx.signal(text.arguments),
      working_directory: ctx.signal(text.working_directory),
      variables: ctx.signal(text.variables),
      url: ctx.signal(text.url),
      bearer: ctx.signal(BearerChoice(text.bearer)),
      insert: ctx.signal(InsertSecret(None)),
      frame: ElementRef::new(),
      context,
    };
    ctx.focus(&dialog.frame);
    dialog.watch(ctx);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let save = save_action(ctx);
    let draft = self.text().draft(&self.context.mcp_taken, &self.context.secrets);
    let errors = draft
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let draft = draft.map_err(|errors| first_error(&errors));
    let submit = self.save(&save, draft.as_ref().ok());
    let fields = self.form(ctx, &errors);
    let state = save.state().get();
    let saving = state.is_pending();
    let bar = SaveBar {
      saving,
      reason: save_reason_of(&draft, state.error.filter(|_| !saving)),
      submit,
    };
    let title = match self.context.mode {
      DialogMode::EditMcp(_) => "connections.mcp.dialog.editTitle",
      _ => "connections.mcp.dialog.addTitle",
    };
    dialog(ctx, &self.frame, &ctx.t(title), &self.context, fields, bar)
  }
}

impl McpDialog {
  /// Another type switches to its dialog; a saved server closes this one; a picked secret adds a
  /// variable line for it.
  fn watch(&self, ctx: &mut Ctx) {
    let retype = self.context.retype.clone();
    ctx.watch(&self.kind, move |kind| {
      if *kind != KindChoice::Mcp {
        retype(*kind);
      }
    });
    let saved = self.context.saved.clone();
    let save = save_action(ctx);
    ctx.watch(&save.state(), move |state| {
      if let (false, Some(Saved(id))) = (state.is_pending(), state.data) {
        saved(id);
      }
    });
    let (variables, insert, secrets) = (
      self.variables.clone(),
      self.insert.clone(),
      self.context.secrets.clone(),
    );
    ctx.watch(&self.insert, move |choice| {
      let Some(secret) = choice.0.and_then(|id| secrets.iter().find(|secret| secret.id() == id)) else {
        return;
      };
      let line = format!("{}={}", secret.name(), secret_reference(secret));
      variables.update(|text| {
        if !text.is_empty() && !text.ends_with('\n') {
          text.push('\n');
        }
        text.push_str(&line);
      });
      insert.set(InsertSecret(None));
    });
  }

  fn text(&self) -> McpServerText {
    McpServerText {
      name: self.name.get(),
      transport: self.transport.get(),
      program: self.program.get(),
      arguments: self.arguments.get(),
      working_directory: self.working_directory.get(),
      variables: self.variables.get(),
      url: self.url.get(),
      bearer: self.bearer.get().0,
    }
  }

  /// Stores the draft; `None` until it can be saved, and while it is being saved.
  fn save(&self, save: &SaveAction, draft: Option<&McpServerDraft>) -> Submit {
    if save.state().get_untracked().is_pending() {
      return None;
    }
    let id = match &self.context.mode {
      DialogMode::EditMcp(server) => Some(server.id()),
      _ => None,
    };
    let (save, repo_root, draft) = (save.clone(), self.context.project.repo_root.clone(), draft?.clone());
    Some(Arc::new(move || save.run((repo_root.clone(), id, draft.clone()))))
  }
}

/// The first reason the server cannot be saved, in the order the fields read.
fn first_error(errors: &McpServerErrors) -> Message {
  [
    &errors.name,
    &errors.program,
    &errors.working_directory,
    &errors.arguments,
    &errors.variables,
    &errors.url,
  ]
  .into_iter()
  .find_map(Clone::clone)
  .expect("a server that cannot be saved says why in one of its fields")
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  let project = ctx
    .use_context::<DialogContext>()
    .expect("the connections screen provides the dialog context")
    .project;
  ctx.future_action(
    move |(repo_root, id, draft): (PathBuf, Option<ConnectionId>, McpServerDraft)| {
      let spec = operations::connection_save(&project, draft.name.as_str());
      let activity = activity.clone();
      async move {
        let save = async move { background::run(move || save_in(&repo_root, id, draft).map(Saved)).await? };
        activity.track(spec, save).await
      }
    },
  )
}
