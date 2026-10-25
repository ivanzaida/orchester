//! Sending a message to the Orchestrator and stopping its answer, off the UI thread, through
//! `orchester_workspace::orchestrator::send`. The turn's tools are served first (`mcp::chat`), so the runner is
//! told where its agent reaches them, until the turn ends; a message the runner never took stops them at once. A
//! message written while it answers goes as the person chose (`orchester_workspace::orchestrator::queue`): its tools
//! are served once its own turn starts, by the project's follower (`project::waits`). A queued message is reworded or
//! removed here too.

use std::path::PathBuf;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::id::{ChatId, ChatMessageId};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    queue::{self, WhileAnswering},
    send::{self, Outgoing, SendError},
    tools::ChatScope,
  },
  workspace::OpenProject,
};

use crate::services::Services;

/// A message to send, to the project's Orchestrator, whether it is the draft's, and how it goes while the Orchestrator
/// answers (`None`: it starts a turn of its own now).
#[derive(Clone)]
pub(super) struct Sending {
  pub(super) project: OpenProject,
  pub(super) outgoing: Outgoing,
  pub(super) from_draft: bool,
  pub(super) how: Option<WhileAnswering>,
}

impl DevtoolsInspectable for Sending {}

/// The conversation a message went to, the message, and whether it was the draft's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Sent {
  pub(super) chat: ChatId,
  pub(super) message: ChatMessageId,
  pub(super) from_draft: bool,
}

impl DevtoolsInspectable for Sent {}

/// What the person does with a queued message: takes its words back into the composer, or removes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueuedChange {
  Edit,
  Remove,
}

/// A change of the queued message `message`, with its words, of the project at `root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ChangeQueued {
  pub(super) root: PathBuf,
  pub(super) message: ChatMessageId,
  pub(super) text: String,
  pub(super) change: QueuedChange,
}

impl DevtoolsInspectable for ChangeQueued {}

/// What a change of a queued message did: the words to take back into the composer, when it was an edit of one that
/// still waited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Changed {
  pub(super) words: Option<String>,
}

impl DevtoolsInspectable for Changed {}

pub(super) type SendAction = FutureAction<Sending, Sent, SendError>;
pub(super) type StopAction = FutureAction<(OpenProject, ChatId), (), Message>;
pub(super) type QueuedAction = FutureAction<ChangeQueued, Changed, Message>;

pub(super) fn send_action(ctx: &mut Ctx) -> SendAction {
  let services = Services::of(ctx);
  let i18n = ctx.i18n().clone();
  ctx.future_action(move |sending: Sending| {
    let (services, i18n) = (services.clone(), i18n.clone());
    async move {
      let Sending {
        project,
        outgoing,
        from_draft,
        how,
      } = sending;
      let sent = Sent {
        chat: outgoing.chat,
        message: outgoing.message,
        from_draft,
      };
      if let Some(how) = how {
        queue::send_while_answering(&services.runners, &project, &outgoing, how).await?;
        return Ok(sent);
      }
      let scope = ChatScope {
        project: project.project.id(),
        name: project.project.name().as_str().to_owned(),
        root: project.repo_root.clone(),
        chat: outgoing.chat,
        on_its_own: false,
      };
      let served = services
        .chat_tools
        .serve_turn(&services, i18n, (scope, outgoing.message));
      let tools = served.map_err(|error| {
        tracing::error!(%error, "the Orchestrator's tools could not be served");
        send::tools_unavailable()
      })?;
      let result = send::send(&services.runners, &project, &outgoing, tools).await;
      match &result {
        Err(SendError::NotSent(_)) => services.chat_tools.end(outgoing.message),
        Ok(()) | Err(SendError::Unconfirmed(_)) => {
          services
            .chat_tools
            .end_with_turn(&services, project.repo_root.clone(), outgoing.message);
        }
      }
      result?;
      Ok(sent)
    }
  })
}

pub(super) fn stop_action(ctx: &mut Ctx) -> StopAction {
  let runners = Services::of(ctx).runners;
  ctx.future_action(move |(project, chat): (OpenProject, ChatId)| {
    let runners = runners.clone();
    async move { send::stop(&runners, &project, chat).await }
  })
}

/// Rewords or removes a queued message: an edit removes it and gives its words back to the composer, so it is never
/// sent as well; a message whose turn started meanwhile stays as it is.
pub(super) fn queued_action(ctx: &mut Ctx) -> QueuedAction {
  let runners = Services::of(ctx).runners;
  ctx.future_action(move |change: ChangeQueued| {
    let runners = runners.clone();
    async move {
      let (root, message) = (change.root.clone(), change.message);
      let removed = background::run(move || queue::remove_in(&root, message)).await??;
      runners.chat_changed(&change.root);
      let words = (removed && change.change == QueuedChange::Edit).then_some(change.text);
      Ok(Changed { words })
    }
  })
}
