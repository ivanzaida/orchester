//! *Compact* of the conversation's session from its context meter (ORC-309, `docs/design/context-meter.md`): offered
//! in the meter's popover and, from 80 %, beside it; it waits while a turn is under way or a message is on its way, and
//! while the session is compacted the meter says so and nothing is sent (`06b` `j7eSQO`). The compaction's CLI gets the
//! conversation's tools as a turn's does, served until it ended. A compaction the runner did not start says why over
//! the composer until it is dismissed; one that started and failed says so on its own line in the conversation.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureState},
  node::Element,
};
use orchester_core::id::{ChatId, ChatMessageId};
use orchester_workspace::{
  messages::Message,
  orchestrator::{chat::ChatView, compact, tools::ChatScope},
  workspace::OpenProject,
};

use super::View;
use crate::{
  project::ShownProject,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    context_meter::Compact,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

pub(super) type CompactAction = FutureAction<(OpenProject, ChatId), (), Message>;

/// Asks the project's runner to compact a conversation's session, serving its tools until the compaction ended.
pub(super) fn compact_action(ctx: &mut Ctx) -> CompactAction {
  let services = Services::of(ctx);
  let i18n = ctx.i18n().clone();
  ctx.future_action(move |(project, chat): (OpenProject, ChatId)| {
    let (services, i18n) = (services.clone(), i18n.clone());
    async move {
      let scope = ChatScope {
        project: project.project.id(),
        name: project.project.name().as_str().to_owned(),
        root: project.repo_root.clone(),
        chat,
        on_its_own: false,
      };
      // The tools are served under a key of the compaction's own, as a turn's under its message.
      let key = ChatMessageId::generate();
      let tools = services
        .chat_tools
        .serve_turn(&services, i18n, (scope, key))
        .map_err(|error| {
          tracing::error!(%error, "the Orchestrator's tools could not be served for a compaction");
          compact::tools_unavailable()
        })?;
      match compact::compact(&services.runners, &project, chat, tools).await {
        Ok(()) => {
          let root = project.repo_root.clone();
          services.chat_tools.end_with_compaction(&services, root, (chat, key));
          Ok(())
        }
        Err(why) => {
          services.chat_tools.end(key);
          Err(why)
        }
      }
    }
  })
}

/// What *Compact* does for conversation `shown` now.
pub(super) fn offer(view: &View, project: &ShownProject, shown: &ChatView) -> Compact {
  if shown.compacting() {
    return Compact::Running;
  }
  if shown.thinking() || view.send.state().get().is_pending() || view.compact.state().get().is_pending() {
    return Compact::Waits;
  }
  let (action, project, chat) = (view.compact.clone(), project.open.clone(), shown.chat.id());
  Compact::Ready(Arc::new(move || action.run((project.clone(), chat))))
}

/// A compaction the runner did not start: why, until the person dismisses it or compacts again.
pub(super) fn problem(ctx: &mut Ctx, view: &View) -> Option<Element> {
  let state = view.compact.state();
  let error = state.get().error?;
  let dismiss = ActionButton::new(
    "orchestrator-compact-dismiss",
    ButtonKind::Secondary,
    ctx.t("orchestrator.queued.dismiss"),
  )
  .on_click(move || state.set(FutureState::idle()));
  Some(
    Notice::new(
      "orchestrator-compact-problem",
      NoticeTone::Danger,
      ctx.t("orchestrator.context.notStarted"),
    )
    .icon(Icon::CircleAlert)
    .body(message_text(ctx, &error))
    .action(dismiss)
    .into(),
  )
}
