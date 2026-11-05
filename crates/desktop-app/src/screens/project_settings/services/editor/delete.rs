//! *Delete service* asks in a modal first (confirmations are modals, the person's rule of 2026-10-28; the narrow
//! [`Dialog`] in the danger tone, as `08e · History · Delete confirmation` `I4xzOf`): *Delete <name>?* over what
//! deleting does — runs after it cannot start the service and needs that name it no longer resolve, runs that started
//! it keep their version, and it cannot be undone — then *Enter deletes* and *Esc cancels*, *Cancel* and *Delete
//! service*. A delete that failed says why and the dialog stays; one that worked opens Services. Only a person
//! deletes: an MCP client's press or Enter is refused, and the dialog says so.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::ResourceId;
use orchester_workspace::{agent_testing::editor::delete_in, background, messages::Message};

use super::{super::super::TabProps, View};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  theme::NARROW_DIALOG_WIDTH,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone, line, lines},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

/// The modal asking whether to delete the service `view` shows, while `open` holds; a new service has nothing to
/// delete.
pub(super) fn dialog(ctx: &mut Ctx, tab: &TabProps, view: &View, open: &Signal<bool>) -> Option<Element> {
  let entry = view.stored.entry.as_ref()?;
  Some(ctx.mount_keyed::<DeleteView>(
    &format!("settings-service-delete-{}", entry.id),
    DeleteProps {
      tab: tab.clone(),
      id: entry.id,
      name: entry.name.clone(),
      open: open.clone(),
    },
  ))
}

#[derive(Clone)]
struct DeleteProps {
  tab: TabProps,
  id: ResourceId,
  name: Arc<str>,
  open: Signal<bool>,
}

impl PartialEq for DeleteProps {
  fn eq(&self, other: &Self) -> bool {
    (self.id, &self.name) == (other.id, &other.name) && self.tab == other.tab
  }
}

impl DevtoolsInspectable for DeleteProps {}

type DeleteAction = FutureAction<(PathBuf, ResourceId), (), Message>;

struct DeleteView {
  delete: DeleteAction,
  /// An MCP client's input asked to delete and was refused.
  refused: Signal<bool>,
}

impl Component for DeleteView {
  type Props = DeleteProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<DeleteProps>().clone();
    let delete: DeleteAction = ctx.future_action(|(root, id): (PathBuf, ResourceId)| async move {
      background::run(move || delete_in(&root, id)).await?
    });
    let navigation = Navigation::of(ctx);
    let (project, feed, open) = (props.tab.project.open.project.id(), props.tab.project.feed, props.open);
    ctx.watch(&delete.state(), move |state| {
      if !state.is_fulfilled() || state.data.is_none() {
        return;
      }
      open.set(false);
      feed.saved();
      navigation.replace(Route::Project(
        project,
        ProjectPage::Settings(ProjectSettingsTab::Services),
      ));
    });
    Self {
      delete,
      refused: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DeleteProps>().clone();
    let state = self.delete.state().get();
    let pending = state.is_pending();
    let does = lines(
      "settings-service-delete-does",
      [
        line(
          "settings-service-delete-gone",
          Icon::Server,
          &ctx.t("project_settings.services.editor.delete.gone"),
        ),
        line(
          "settings-service-delete-kept",
          Icon::History,
          &ctx.t("project_settings.services.editor.delete.kept"),
        ),
        line(
          "settings-service-delete-final",
          Icon::Trash2,
          &ctx.t("project_settings.services.editor.delete.final"),
        ),
      ],
    );
    let confirm = self.confirm(props.tab.project.open.repo_root.clone(), props.id);
    let button = ActionButton::new(
      "settings-service-delete-confirm",
      ButtonKind::Danger,
      ctx.t("project_settings.services.editor.delete.confirm"),
    )
    .icon(Icon::Trash2);
    let button = if pending {
      button.loading()
    } else {
      button.on_click(confirm.clone())
    };
    let failure = state.error.filter(|_| !pending).map(|reason| {
      let reason = message_text(ctx, &reason);
      Element::from(field_error(
        "settings-service-delete-error",
        &ctx.t_args("project_settings.services.editor.delete.failed", [("reason", &*reason)]),
      ))
    });
    let dialog = Dialog::new(
      "settings-service-delete-dialog",
      Icon::Trash2,
      ctx.t_args(
        "project_settings.services.editor.delete.title",
        [("name", &*props.name)],
      ),
      ctx.t("project_settings.services.editor.delete.cancel"),
    )
    .tone(Tone::Danger)
    .width(NARROW_DIALOG_WIDTH)
    .part(does);
    let dialog = if self.refused.get() {
      dialog.person_only(ctx.t("project_settings.services.editor.delete.personOnly"))
    } else {
      dialog
    };
    dialog
      .on_enter(ctx.t("project_settings.services.editor.delete.enterDeletes"), confirm)
      .action(button)
      .under(failure)
      .render(ctx, &props.open)
  }
}

impl DeleteView {
  /// Deletes service `id` of the project at `root`, a person's press or Enter only: no MCP tool deletes a service,
  /// so synthetic input (an MCP client's) is refused and the dialog says so.
  fn confirm(&self, root: PathBuf, id: ResourceId) -> impl Fn() + Clone + Send + Sync + 'static {
    let (delete, refused) = (self.delete.clone(), self.refused.clone());
    move || {
      if is_synthetic_input() {
        tracing::warn!("synthetic input asked to delete a service; not taken");
        refused.set(true);
        return;
      }
      refused.set(false);
      if !delete.state().get_untracked().is_pending() {
        delete.run((root.clone(), id));
      }
    }
  }
}
