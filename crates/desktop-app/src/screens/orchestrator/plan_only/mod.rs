//! The Orchestrator's plan-only mode on the screen (ORC-646; `12`–`12d` `SDd2A`, `X7BYz`, `j7Oa9v`, `y79oJ7` in
//! `Orchestrator · redesign` `hujxu`; `orchester_workspace::orchestrator::plan`): the conversation's *Plan only*
//! switch in the composer's bar and the *Mode* line over its message ([`composer`]), the plan the Orchestrator answered
//! with as a card that folds once decided ([`card`]), and *Change it*, which holds the plan being changed as a chip in
//! the composer until the correction is sent. The switch is the conversation's and kept with it; a new conversation's
//! is the screen's until its first message starts it, which keeps it.

pub(super) mod card;
pub(super) mod composer;

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::TurnMode,
  run::Answerer,
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    plan,
    send::{SendError, tools_unavailable},
    tools::ChatScope,
  },
  workspace::OpenProject,
};
use parking_lot::Mutex;

use super::draft::Conversation;
use crate::services::Services;

/// The plan *Change it* is changing: its entry, how many steps it has and when it was proposed, as its chip says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Changing {
  pub(super) seq: i64,
  pub(super) steps: usize,
  pub(super) at: Timestamp,
}

impl DevtoolsInspectable for Changing {}

/// What the screen keeps of plan-only mode: the switch as the person last set it, of the conversation it was set for;
/// the switch the conversation shown keeps (off for a new one); and the plan being changed.
#[derive(Clone)]
pub(super) struct PlanOnly {
  switched: Signal<Option<(Conversation, bool)>>,
  kept: Signal<bool>,
  pub(super) changing: Signal<Option<Changing>>,
  set: SetAction,
  /// The switch each conversation was last set to, which every save keeps: a save the next flip replaced still
  /// finishes its write, so the writes keep the newest switch whatever order they end in.
  wanted: Arc<Wanted>,
}

/// The switches the person last set, by conversation, and the lock one save holds while it writes; the screen only
/// takes the first, for as long as it sets one.
#[derive(Default)]
struct Wanted {
  switches: Mutex<HashMap<ChatId, bool>>,
  writing: Mutex<()>,
}

impl Wanted {
  /// Keeps conversation `chat`'s newest switch in the project at `root`. Blocks on SQLite.
  fn write(&self, root: &Path, chat: ChatId) -> Result<(), Message> {
    let _writing = self.writing.lock();
    let on = self.switches.lock().get(&chat).copied();
    on.map_or(Ok(()), |on| plan::set_plan_only_in(root, chat, on))
  }
}

/// A switch of conversation `chat` of the project at `root`, kept as the person last set it.
#[derive(Clone)]
struct Switching {
  root: PathBuf,
  chat: ChatId,
}

impl DevtoolsInspectable for Switching {}

type SetAction = FutureAction<Switching, (), Message>;

impl PlanOnly {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let runners = Services::of(ctx).runners;
    let wanted = Arc::new(Wanted::default());
    let set = ctx.future_action({
      let wanted = wanted.clone();
      move |switching: Switching| {
        let (runners, wanted) = (runners.clone(), wanted.clone());
        async move {
          let Switching { root, chat } = switching;
          let at = root.clone();
          background::run(move || wanted.write(&at, chat)).await??;
          runners.chat_changed(&root);
          Ok(())
        }
      }
    });
    Self {
      switched: ctx.signal(None),
      kept: ctx.signal(false),
      changing: ctx.signal(None),
      set,
      wanted,
    }
  }

  /// The conversation shown keeps the switch `kept`.
  pub(super) fn hold(&self, kept: bool) {
    if self.kept.get_untracked() != kept {
      self.kept.set(kept);
    }
  }

  /// Whether the switch is on for `of`: as the person last set it there, else as the conversation keeps it, which a
  /// save that failed leaves it as.
  pub(super) fn on(&self, of: Option<Conversation>) -> bool {
    let failed = self.set.state().get().error.is_some();
    match (self.switched.get(), of) {
      (Some((switched, on)), Some(of)) if switched == of && !failed => on,
      _ => self.kept.get(),
    }
  }

  /// [`Self::on`] in a handler, which no change of the switch renders again.
  pub(super) fn on_now(&self, of: Option<Conversation>) -> bool {
    let failed = self.set.state().get_untracked().error.is_some();
    match (self.switched.get_untracked(), of) {
      (Some((switched, on)), Some(of)) if switched == of && !failed => on,
      _ => self.kept.get_untracked(),
    }
  }

  /// Why `of`'s switch could not be kept, the last time the person flipped it.
  pub(super) fn failed(&self, of: Option<Conversation>) -> Option<Message> {
    let switched = self.switched.get().map(|(switched, _)| switched);
    self
      .set
      .state()
      .get()
      .error
      .filter(|_| switched.is_some() && switched == of)
  }

  /// Flips it for `of`, where it is `on` now; a kept conversation of the project at `root` keeps it.
  pub(super) fn flip(&self, of: Conversation, on: bool, root: &Path) {
    self.switched.set(Some((of, !on)));
    if let Conversation::Chat(chat) = of {
      self.wanted.switches.lock().insert(chat, !on);
      self.set.run(Switching {
        root: root.to_path_buf(),
        chat,
      });
    }
  }

  /// How a message sent now goes, with the switch `on`: it plans while the switch is on or a plan is being changed.
  pub(super) fn mode(&self, on: bool) -> TurnMode {
    if on || self.changing.get_untracked().is_some() {
      TurnMode::Plans
    } else {
      TurnMode::Acts
    }
  }

  /// Nothing is being changed any more: the change was sent, or the composer holds another conversation.
  pub(super) fn stop_changing(&self) {
    if self.changing.get_untracked().is_some() {
      self.changing.set(None);
    }
  }
}

/// An approval of the plan kept as entry `seq` of `project`'s conversation `chat`, sent `by` the person.
#[derive(Clone)]
pub(super) struct Approving {
  pub(super) project: OpenProject,
  pub(super) chat: ChatId,
  pub(super) seq: i64,
  pub(super) by: Answerer,
}

impl DevtoolsInspectable for Approving {}

pub(super) type ApproveAction = FutureAction<Approving, (), SendError>;
pub(super) type DismissAction = FutureAction<(PathBuf, i64), (), Message>;

/// Approves a plan, off the UI thread: the tools of the turn that carries it out are served first, as for a message
/// sent ([`super::send`]), and stop at once when the runner did not take the turn.
pub(super) fn approve_action(ctx: &mut Ctx) -> ApproveAction {
  let services = Services::of(ctx);
  let i18n = ctx.i18n().clone();
  ctx.future_action(move |approving: Approving| {
    let (services, i18n) = (services.clone(), i18n.clone());
    async move {
      let Approving { project, chat, seq, by } = approving;
      let message = ChatMessageId::generate();
      let scope = ChatScope {
        project: project.project.id(),
        name: project.project.name().as_str().to_owned(),
        root: project.repo_root.clone(),
        chat,
        on_its_own: false,
      };
      let tools = services
        .chat_tools
        .serve_turn(&services, i18n, (scope, message))
        .map_err(|error| {
          tracing::error!(%error, "the Orchestrator's tools could not be served");
          tools_unavailable()
        })?;
      let result = plan::approve(&services.runners, &project, (seq, message, by), tools).await;
      match &result {
        Err(SendError::NotSent(_)) => services.chat_tools.end(message),
        Ok(()) | Err(SendError::Unconfirmed(_)) => {
          services
            .chat_tools
            .end_with_turn(&services, project.repo_root.clone(), message);
        }
      }
      result
    }
  })
}

/// Dismisses a plan, off the UI thread.
pub(super) fn dismiss_action(ctx: &mut Ctx) -> DismissAction {
  let runners = Services::of(ctx).runners;
  ctx.future_action(move |(root, seq): (PathBuf, i64)| {
    let runners = runners.clone();
    async move {
      let at = root.clone();
      background::run(move || plan::dismiss_in(&at, seq)).await??;
      runners.chat_changed(&root);
      Ok(())
    }
  })
}
