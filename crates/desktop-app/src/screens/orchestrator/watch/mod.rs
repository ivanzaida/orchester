//! `/watch-module` (ORC-772, `02 · Choosing modules` `c8pALl` in `Orchestrator watch · redesign`): chosen in the slash
//! menu it becomes the composer's command chip and opens the picker of the project's modules ([`picker`]); the modules
//! chosen there become chips, and Ctrl Enter (Cmd Enter) has the conversation watch them with the words after the
//! chips as their order (`orchester_workspace::orchestrator::watch::watch_in`), off the UI thread and with no agent
//! turn. A new conversation is kept with its command, and the screen shows it. What another conversation leads is
//! kept nowhere: the conversation's line names it, and its card offers it. A command that applied nothing says why
//! over the composer.

mod picker;
mod rows;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, FutureState},
    events::is_synthetic_input,
  },
  node::Element,
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::{Chat, WatchTarget, watch::NamedTarget},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::watch::{WatchError, watch_in},
};

use self::picker::ModulePicker;
use super::{View, draft::Conversation};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    slash::{Applied, Apply, Does, SlashCommand, Taking},
  },
};

/// The command's name, which its chip shows after a `/`.
pub(super) const MODULE: &str = "watch-module";
/// The keys of the picker's two rows that are no module.
const WITHOUT_MODULE: &str = "without-module";
const PROJECT: &str = "project";
/// The app's key for a /watch synthetic input asked for, which only a person applies.
const PERSON_ONLY: &str = "orchestrator.watch.personOnly";

/// A /watch to apply: in the project at `root`, conversation `chat` (`new` when the command starts it) watches
/// `targets` following `order`.
#[derive(Clone)]
pub(super) struct Command {
  root: PathBuf,
  chat: Chat,
  new: bool,
  targets: Vec<NamedTarget>,
  order: String,
}

impl DevtoolsInspectable for Command {}

/// The conversation a /watch was applied in, and whether the command started it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Done {
  chat: ChatId,
  new: bool,
}

impl DevtoolsInspectable for Done {}

pub(super) type WatchAction = FutureAction<Command, Done, Message>;

pub(super) fn watch_action(ctx: &mut Ctx) -> WatchAction {
  let runners = Services::of(ctx).runners;
  ctx.future_action(move |command: Command| {
    let runners = runners.clone();
    async move {
      let Command {
        root,
        chat,
        new,
        targets,
        order,
      } = command;
      let (at, id) = (root.clone(), chat.id());
      background::run(move || watch_in(&at, &chat, &targets, &order).map_err(|error: WatchError| error.message()))
        .await??;
      runners.chat_changed(&root);
      Ok(Done { chat: id, new })
    }
  })
}

/// Once a command that started a conversation is applied, the route names that conversation while it still names a
/// new or the latest one, so History and a reload find it.
pub(super) fn follow(ctx: &mut Ctx, view: &View, project: ProjectId) {
  let navigation = Navigation::of(ctx);
  ctx.watch(&view.watching.state(), move |state| {
    if !state.is_fulfilled() {
      return;
    }
    let Some(done) = state.data.filter(|done| done.new) else {
      return;
    };
    let route = navigation.route_now();
    let on = |chat| route == Route::Project(project, ProjectPage::Orchestrator(chat));
    if on(OrchestratorChat::New) || on(OrchestratorChat::Latest) {
      navigation.replace(Route::Project(
        project,
        ProjectPage::Orchestrator(OrchestratorChat::Chat(done.chat)),
      ));
    }
  });
}

/// *Watch modules* in the slash menu: it takes the modules chosen in its picker.
pub(super) fn module_command(ctx: &Ctx, view: &View, project: &ShownProject) -> SlashCommand {
  let picker = ModulePicker::new(project, view.of.clone());
  SlashCommand {
    name: MODULE,
    icon: Icon::Eye,
    label: ctx.t("composer.slash.command.watchModule.label"),
    detail: ctx.t("composer.slash.command.watchModule.detail"),
    does: Some(Does::Take(Taking {
      picker: Arc::new(picker),
      apply: apply(view, project.open.repo_root.clone()),
    })),
  }
}

/// Ctrl Enter on the chips: the conversation shown, or a new one when the screen shows none yet, watches the targets
/// chosen with the words after them as their order. Only a person's keys apply it: the order is kept as the person's
/// own words, which the agent follows, and no subscription records who asked for it, so synthetic input (an MCP
/// client's) is refused with a notice, and the chips and the order stay.
fn apply(view: &View, root: PathBuf) -> Apply {
  let (of, action) = (view.of.clone(), view.watching.clone());
  Arc::new(move |applied: Applied| {
    if is_synthetic_input() {
      tracing::warn!("synthetic input asked to watch modules; not taken");
      let refused = Message::new(PERSON_ONLY);
      action.state().set(FutureState::rejected(refused, None));
      return false;
    }
    let targets: Vec<NamedTarget> = applied
      .choices
      .iter()
      .filter_map(|choice| {
        let target = target_of(&choice.key)?;
        let name = matches!(target, WatchTarget::Module(_)).then(|| choice.label.to_string());
        Some(NamedTarget { target, name })
      })
      .collect();
    let now = Timestamp::now();
    let (chat, new) = match of.get_untracked() {
      Some(Conversation::Chat(id)) => (Chat::named(id, "", now), false),
      Some(Conversation::New) | None => (Chat::named(ChatId::generate(), &title(&applied), now), true),
    };
    action.run(Command {
      root: root.clone(),
      chat,
      new,
      targets,
      order: applied.text,
    });
    true
  })
}

/// What a conversation the command starts is named: its order, or else what it watches.
fn title(applied: &Applied) -> String {
  if !applied.text.is_empty() {
    return applied.text.clone();
  }
  let labels: Vec<&str> = applied.choices.iter().map(|choice| &*choice.label).collect();
  labels.join(", ")
}

/// The key of the picker's chip of `target`; `None` for a task, which this picker does not offer.
fn key_of(target: WatchTarget) -> Option<Arc<str>> {
  match target {
    WatchTarget::Module(id) => Some(id.to_string().into()),
    WatchTarget::WithoutModule => Some(WITHOUT_MODULE.into()),
    WatchTarget::Project => Some(PROJECT.into()),
    WatchTarget::Task(_) => None,
  }
}

/// The target a chip of the picker names; `None` for a key it never makes.
fn target_of(key: &str) -> Option<WatchTarget> {
  match key {
    WITHOUT_MODULE => Some(WatchTarget::WithoutModule),
    PROJECT => Some(WatchTarget::Project),
    module => module.parse().ok().map(WatchTarget::Module),
  }
}

/// A /watch that applied nothing: why, until the person dismisses it or applies one again. The refusal of synthetic
/// input is the app's own words; the rest are `watch_in`'s.
pub(super) fn problem(ctx: &mut Ctx, view: &View) -> Option<Element> {
  let state = view.watching.state();
  let error = state.get().error?;
  let why = if error.key == PERSON_ONLY {
    ctx.t(PERSON_ONLY)
  } else {
    message_text(ctx, &error)
  };
  let dismiss = ActionButton::new(
    "orchestrator-watch-dismiss",
    ButtonKind::Secondary,
    ctx.t("orchestrator.watch.dismiss"),
  )
  .on_click(move || state.set(FutureState::idle()));
  Some(
    Notice::new(
      "orchestrator-watch-problem",
      NoticeTone::Danger,
      ctx.t("orchestrator.watch.failed"),
    )
    .icon(Icon::CircleAlert)
    .body(why)
    .action(dismiss)
    .into(),
  )
}
