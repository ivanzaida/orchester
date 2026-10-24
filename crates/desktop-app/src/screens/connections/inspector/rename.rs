//! Renaming an agent or GitHub connection in its inspector's header: the name with a pencil that edits it in place,
//! checked as Add connection checks a name (not blank, at most 200 characters, no other agent or GitHub connection of
//! the project has it), and Save name storing it. Profiles, pipelines and runs name a connection by its id, so they
//! keep running on it. An MCP server's name is what agents know it by, so it is renamed in Edit MCP server instead.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ConnectionId, name::Name};
use orchester_workspace::{
  activity::operations, background, connections::data::rename_in, messages::Message, tasks::outside::Outside,
  workspace::OpenProject,
};

use super::{Inspecting, title};
use crate::{
  screens::connections::{add_agent::naming::problem_among, data::Saved},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
  },
};

/// The name of connection `id`, called `name`, as the header shows it, with its rename.
pub(super) fn title_with_rename(
  ctx: &mut Ctx,
  inspecting: &Inspecting<'_>,
  (id, name): (ConnectionId, &str),
) -> Element {
  let listed = inspecting.listed;
  let agents = listed
    .connections
    .iter()
    .map(|connection| (connection.id(), connection.name()));
  let taken = agents
    .chain(
      listed
        .github
        .iter()
        .map(|connection| (connection.id(), connection.name())),
    )
    .filter(|(other, _)| *other != id)
    .map(|(_, name)| name.as_str().to_owned())
    .collect();
  ctx.mount_keyed::<Rename>(
    &format!("connection-rename-{id}"),
    RenameProps {
      open: inspecting.open.clone(),
      connection: id,
      name: name.to_owned(),
      taken: Arc::new(taken),
    },
  )
}

#[derive(Clone, PartialEq)]
pub(super) struct RenameProps {
  open: OpenProject,
  connection: ConnectionId,
  name: String,
  /// The names of the project's other agent and GitHub connections.
  taken: Arc<Vec<String>>,
}

impl DevtoolsInspectable for RenameProps {}

type RenameAction = FutureAction<(OpenProject, ConnectionId, Name), Saved, Message>;

pub(super) struct Rename {
  editing: Signal<bool>,
  draft: Signal<String>,
  /// The name Save name last tried, whose refusal is said while the field still holds it.
  tried: Signal<String>,
  save: RenameAction,
}

impl Component for Rename {
  type Props = RenameProps;

  fn create(ctx: &mut Ctx) -> Self {
    let save = save_action(ctx);
    let editing = ctx.signal(false);
    let done = editing.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_fulfilled() {
        done.set(false);
      }
    });
    Self {
      editing,
      draft: ctx.signal(String::new()),
      tried: ctx.signal(String::new()),
      save,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RenameProps>().clone();
    if self.editing.get() {
      return self.editor(ctx, &props);
    }
    let (editing, draft, name) = (self.editing.clone(), self.draft.clone(), props.name.clone());
    let rename = IconButton::new("connection-rename", Icon::Pencil, ctx.t("connections.rename.action"))
      .compact()
      .on_click(move || {
        draft.set(name.clone());
        editing.set(true);
      })
      .render(ctx);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(title(&props.name).flex_shrink(1.0).min_width(Dimension::Px(0.0)))
      .child(rename)
      .into()
  }
}

impl Rename {
  /// The name in a field, why it cannot be saved under it, and Save name and Cancel.
  fn editor(&self, ctx: &mut Ctx, props: &RenameProps) -> Element {
    let draft = self.draft.get();
    let state = self.save.state().get();
    let problem = problem_among(ctx, props.taken.iter().map(String::as_str), &draft);
    let refused = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected() && draft.trim() == self.tried.get())
      .map(|error| message_text(ctx, error));
    let mut field = FieldInput::new(
      "connection-rename-name",
      ctx.t("connections.dialog.name"),
      self.draft.clone(),
    );
    if let Some(reason) = problem.clone().or(refused) {
      field = field.error(reason);
    }
    let save = ActionButton::new(
      "connection-rename-save",
      ButtonKind::Primary,
      ctx.t("connections.rename.save"),
    );
    let unchanged = draft.trim() == props.name;
    let save = match Name::new(&draft) {
      _ if state.is_pending() => save.loading(),
      Ok(name) if problem.is_none() && !unchanged => {
        let (action, tried, open, id) = (
          self.save.clone(),
          self.tried.clone(),
          props.open.clone(),
          props.connection,
        );
        save.on_click(move || {
          tried.set(name.as_str().to_owned());
          action.run((open.clone(), id, name.clone()));
        })
      }
      _ => save.disabled(),
    };
    let editing = self.editing.clone();
    let cancel = ActionButton::new(
      "connection-rename-cancel",
      ButtonKind::Secondary,
      ctx.t("connections.cancel"),
    )
    .on_click(move || editing.set(false));
    let cancel = if state.is_pending() { cancel.disabled() } else { cancel };
    Column::new()
      .id("connection-rename")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field.render(ctx))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(save)
          .child(cancel),
      )
      .into()
  }
}

/// Stores a new name, listed in the status bar's activity; every screen that shows the project's connections reads
/// them again.
fn save_action(ctx: &mut Ctx) -> RenameAction {
  let Services { activity, changes, .. } = Services::of(ctx);
  ctx.future_action(move |(open, id, name): (OpenProject, ConnectionId, Name)| {
    let (activity, changes) = (activity.clone(), changes.clone());
    async move {
      let spec = operations::connection_save(&open, name.as_str());
      let (project, repo_root) = (open.project.id(), open.repo_root.clone());
      let rename = async move {
        background::run(move || {
          rename_in(&repo_root, id, name).map_err(|error| {
            tracing::error!(%error, "cannot rename the connection");
            error.message()
          })
        })
        .await?
      };
      activity.track(spec, rename).await?;
      changes.record(project, Outside::Connections);
      Ok(Saved(id))
    }
  })
}
