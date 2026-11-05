//! The latest run's merge on its task page (ORC-731; `Review · redesign`, Row 4 `XHQpC`: 01i `k9GBy`, 01j `k4C0Es`):
//! whether the run waits to be merged or its last merge attempt did not land, read from its merge attempts while it
//! waits for review ([`reader`]); the one action that moves it on, which the Summary's top card and the header's
//! primary action both show with its key cap ([`action`]); and ⌘↵ (Ctrl+Enter elsewhere), which presses it from
//! anywhere on the page but a text field ([`shortcut`]).

mod action;
pub(super) mod reader;

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, events::KeyboardEvent},
  core::Signal,
};
use orchester_core::id::RunId;
use orchester_workspace::{messages::Message, tasks::integration_jobs::JobState};

pub use self::action::{MergeAction, action};
use super::{
  TaskContext,
  summary::{Waiting, waiting},
};
use crate::{
  navigation::press::OpenIn,
  screens::merges::attempt::{
    actions::{Actions, Busy},
    standing::Standing,
  },
  services::Services,
  ui::pressable::Press,
};

/// Where the latest run's merge stands while the run waits to be merged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Merge {
  /// No merge was tried, or one goes on: Orchester or the merge queue tries again, or the queue waits for approval.
  Ready,
  /// The last attempt did not land, and stands so.
  NotMerged(Standing),
}

impl Merge {
  /// How `task`'s latest run stands, while it waits for its review and its merge did not land; `None` otherwise.
  pub fn of(task: &TaskContext) -> Option<Self> {
    let run = task.detail.run.as_ref()?;
    if waiting(task) != Some(Waiting::Ready) || task.detail.merge.is_some() {
      return None;
    }
    // A merge git made shows as made before the task is read again with it; one it could not record is not merged.
    let job = task.jobs.run(run.id());
    if job.merged.is_some() && !matches!(job.merge, JobState::Failed(_)) {
      return None;
    }
    Some(match task.attempt.as_deref().and_then(Standing::of) {
      None | Some(Standing::Merged | Standing::Retrying(_) | Standing::AwaitingApproval) => Self::Ready,
      Some(standing) => Self::NotMerged(standing),
    })
  }
}

/// The keys that press the action, as its cap shows them: ⌘↵ on macOS, Ctrl+Enter elsewhere.
pub fn keys(ctx: &Ctx) -> Arc<str> {
  if cfg!(target_os = "macos") {
    ctx.t("task.merge.keysCmd")
  } else {
    ctx.t("task.merge.keysCtrl")
  }
}

/// ⌘↵ (Ctrl+Enter elsewhere) presses `press`, the action as it stands, unless a text field has the keys or something
/// took them already; nothing while the action cannot be pressed.
pub fn shortcut(press: Option<Press>) -> impl Fn(KeyboardEvent) + use<> {
  move |event: KeyboardEvent| {
    let held = if cfg!(target_os = "macos") {
      event.meta && !event.ctrl
    } else {
      event.ctrl && !event.meta
    };
    if event.key != "Enter" || !held || event.shift || event.alt || event.composing {
      return;
    }
    if event.default_prevented() || event.text_input_focused {
      return;
    }
    let Some(press) = &press else {
      return;
    };
    event.prevent_default();
    match press {
      Press::Run(run) => run(),
      Press::Open(open) => open(OpenIn::ActiveTab),
    }
  }
}

/// The page's own work on a merge that did not land (*Record merge*, *Put files back*, *Try again* in the merge
/// queue), shared by the top card and the header: what is under way, why the last did not happen, and a count that
/// reads the run's attempts again after each.
#[derive(Clone)]
pub struct MergeWork {
  pub failed: Signal<Option<Message>>,
  pub busy: Signal<Busy>,
  pub again: Signal<u64>,
}

impl MergeWork {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      failed: ctx.signal(None),
      busy: ctx.signal(Busy::Idle),
      again: ctx.signal(0),
    }
  }

  /// The work for `run` of `task`'s project.
  fn actions(&self, ctx: &mut Ctx, task: &TaskContext, run: RunId) -> Actions {
    Actions::new(
      (&task.project.open.repo_root, Services::of(ctx).programs, run),
      ctx.t("task.listSeparator"),
      (self.failed.clone(), self.busy.clone(), self.again.clone()),
    )
  }
}
