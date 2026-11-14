//! `/watching` (ORC-775, `06 · /watching` `Ff7AD` in `Orchestrator watch · redesign`): *What I watch* in the slash
//! menu, offered while the conversation leads something. Chosen, it keeps the person's command
//! (`orchester_workspace::orchestrator::watch::watching_in`) off the UI thread, with no agent turn and nothing spent,
//! also while an answer is under way. Its row shows the command's bubble and Orchester's answer under it
//! ([`answer`]), built from what the conversation leads now (`orchester_workspace::orchestrator::watching`), read
//! again as runs and tasks change: an earlier `/watching` shows the live state too. *Stop watching* on a
//! subscription's head row ends it at once, as × on its header chip does, and so does `/unwatch` ([`unwatch`],
//! ORC-774) for what is chosen in its picker. A subscription that leads its target to the end shows how it takes it
//! and within which ceilings, and the person changes both there ([`lead`], [`ceilings`]; `L02` `HpcQJ` on the board
//! `Orchestrator autonomy · frames to approve`, OA-10). A command, a stop or a change that applied nothing says why
//! over the composer.

mod answer;
mod ceilings;
mod lead;
mod unwatch;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::ChatId,
  orchestrator::{Ceilings, Chat, WatchTarget, watch::NamedTarget},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    watch::{StopBy, StopWhat, WatchError, follow_in, set_ceilings_in, stop_in, watching_in},
    watching::Watching,
  },
};

pub(super) use self::answer::row;
use super::data::{Read, Reads};
use crate::{
  mcp::qa_person::QaPerson,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    chat::follow::Follow,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    slash::{Does, SlashCommand},
  },
};

/// The command's name, as its row in the slash menu and its chip show it.
const COMMAND: &str = "watching";

/// A `/watching` to keep: conversation `chat` of the project at `root` asked it.
#[derive(Clone)]
pub(super) struct Asked {
  root: PathBuf,
  chat: ChatId,
}

impl DevtoolsInspectable for Asked {}

/// A stop of `what` that conversation `chat` of the project at `root` watches: *Stop watching* on a head row of the
/// answer, or `/unwatch` (`by`).
#[derive(Clone)]
pub(super) struct Stopping {
  root: PathBuf,
  chat: ChatId,
  what: StopWhat,
  by: StopBy,
}

impl DevtoolsInspectable for Stopping {}

/// What the person changes of how a conversation takes a target it leads to the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LeadChange {
  /// It only follows the target from now on.
  Follow,
  /// It leads it on within these ceilings.
  Ceilings(Ceilings),
}

/// A change of how conversation `chat` of the project at `root` takes `target`.
#[derive(Clone)]
pub(super) struct Leading {
  root: PathBuf,
  chat: ChatId,
  target: WatchTarget,
  change: LeadChange,
}

impl DevtoolsInspectable for Leading {}

/// The conversation shown and what it was read to lead, as the screen's last pass read them.
#[derive(Clone, Debug, Default, PartialEq)]
struct Held(Option<(ChatId, Read<Arc<Watching>>)>);

impl DevtoolsInspectable for Held {}

/// What the screen keeps for `/watching`: keeping the command, stopping a subscription from the answer or with
/// `/unwatch`, and what the conversation shown leads, which the slash menu, the `/unwatch` picker and the rows read.
#[derive(Clone)]
pub(super) struct WhatIWatch {
  ask: FutureAction<Asked, (), Message>,
  stop: FutureAction<Stopping, (), Message>,
  lead: FutureAction<Leading, (), Message>,
  held: Signal<Held>,
  /// Whether a stop takes synthetic input as the person's: in a QA copy only.
  qa_person: QaPerson,
}

/// What a `/watching` row shows of the live state: what the conversation leads (`None` for a new one), whether a
/// stop or a change of how a target is taken is under way, and what *Stop watching* and such a change do.
#[derive(Clone)]
pub(super) struct Live {
  pub(super) read: Option<Read<Arc<Watching>>>,
  pub(super) stopping: bool,
  pub(super) changing: bool,
  pub(super) stop: Arc<dyn Fn(WatchTarget) + Send + Sync>,
  /// Changes how a target is taken; whether the change started.
  pub(super) change: Arc<dyn Fn(WatchTarget, LeadChange) -> bool + Send + Sync>,
}

/// Compared by what it shows; the stop and the change are the screen's.
impl PartialEq for Live {
  fn eq(&self, other: &Self) -> bool {
    self.read == other.read && self.stopping == other.stopping && self.changing == other.changing
  }
}

impl WhatIWatch {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let runners = Services::of(ctx).runners;
    let asking = runners.clone();
    let ask = ctx.future_action(move |Asked { root, chat }: Asked| {
      let runners = asking.clone();
      async move {
        let at = root.clone();
        background::run(move || {
          watching_in(&at, &Chat::named(chat, "", Timestamp::now())).map_err(|error: WatchError| error.message())
        })
        .await??;
        runners.chat_changed(&root);
        Ok(())
      }
    });
    let leading = runners.clone();
    let lead = ctx.future_action(move |asked: Leading| {
      let runners = leading.clone();
      async move {
        let Leading {
          root,
          chat,
          target,
          change,
        } = asked;
        let at = root.clone();
        background::run(move || {
          let changed = match change {
            LeadChange::Follow => follow_in(&at, chat, target),
            LeadChange::Ceilings(ceilings) => set_ceilings_in(&at, chat, target, ceilings),
          };
          changed.map_err(|error: WatchError| error.message())
        })
        .await??;
        runners.chat_changed(&root);
        Ok(())
      }
    });
    let stop = ctx.future_action(move |Stopping { root, chat, what, by }: Stopping| {
      let runners = runners.clone();
      async move {
        let at = root.clone();
        background::run(move || {
          stop_in(&at, &Chat::named(chat, "", Timestamp::now()), &what, by)
            .map(|_| ())
            .map_err(|error: WatchError| error.message())
        })
        .await??;
        runners.chat_changed(&root);
        Ok(())
      }
    });
    Self {
      ask,
      stop,
      lead,
      held: ctx.signal(Held::default()),
      qa_person: QaPerson::of(ctx),
    }
  }

  /// Holds what `reads` found for the conversation shown; the `/unwatch` picker follows it.
  pub(super) fn hold(&self, reads: &Reads) {
    let held = Held(reads.target.zip(reads.watching.clone()));
    if self.held.get_untracked() != held {
      self.held.set(held);
    }
  }

  /// The conversation shown, while it was read to lead something.
  fn watches(&self) -> Option<ChatId> {
    let Held(held) = self.held.get_untracked();
    let (chat, read) = held?;
    read
      .ready()
      .filter(|watching| !watching.subscriptions.is_empty())
      .map(|_| chat)
  }

  /// Stops `what` that conversation `chat` of the project at `root` watches, `by` the person: whether it started. A
  /// stop is a person's press only: stopping stays the person's command in the conversation and no MCP tool stops a
  /// subscription either, so synthetic input (an MCP client's) is refused to it outside a QA copy (`mcp::qa_person`),
  /// and the subscriptions go on. Nothing starts while a stop is under way.
  fn stop_by_person(&self, (root, chat): (PathBuf, ChatId), what: StopWhat, by: StopBy) -> bool {
    if self
      .qa_person
      .refuses("stop what a conversation of the Orchestrator watches")
    {
      return false;
    }
    if self.stop.state().get_untracked().is_pending() {
      return false;
    }
    self.stop.run(Stopping { root, chat, what, by });
    true
  }

  /// Changes how conversation `chat` of the project at `root` takes `target`: whether it started. Whether a target is
  /// led to the end, and within which ceilings, bounds what the Orchestrator does without the person, so it is a
  /// person's press only: synthetic input (an MCP client's) is refused to it outside a QA copy (`mcp::qa_person`), and
  /// the subscription stays as it was. Nothing starts while a change is under way.
  fn change_by_person(&self, (root, chat): (PathBuf, ChatId), target: WatchTarget, change: LeadChange) -> bool {
    if self
      .qa_person
      .refuses("change whether the Orchestrator leads what it watches to the end, or its ceilings")
    {
      return false;
    }
    if self.lead.state().get_untracked().is_pending() {
      return false;
    }
    self.lead.run(Leading {
      root,
      chat,
      target,
      change,
    });
    true
  }

  /// *What I watch* in the slash menu of the project at `root`: it keeps the command in the conversation shown, whose
  /// list goes to its end and follows it; faded, with why, while the conversation leads nothing.
  pub(super) fn command(&self, ctx: &Ctx, root: PathBuf, follow: &Follow) -> SlashCommand {
    let label = ctx.t("composer.slash.command.watching.label");
    let Some(chat) = self.watches() else {
      return SlashCommand {
        name: COMMAND,
        icon: Icon::ListTree,
        label,
        detail: ctx.t("composer.slash.command.watch.nothing"),
        does: None,
      };
    };
    let (ask, follow) = (self.ask.clone(), follow.clone());
    SlashCommand {
      name: COMMAND,
      icon: Icon::ListTree,
      label,
      detail: ctx.t("composer.slash.command.watching.detail"),
      does: Some(Does::Run(Arc::new(move || {
        if ask.state().get_untracked().is_pending() {
          return;
        }
        follow.follow_again();
        ask.run(Asked {
          root: root.clone(),
          chat,
        });
      }))),
    }
  }

  /// What the rows show of the conversation shown, in the project at `root`; *Stop watching* and a change of how a
  /// target is taken are a person's press only ([`Self::stop_by_person`], [`Self::change_by_person`]).
  pub(super) fn live(&self, root: PathBuf) -> Live {
    let Held(held) = self.held.get_untracked();
    let chat = held.as_ref().map(|(chat, _)| *chat);
    let (stopper, changer, at) = (self.clone(), self.clone(), root.clone());
    Live {
      read: held.map(|(_, read)| read),
      stopping: self.stop.state().get().is_pending(),
      changing: self.lead.state().get().is_pending(),
      stop: Arc::new(move |target| {
        let Some(chat) = chat else {
          return;
        };
        let what = StopWhat::Targets(vec![NamedTarget { target, name: None }]);
        stopper.stop_by_person((root.clone(), chat), what, StopBy::Chip);
      }),
      change: Arc::new(move |target, change| {
        chat.is_some_and(|chat| changer.change_by_person((at.clone(), chat), target, change))
      }),
    }
  }

  /// A `/watching`, a *Stop watching* or a change of how a target is taken that applied nothing: why, until the
  /// person dismisses it or applies one again.
  pub(super) fn problem(&self, ctx: &mut Ctx) -> Option<Element> {
    let (asked, stopped, changed) = (self.ask.state(), self.stop.state(), self.lead.state());
    let error = asked
      .get()
      .error
      .or_else(|| stopped.get().error)
      .or_else(|| changed.get().error)?;
    let body = message_text(ctx, &error);
    let dismiss = ActionButton::new(
      "orchestrator-watching-dismiss",
      ButtonKind::Secondary,
      ctx.t("orchestrator.watching.dismiss"),
    )
    .on_click(move || {
      asked.set(FutureState::idle());
      stopped.set(FutureState::idle());
      changed.set(FutureState::idle());
    });
    Some(
      Notice::new(
        "orchestrator-watching-problem",
        NoticeTone::Danger,
        ctx.t("orchestrator.watching.failed"),
      )
      .icon(Icon::CircleAlert)
      .body(body)
      .action(dismiss)
      .into(),
    )
  }
}
