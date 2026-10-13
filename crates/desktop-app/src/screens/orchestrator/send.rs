//! Sending a message to the Orchestrator and stopping its answer, off the UI thread, through
//! `orchester_workspace::orchestrator::send`. The chat's tools are served first (`mcp::chat`), so the runner is
//! told where its agent reaches them.

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::id::ChatId;
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

/// The conversation a message went to, and whether it was the draft's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Sent {
  pub(super) chat: ChatId,
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
      let tools = services.chat_tools.tools_for(&services, i18n, scope).map_err(|error| {
        tracing::error!(%error, "the Orchestrator's tools could not be served");
        send::tools_unavailable()
      })?;
      send::send(&services.runners, &project, &outgoing, tools).await?;
      Ok(Sent {
        chat: outgoing.chat,
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
