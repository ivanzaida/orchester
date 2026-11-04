//! *Fork from here* (ORC-334; its row of the message menu in 09 `GYAPg`, the fork in 11 `RT5T7`,
//! `orchester_workspace::orchestrator::fork`): without a question, since nothing is lost, a new conversation titled
//! *{title} · fork* is kept with the messages up to the point, and the screen opens it; on the person's own message it
//! stops before it, and the message's words go to the fork's composer. The original stays as it was. It may be made
//! while the Orchestrator answers: it takes what was kept so far. Automation may fork too, since it drops nothing. A
//! fork that failed keeps nothing, and the chat says why until the person dismisses it.

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureState},
  node::Element,
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::RewindPoint,
};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::fork::{Forked, fork_in},
  workspace::OpenProject,
};

use super::Rewinding;
use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  screens::orchestrator::{View, draft},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// A fork of conversation `from` of `project`, titled `title` now, at `point`.
#[derive(Clone)]
pub(in crate::screens::orchestrator) struct Fork {
  project: OpenProject,
  from: ChatId,
  title: String,
  point: RewindPoint,
}

impl lurq::app::component::DevtoolsInspectable for Fork {}

/// A fork kept in `project`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::orchestrator) struct Made {
  project: ProjectId,
  forked: Forked,
}

impl lurq::app::component::DevtoolsInspectable for Made {}

pub(in crate::screens::orchestrator) type ForkAction = FutureAction<Fork, Made, Message>;

/// The action that keeps a fork, titled after its original.
pub(super) fn action(ctx: &mut Ctx) -> ForkAction {
  let services = Services::of(ctx);
  let i18n = ctx.i18n().clone();
  ctx.future_action(move |fork: Fork| {
    let (services, i18n) = (services.clone(), i18n.clone());
    async move {
      let Fork {
        project,
        from,
        title,
        point,
      } = fork;
      let title = i18n
        .t_args("orchestrator.fork.title", [("title", title.as_str())])
        .to_string();
      let root = project.repo_root.clone();
      let forked = background::run(move || fork_in(&root, (from, point), &title))
        .await
        .and_then(|forked| forked)?;
      services.runners.chat_changed(&project.repo_root);
      Ok(Made {
        project: project.project.id(),
        forked,
      })
    }
  })
}

impl Rewinding {
  /// Forks the conversation held at `point` into a new one of `project`; nothing while a fork is being made or no
  /// conversation is held.
  pub(super) fn fork(&self, project: &OpenProject, point: RewindPoint) {
    if self.fork.state().get_untracked().is_pending() {
      return;
    }
    let Some((from, title)) = self
      .held
      .lock()
      .as_ref()
      .map(|holding| (holding.chat, holding.title.to_string()))
    else {
      return;
    };
    self.fork.run(Fork {
      project: project.clone(),
      from,
      title,
      point,
    });
  }
}

/// Once a fork is kept, the words it gave back become its draft, and the screen opens it with the focus in the
/// composer: Back returns to the original.
pub(super) fn watch(ctx: &mut Ctx, view: &View) {
  let navigation = Navigation::of(ctx);
  let view = view.clone();
  ctx.watch(&view.rewind.fork.state(), move |state| {
    let Some(made) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
      return;
    };
    let chat = made.forked.chat;
    if let Some(words) = &made.forked.words {
      draft::set(made.project, draft::Conversation::Chat(chat), words);
    }
    navigation.go(Route::Project(
      made.project,
      ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)),
    ));
    view.focus.set(true);
  });
}

/// A fork that failed: why, until the person dismisses it or forks again.
pub(super) fn problem(ctx: &mut Ctx, view: &View) -> Option<Element> {
  let state = view.rewind.fork.state();
  let why = state.get().error?;
  let dismiss = ActionButton::new(
    "orchestrator-fork-dismiss",
    ButtonKind::Secondary,
    ctx.t("orchestrator.fork.dismiss"),
  )
  .on_click(move || state.set(FutureState::idle()));
  Some(
    Notice::new(
      "orchestrator-fork-failed",
      NoticeTone::Danger,
      ctx.t("orchestrator.fork.failed"),
    )
    .icon(Icon::CircleAlert)
    .body(ctx.t_args(
      "orchestrator.fork.failedBody",
      [("why", message_text(ctx, &why).to_string())],
    ))
    .action(dismiss)
    .into(),
  )
}
