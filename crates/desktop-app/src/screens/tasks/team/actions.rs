//! A team project's list changes a task's lifecycle (decision D4, `Task - v2` `jDN0r` T7 `lCoNt`, T8 `vdSWS`, T10
//! `tlw0r`): Mark as done, Cancel task, Reopen, a move between Backlog and Todo, Mark as blocked and Clear blocked,
//! from a row's menu, each through the dispatcher on the task's revision as the list read it
//! (`crate::team::lifecycle`). Mark as blocked asks for its reason in a dialog. What the server refused is said once
//! over the list, with the task's key.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
  node::Element,
};
use orchester_core::{id::TaskId, sharing::TaskRevision, task::organization::TaskOrganization};

use crate::{
  services::Services,
  team::{
    TeamSource,
    act::{Acted, Acting},
    block_dialog::{Blocking, dialog},
    lifecycle::Change,
  },
  ui::notice::{Notice, NoticeTone},
};

/// A lifecycle change of one listed task: the task as the list read it, its organization then and the revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::tasks) struct Ask {
  pub task: TaskId,
  pub key: Arc<str>,
  pub base: TaskOrganization,
  pub revision: TaskRevision,
  pub change: Change,
}

impl lurq::app::component::DevtoolsInspectable for Ask {}

/// What the last ask came to, with the task it was about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::tasks) struct Answer {
  key: Arc<str>,
  acted: Acted,
}

impl lurq::app::component::DevtoolsInspectable for Answer {}

/// The question Mark as blocked waits on: whether it is open, the task it is about (an [`Ask`] whose change the reason
/// completes) and the reason typed so far.
#[derive(Clone)]
pub(in crate::screens::tasks) struct Handles {
  open: Signal<bool>,
  task: Signal<Option<Ask>>,
  reason: Signal<String>,
}

impl Handles {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      task: ctx.signal(None),
      reason: ctx.signal(String::new()),
    }
  }
}

/// The list's lifecycle changes, which its rows' menus share.
#[derive(Clone)]
pub(in crate::screens::tasks) struct TeamActions {
  action: FutureAction<Ask, Answer, ()>,
  handles: Handles,
}

impl TeamActions {
  /// Provides the changes for the team project read through `source`; made on every render.
  pub fn provide(ctx: &mut Ctx, source: &TeamSource, handles: &Handles) {
    let acting = Acting::of(&Services::of(ctx));
    let source = source.clone();
    let action = ctx.future_action(move |ask: Ask| {
      let (acting, source) = (acting.clone(), source.clone());
      async move {
        let Ask {
          task,
          key,
          base,
          revision,
          change,
        } = ask;
        let acted = match change.action(task, &base, revision) {
          Ok(action) => acting.act(&source, action).await.0,
          // A blank reason never reaches the server: the dialog does not offer it.
          Err(error) => {
            tracing::warn!(%error, "a lifecycle change was refused before it was sent");
            Acted::Failed
          }
        };
        Ok(Answer { key, acted })
      }
    });
    ctx.provide(Self {
      action,
      handles: handles.clone(),
    });
  }

  pub fn of(ctx: &mut Ctx) -> Option<Self> {
    ctx.use_context::<Self>()
  }

  /// Carries out `ask` once no other is under way.
  pub fn run(&self, ask: Ask) {
    if !self.action.state().get_untracked().is_pending() {
      self.action.run(ask);
    }
  }

  /// Opens Mark as blocked's question for `ask`'s task.
  pub fn ask_reason(&self, ask: Ask) {
    self.handles.reason.set(String::new());
    self.handles.task.set(Some(ask));
    self.handles.open.set(true);
  }

  /// What the last change did not do, once it ended: the words' key and the task's key.
  fn refused(&self) -> Option<(Arc<str>, &'static str)> {
    let state = self.action.state().get();
    let pending = state.is_pending();
    let answer = state.data.filter(|_| !pending)?;
    let words = match &answer.acted {
      Acted::Done => return None,
      Acted::Lifecycle(refusal) => lifecycle_words(*refusal),
      Acted::Refused(orchester_api::v2::AuthorityErrorCode::Forbidden) => "summary.team.refused.forbidden",
      Acted::Changed(_) => "summary.team.refused.changed",
      Acted::Unavailable => "summary.team.refused.unavailable",
      Acted::SignedOut => "summary.team.refused.signedOut",
      Acted::NeedsVersion(_) | Acted::Refused(_) | Acted::PipelineRefused(_) | Acted::Failed => {
        "summary.team.refused.other"
      }
    };
    Some((answer.key, words))
  }
}

const fn lifecycle_words(refusal: orchester_workspace::actions::LifecycleRefusal) -> &'static str {
  use orchester_workspace::actions::LifecycleRefusal;
  match refusal {
    LifecycleRefusal::NotHolder => "summary.team.refused.notHolder",
    LifecycleRefusal::StopRunFirst => "summary.team.refused.stopRunFirst",
    LifecycleRefusal::MissingReason => "summary.team.refused.missingReason",
  }
}

/// Why the last change did not happen, over the list: *Couldn't change KON-12* and the server's reason.
pub(in crate::screens::tasks) fn refusal(ctx: &mut Ctx) -> Option<Element> {
  let actions = TeamActions::of(ctx)?;
  let (key, words) = actions.refused()?;
  Some(
    Notice::new(
      "tasks-team-refused",
      NoticeTone::Danger,
      ctx.t_args("tasks.team.lifecycle.refusedTitle", [("task", key.as_ref())]),
    )
    .body(ctx.t(words))
    .into(),
  )
}

/// Mark as blocked's question: the reason, which is not blank.
pub(in crate::screens::tasks) fn blocking_dialog(ctx: &mut Ctx) -> Option<Element> {
  let actions = TeamActions::of(ctx)?;
  let handles = actions.handles.clone();
  if !handles.open.get() {
    return None;
  }
  let ask = handles.task.get()?;
  let key = ask.key.clone();
  let blocking = Blocking {
    id: "tasks-team-block",
    key: &key,
    reason: &handles.reason,
    open: &handles.open,
  };
  Some(dialog(ctx, &blocking, move |reason| {
    actions.run(Ask {
      change: Change::Block(reason),
      ..ask.clone()
    });
  }))
}
