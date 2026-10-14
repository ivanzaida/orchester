//! Sending a message to the Orchestrator and stopping its answer, off the UI thread, through
//! `orchester_workspace::orchestrator::send`. The turn's tools are served first (`mcp::chat`), so the runner is
//! told where its agent reaches them, until the turn ends; a message the runner never took stops them at once.

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::id::{ChatId, ChatMessageId};
use orchester_workspace::{
  messages::Message,
  orchestrator::{
    send::{self, Outgoing, SendError},
    tools::ChatScope,
  },
  workspace::OpenProject,
};

use crate::services::Services;

/// A message to send, to the project's Orchestrator, and whether it is the draft's.
#[derive(Clone)]
pub(super) struct Sending {
  pub(super) project: OpenProject,
  pub(super) outgoing: Outgoing,
  pub(super) from_draft: bool,
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

pub(super) type SendAction = FutureAction<Sending, Sent, SendError>;
pub(super) type StopAction = FutureAction<(OpenProject, ChatId), (), Message>;

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
      } = sending;
      let scope = ChatScope {
        project: project.project.id(),
        name: project.project.name().as_str().to_owned(),
        root: project.repo_root.clone(),
        chat: outgoing.chat,
      };
      let served = services.chat_tools.serve_turn(&services, i18n, (scope, outgoing.message));
      let tools = served.map_err(|error| {
        tracing::error!(%error, "the Orchestrator's tools could not be served");
        send::tools_unavailable()
      })?;
      let sent = send::send(&services.runners, &project, &outgoing, tools).await;
      match &sent {
        Err(SendError::NotSent(_)) => services.chat_tools.end(outgoing.message),
        Ok(()) | Err(SendError::Unconfirmed(_)) => {
          services
            .chat_tools
            .end_with_turn(&services, project.repo_root.clone(), outgoing.message);
        }
      }
      sent?;
      Ok(Sent {
        chat: outgoing.chat,
        message: outgoing.message,
        from_draft,
      })
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
