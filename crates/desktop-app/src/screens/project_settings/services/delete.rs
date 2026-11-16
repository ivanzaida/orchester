//! *Delete service*, *Delete stack* and *Delete suite* ask in a modal first (confirmations are modals, the person's
//! rule of 2026-10-28; the narrow [`Dialog`] in the danger tone, as `08e · History · Delete confirmation` `I4xzOf`):
//! *Delete <name>?* over what deleting does — what can no longer start it or resolve it, that runs that started it
//! keep their version, and that it cannot be undone — then *Enter deletes* and *Esc cancels*, *Cancel* and the delete
//! itself. A delete that failed says why and the dialog stays; one that worked opens Services. Only a person deletes:
//! an MCP client's press or Enter is refused, and the dialog says so.

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

use super::super::TabProps;
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

/// What is deleted: its element ids' start and its words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Deleted {
  /// `settings-service`: the button is `<prefix>-delete`, the dialog `<prefix>-delete-dialog`.
  prefix: &'static str,
  /// The page's button that asks.
  opens: &'static str,
  /// What its first line says it no longer does, with its glyph.
  gone: (Icon, &'static str),
  /// The dialog's button that deletes.
  button: &'static str,
  person_only: &'static str,
  failed: &'static str,
}

impl Deleted {
  pub(super) const SERVICE: Self = Self {
    prefix: "settings-service",
    opens: "project_settings.services.editor.delete.button",
    gone: (Icon::Server, "project_settings.services.editor.delete.gone"),
    button: "project_settings.services.editor.delete.confirm",
    person_only: "project_settings.services.editor.delete.personOnly",
    failed: "project_settings.services.editor.delete.failed",
  };
  pub(super) const STACK: Self = Self {
    prefix: "settings-stack",
    opens: "project_settings.services.stack.delete.button",
    gone: (Icon::Layers, "project_settings.services.stack.delete.gone"),
    button: "project_settings.services.stack.delete.confirm",
    person_only: "project_settings.services.stack.delete.personOnly",
    failed: "project_settings.services.stack.delete.failed",
  };
  pub(super) const SUITE: Self = Self {
    prefix: "settings-suite",
    opens: "project_settings.services.suite.delete.button",
    gone: (Icon::FlaskConical, "project_settings.services.suite.delete.gone"),
    button: "project_settings.services.suite.delete.confirm",
    person_only: "project_settings.services.suite.delete.personOnly",
    failed: "project_settings.services.suite.delete.failed",
  };

  /// The button that opens the modal while `open` holds, in the danger ghost look (`Delete stack` `v70gf`).
  pub(super) fn button(self, ctx: &Ctx, open: &Signal<bool>) -> Element {
    let open = open.clone();
    ActionButton::new(
      format!("{}-delete", self.prefix),
      ButtonKind::DangerGhost,
      ctx.t(self.opens),
    )
    .icon(Icon::Trash2)
    .on_click(move || open.set(true))
    .into()
  }
}

/// The modal asking whether to delete `what` (`id`, `name`), while `open` holds.
pub(super) fn dialog(
  ctx: &mut Ctx,
  tab: &TabProps,
  (what, id, name): (Deleted, ResourceId, Arc<str>),
  open: &Signal<bool>,
) -> Element {
  ctx.mount_keyed::<DeleteView>(
    &format!("{}-delete-{id}", what.prefix),
    DeleteProps {
      tab: tab.clone(),
      what,
      id,
      name,
      open: open.clone(),
    },
  )
}

#[derive(Clone)]
struct DeleteProps {
  tab: TabProps,
  what: Deleted,
  id: ResourceId,
  name: Arc<str>,
  open: Signal<bool>,
}

impl PartialEq for DeleteProps {
  fn eq(&self, other: &Self) -> bool {
    (self.what, self.id, &self.name) == (other.what, other.id, &other.name) && self.tab == other.tab
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
    let prefix = props.what.prefix;
    let state = self.delete.state().get();
    let pending = state.is_pending();
    let (glyph, gone) = props.what.gone;
    let does = lines(
      &format!("{prefix}-delete-does"),
      [
        line(format!("{prefix}-delete-gone"), glyph, &ctx.t(gone)),
        line(
          format!("{prefix}-delete-kept"),
          Icon::History,
          &ctx.t("project_settings.services.editor.delete.kept"),
        ),
        line(
          format!("{prefix}-delete-final"),
          Icon::Trash2,
          &ctx.t("project_settings.services.editor.delete.final"),
        ),
      ],
    );
    let confirm = self.confirm(props.tab.project.open.repo_root.clone(), props.id);
    let button = ActionButton::new(
      format!("{prefix}-delete-confirm"),
      ButtonKind::Danger,
      ctx.t(props.what.button),
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
        &format!("{prefix}-delete-error"),
        &ctx.t_args(props.what.failed, [("reason", &*reason)]),
      ))
    });
    let dialog_id = format!("{prefix}-delete-dialog");
    let dialog = Dialog::new(
      &dialog_id,
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
      dialog.person_only(ctx.t(props.what.person_only))
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
  /// Deletes `id` of the project at `root`, a person's press or Enter only: no MCP tool deletes a service, a stack or a
  /// suite, so synthetic input (an MCP client's) is refused and the dialog says so.
  fn confirm(&self, root: PathBuf, id: ResourceId) -> impl Fn() + Clone + Send + Sync + 'static {
    let (delete, refused) = (self.delete.clone(), self.refused.clone());
    move || {
      if is_synthetic_input() {
        tracing::warn!("synthetic input asked to delete a service, a stack or a suite; not taken");
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
