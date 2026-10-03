//! A request that ended before its answer: the runner answers that it no longer waits when its agent can
//! no longer receive an answer (the agent ended, its runner was restarted, its run stopped). The runner
//! records it as ended without an answer; the dialog drops it as soon as the answer comes back, says so
//! over the next request and in the status bar, and the inline card reads that it ended (built, not
//! drawn).

use std::{path::Path, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::id::TaskId;

use super::DecisionAction;
use crate::{
  activity::{Activity, Place, Target},
  messages::Message,
  screens::project::{approval_queue::ApprovalQueue, pages::Shown},
};

/// Why an answer to a request did not go through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::project::task) enum Unanswered {
  /// Request `approval` of `task` no longer waits: its agent can no longer receive an answer.
  Ended { approval: String, task: Option<TaskId> },
  /// Request `approval` was answered already (a second click, or Orchester answered it meanwhile): it
  /// leaves without saying it ended.
  Answered { approval: String },
  /// Anything else, as the dialog and the card say it.
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for Unanswered {}

impl Unanswered {
  /// What failed, to show beside the answers; nothing for a request that ended or was answered, which
  /// leaves.
  pub(in crate::screens::project::task) fn failure(&self) -> Option<&Message> {
    match self {
      Self::Ended { .. } | Self::Answered { .. } => None,
      Self::Failed(message) => Some(message),
    }
  }

  /// Whether this is the end of request `id`.
  pub(in crate::screens::project::task) fn ended(&self, id: &str) -> bool {
    matches!(self, Self::Ended { approval, .. } if approval == id)
  }

  /// Whether this says request `id` was answered already.
  pub(in crate::screens::project::task) fn answered(&self, id: &str) -> bool {
    matches!(self, Self::Answered { approval } if approval == id)
  }
}

/// Once the dialog's `action` comes back with an [`Unanswered::Ended`], the request leaves the dialog (the
/// next waiting one shows, or it closes) and the status bar says it ended; with an
/// [`Unanswered::Answered`] it only leaves. Called where `action` is created.
pub(super) fn drop_when_ended(ctx: &mut Ctx, action: &DecisionAction) {
  let queue = ctx.use_context::<ApprovalQueue>();
  let activity = Activity::of(ctx);
  let project: Option<(Arc<str>, _)> = ctx
    .use_context::<Shown>()
    .map(|shown| (shown.open.project.name().as_str().into(), shown.open.repo_root));
  ctx.watch(&action.state(), move |state| {
    let (approval, task) = match state.error.as_ref().filter(|_| !state.is_pending()) {
      Some(Unanswered::Ended { approval, task }) => (approval, task),
      Some(Unanswered::Answered { approval }) => {
        if let Some(queue) = &queue {
          queue.drop_answered(approval);
        }
        return;
      }
      Some(Unanswered::Failed(_)) | None => return,
    };
    if let Some(queue) = &queue {
      queue.drop_ended(approval);
    }
    let (name, target) = match &project {
      Some((name, repo_root)) => (Some(name.clone()), notice_target(repo_root, *task)),
      None => (None, Target::Nothing),
    };
    activity.record(name, Message::new("statusBar.approvals.ended"), target);
  });
}

/// Where the status bar's notice leads: the task of the request, else the project's Overview.
fn notice_target(repo_root: &Path, task: Option<TaskId>) -> Target {
  Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: task.map_or(Place::Overview, Place::Task),
  }
}
