//! *Delete…* of a conversation asks in a modal first (`08e · History · Delete confirmation` `I4xzOf`, a 460-wide
//! [`Dialog`] in the danger tone): *Delete “<title>”?* over what deleting does — its messages and the files attached
//! to them are removed from this computer, the tasks, runs and decisions it started stay, and it cannot be undone while
//! *Archive* keeps it out of History instead — then *Enter deletes* and *Esc cancels*, *Cancel* and *Delete
//! conversation*. A delete that failed says why under those lines and the dialog stays; one that succeeded closes it,
//! and the conversation the screen showed falls back to a new one. Only a person deletes: an MCP client's press or
//! Enter is refused to it, and the dialog stays as it was.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::ChatId;
use orchester_store::ChatListing;
use orchester_workspace::{background, messages::Message, orchestrator::chat::delete_in};

use super::{Deleting, Rows};
use crate::{
  mcp::qa_person::QaPerson,
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  services::Services,
  theme::NARROW_DIALOG_WIDTH,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone, line, lines},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

/// The modal asking whether to delete `listing`, which the screen shows when `shown`.
pub(super) fn dialog(ctx: &mut Ctx, listing: &ChatListing, rows: &Rows, shown: bool) -> Element {
  ctx.mount_keyed::<DeleteView>(
    &format!("orchestrator-history-delete-{}", listing.chat.id()),
    DeleteProps {
      listing: listing.clone(),
      rows: rows.clone(),
      shown,
    },
  )
}

#[derive(Clone, PartialEq)]
struct DeleteProps {
  listing: ChatListing,
  rows: Rows,
  shown: bool,
}

impl DevtoolsInspectable for DeleteProps {}

/// Deletes a conversation of the project at the path.
type DeleteAction = FutureAction<(Arc<Path>, ChatId), (), Message>;

struct DeleteView {
  /// The dialog shows; *Cancel*, Esc and the scrim end the question.
  open: Signal<bool>,
  delete: DeleteAction,
  /// Whether deleting takes synthetic input as the person's: in a QA copy only.
  qa_person: QaPerson,
}

impl Component for DeleteView {
  type Props = DeleteProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<DeleteProps>().clone();
    let open = ctx.signal(true);
    let asked = props.rows.deleting.clone();
    ctx.watch(&open, move |open| {
      if !*open {
        asked.set(Deleting(None));
      }
    });
    let runners = Services::of(ctx).runners;
    let delete: DeleteAction = ctx.future_action(move |(root, chat): (Arc<Path>, ChatId)| {
      let runners = runners.clone();
      async move {
        let kept = root.clone();
        background::run(move || delete_in(&kept, chat))
          .await
          .and_then(|deleted| deleted)?;
        runners.chat_changed(&root);
        Ok(())
      }
    });
    let navigation = Navigation::of(ctx);
    let (project, chat, shown) = (props.rows.project, props.listing.chat.id(), props.shown);
    let asked = props.rows.deleting;
    ctx.watch(&delete.state(), move |state| {
      if !state.is_fulfilled() || state.data.is_none() {
        return;
      }
      let at = |chat| Route::Project(project, ProjectPage::Orchestrator(chat));
      if shown || navigation.route_now() == at(OrchestratorChat::Chat(chat)) {
        navigation.replace(at(OrchestratorChat::New));
      }
      asked.set(Deleting(None));
    });
    Self {
      open,
      delete,
      qa_person: QaPerson::of(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DeleteProps>().clone();
    let listing = &props.listing;
    let state = self.delete.state().get();
    let pending = state.is_pending();
    let messages = match listing.messages {
      0 => ctx.t("orchestrator.historyPanel.delete.messages.none"),
      1 => ctx.t("orchestrator.historyPanel.delete.messages.one"),
      count => ctx.t_args(
        "orchestrator.historyPanel.delete.messages.other",
        [("count", count.to_string())],
      ),
    };
    let does = lines(
      "orchestrator-history-delete-does",
      [
        line("orchestrator-history-delete-messages", Icon::MessagesSquare, &messages),
        line(
          "orchestrator-history-delete-stays",
          Icon::ListChecks,
          &ctx.t("orchestrator.historyPanel.delete.stays"),
        ),
        line(
          "orchestrator-history-delete-final",
          Icon::Archive,
          &ctx.t("orchestrator.historyPanel.delete.final"),
        ),
      ],
    );
    let confirm = self.confirm(props.rows.root.clone(), listing.chat.id());
    let button = ActionButton::new(
      "orchestrator-history-delete-confirm",
      ButtonKind::Danger,
      ctx.t("orchestrator.historyPanel.delete.confirm"),
    )
    .icon(Icon::Trash2);
    let button = if pending {
      button.loading()
    } else {
      button.on_click(confirm.clone())
    };
    let failure = state.error.filter(|_| !pending).map(|reason| {
      let reason = message_text(ctx, &reason);
      field_error(
        "orchestrator-history-delete-error",
        &ctx.t_args("orchestrator.historyPanel.delete.failed", [("reason", &*reason)]),
      )
      .into()
    });
    let dialog = Dialog::new(
      "orchestrator-history-delete",
      Icon::Trash2,
      ctx.t_args(
        "orchestrator.historyPanel.delete.title",
        [("title", listing.chat.title())],
      ),
      ctx.t("orchestrator.historyPanel.delete.cancel"),
    )
    .tone(Tone::Danger)
    .width(NARROW_DIALOG_WIDTH)
    .part(does);
    dialog
      .on_enter(ctx.t("orchestrator.historyPanel.delete.enterDeletes"), confirm)
      .action(button)
      .under(failure)
      .render(ctx, &self.open)
  }
}

impl DeleteView {
  /// Deletes `chat` of the project at `root`, a person's press or Enter only: what goes cannot be brought back, and no
  /// MCP tool deletes a conversation either, so synthetic input (an MCP client's) is refused to it outside a QA copy
  /// (`mcp::qa_person`).
  fn confirm(&self, root: Arc<Path>, chat: ChatId) -> impl Fn() + Clone + Send + Sync + 'static {
    let (delete, qa_person) = (self.delete.clone(), self.qa_person.clone());
    move || {
      if qa_person.refuses("delete a conversation of the Orchestrator") {
        return;
      }
      if !delete.state().get_untracked().is_pending() {
        delete.run((root.clone(), chat));
      }
    }
  }
}
