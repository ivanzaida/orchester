//! `/watch-module` (ORC-772, `02 · Choosing modules` `c8pALl` in `Orchestrator watch · redesign`; its badge ORC-890):
//! chosen in the slash menu it becomes the composer's *Watch* badge and opens the picker of the project's modules
//! ([`super::picker`]); the modules chosen there join the badge, and the composer's send has the conversation the
//! message goes to watch them with the message's words as their order
//! (`orchester_workspace::orchestrator::watch::watch_in`), off the UI thread and with no turn of its own, before the
//! message is sent. A new conversation is kept with its command, and the screen shows it. What another conversation
//! leads is kept nowhere: the conversation's line names it, and its card offers it. A command that applied nothing
//! says why over the composer. `/watch-task` ([`super::task`]) applies its badge the same way.

use std::{collections::VecDeque, path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
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
  runner::Runners,
};
use parking_lot::Mutex;

use super::picker::ModulePicker;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  screens::orchestrator::{View, draft::Conversation},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    slash::{Applied, Apply, Badge, Choice, Does, SlashCommand, Taking},
  },
};

/// The command's name, which its chip shows after a `/`.
pub(in crate::screens::orchestrator) const MODULE: &str = "watch-module";
/// The keys of the picker's two rows that are no module.
const WITHOUT_MODULE: &str = "without-module";
const PROJECT: &str = "project";
/// The app's key for a /watch synthetic input asked for, which only a person applies.
const PERSON_ONLY: &str = "orchestrator.watch.personOnly";

/// A /watch to apply: in the project at `root`, conversation `chat` (`new` when the command starts it) watches
/// `targets` following `order`.
#[derive(Clone)]
pub(in crate::screens::orchestrator) struct Command {
  root: PathBuf,
  chat: Chat,
  new: bool,
  targets: Vec<NamedTarget>,
  order: String,
}

impl DevtoolsInspectable for Command {}

/// The conversation a /watch was applied in, and whether the command started it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens::orchestrator) struct Done {
  chat: ChatId,
  new: bool,
}

impl DevtoolsInspectable for Done {}

/// The commands asked while a run applies others, and whether one does.
#[derive(Default)]
struct Queue {
  waiting: VecDeque<Command>,
  running: bool,
}

/// What applies the /watch commands of the composer's send. `/watch-module` and `/watch-task` both apply through it,
/// so a message with both badges asks two commands at once: one run applies them in turn, instead of the second run
/// aborting the first. Its state is the last one applied, or the first that failed.
#[derive(Clone)]
pub(in crate::screens::orchestrator) struct WatchAction {
  action: FutureAction<Command, Done, Message>,
  queue: Arc<Mutex<Queue>>,
}

impl WatchAction {
  pub(in crate::screens::orchestrator) fn state(&self) -> Signal<FutureState<Done, Message>> {
    self.action.state()
  }

  /// Applies `command`, once the commands asked before it are, in the run that applies them.
  fn run(&self, command: Command) {
    {
      let mut queue = self.queue.lock();
      if queue.running {
        queue.waiting.push_back(command);
        return;
      }
      queue.running = true;
    }
    self.action.run(command);
  }
}

pub(in crate::screens::orchestrator) fn watch_action(ctx: &mut Ctx) -> WatchAction {
  let runners = Services::of(ctx).runners;
  let queue: Arc<Mutex<Queue>> = Arc::default();
  let next = queue.clone();
  let action = ctx.future_action(move |command: Command| {
    let (runners, queue) = (runners.clone(), next.clone());
    async move {
      let mut outcome = watch(&runners, command).await;
      loop {
        let next = {
          let mut held = queue.lock();
          let next = held.waiting.pop_front();
          held.running = next.is_some();
          next
        };
        let Some(command) = next else {
          return outcome;
        };
        let applied = watch(&runners, command).await;
        // The first failure is what the notice says; each command still applies.
        outcome = outcome.and(applied);
      }
    }
  });
  WatchAction { action, queue }
}

/// Applies `command` off the UI thread, and tells the runners its conversation changed.
async fn watch(runners: &Runners, command: Command) -> Result<Done, Message> {
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

/// Once a command that started a conversation is applied, the route names that conversation while it still names a
/// new or the latest one, so History and a reload find it.
pub(in crate::screens::orchestrator) fn follow(ctx: &mut Ctx, view: &View, project: ProjectId) {
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

/// *Watch modules* in the slash menu: it takes the modules chosen in its picker, and its badge says *Watch*.
pub(in crate::screens::orchestrator) fn module_command(ctx: &Ctx, view: &View, project: &ShownProject) -> SlashCommand {
  let picker = ModulePicker::new(project, view.of.clone());
  SlashCommand {
    name: MODULE,
    icon: Icon::Eye,
    label: ctx.t("composer.slash.command.watchModule.label"),
    detail: ctx.t("composer.slash.command.watchModule.detail"),
    does: Some(Does::Take(Taking {
      picker: Arc::new(picker),
      apply: apply(view, project.open.repo_root.clone(), named_module),
      badge: Badge {
        icon: Icon::Eye,
        verb: ctx.t("composer.slash.command.watchModule.badge"),
      },
    })),
  }
}

/// The target a choice of the module picker names, a module by its name.
fn named_module(choice: &Choice) -> Option<NamedTarget> {
  let target = target_of(&choice.key)?;
  let name = matches!(target, WatchTarget::Module(_)).then(|| choice.label.to_string());
  Some(NamedTarget { target, name })
}

/// The composer's send with the badge: the conversation the message goes to (the one shown, or the new one it
/// starts) watches the targets the badge holds (`named`), with the message's words as their order. A conversation it
/// starts is kept as the message's send keeps it, with the new conversation's *Plan only* switch, so whichever of the
/// two writes it first, it plans as the person set it. Only a person's keys apply it: the order is kept as the
/// person's own words, which the agent follows, and no subscription records who asked for it, so synthetic input (an
/// MCP client's) is refused with a notice outside a QA copy (`mcp::qa_person`), and the badge and the message stay.
pub(super) fn apply(view: &View, root: PathBuf, named: fn(&Choice) -> Option<NamedTarget>) -> Apply {
  let (of, action) = (view.of.clone(), view.watching.clone());
  let (plan_only, qa_person) = (view.plan_only.clone(), view.qa_person.clone());
  Arc::new(move |applied: Applied| {
    if qa_person.refuses_input() {
      tracing::warn!("synthetic input asked to watch; not taken");
      let refused = Message::new(PERSON_ONLY);
      action.state().set(FutureState::rejected(refused, None));
      return false;
    }
    let targets: Vec<NamedTarget> = applied.choices.iter().filter_map(named).collect();
    let now = Timestamp::now();
    let (chat, new) = match of.get_untracked() {
      Some(Conversation::Chat(id)) => (Chat::named(id, "", now), false),
      Some(Conversation::New) | None => {
        let id = applied.chat.unwrap_or_else(ChatId::generate);
        let on = plan_only.on_now(Some(Conversation::New));
        (Chat::named(id, &title(&applied), now).with_plan_only(on), true)
      }
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
pub(super) fn key_of(target: WatchTarget) -> Option<Arc<str>> {
  match target {
    WatchTarget::Module(id) => Some(id.to_string().into()),
    WatchTarget::WithoutModule => Some(WITHOUT_MODULE.into()),
    WatchTarget::Project => Some(PROJECT.into()),
    WatchTarget::Task(_) => None,
  }
}

/// The target a chip of the picker names; `None` for a key it never makes.
pub(super) fn target_of(key: &str) -> Option<WatchTarget> {
  match key {
    WITHOUT_MODULE => Some(WatchTarget::WithoutModule),
    PROJECT => Some(WatchTarget::Project),
    module => module.parse().ok().map(WatchTarget::Module),
  }
}

/// A /watch that applied nothing: why, until the person dismisses it or applies one again. The refusal of synthetic
/// input is the app's own words; the rest are `watch_in`'s.
pub(in crate::screens::orchestrator) fn problem(ctx: &mut Ctx, view: &View) -> Option<Element> {
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
