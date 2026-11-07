//! `/watching` (ORC-775, `06 · /watching` `Ff7AD` in `Orchestrator watch · redesign`): *What I watch* in the slash
//! menu, offered while the conversation leads something. Chosen, it keeps the person's command
//! (`orchester_workspace::orchestrator::watch::watching_in`) off the UI thread, with no agent turn and nothing spent,
//! also while an answer is under way. Its row shows the command's bubble and Orchester's answer under it
//! ([`answer`]), built from what the conversation leads now (`orchester_workspace::orchestrator::watching`), read
//! again as runs and tasks change: an earlier `/watching` shows the live state too. *Stop watching* on a
//! subscription's head row ends it at once, as × on its header chip does, and so does `/unwatch` ([`unwatch`],
//! ORC-774) for what is chosen in its picker. A command or a stop that applied nothing says why over the composer.

mod answer;
mod unwatch;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, FutureState},
    events::is_synthetic_input,
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::ChatId,
  orchestrator::{Chat, WatchTarget, watch::NamedTarget},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    watch::{StopBy, StopWhat, WatchError, stop_in, watching_in},
    watching::Watching,
  },
};

pub(super) use self::answer::row;
use super::data::{Read, Reads};
use crate::{
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
  /// The last stop came from automation and was not taken.
  refused: Signal<bool>,
  held: Signal<Held>,
}

/// What a `/watching` row shows of the live state: what the conversation leads (`None` for a new one), whether a
/// stop is under way, and what *Stop watching* does.
#[derive(Clone)]
pub(super) struct Live {
  pub(super) read: Option<Read<Arc<Watching>>>,
  pub(super) stopping: bool,
  pub(super) stop: Arc<dyn Fn(WatchTarget) + Send + Sync>,
}

/// Compared by what it shows; the stop is the screen's.
impl PartialEq for Live {
  fn eq(&self, other: &Self) -> bool {
    self.read == other.read && self.stopping == other.stopping
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
      refused: ctx.signal(false),
      held: ctx.signal(Held::default()),
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
  /// subscription either, so synthetic input (an MCP client's) is refused, the subscriptions go on and the notice over
  /// the composer says so. Nothing starts while a stop is under way.
  fn stop_by_person(&self, (root, chat): (PathBuf, ChatId), what: StopWhat, by: StopBy) -> bool {
    if is_synthetic_input() {
      tracing::warn!("synthetic input asked to stop watching; not taken");
      self.refused.set(true);
      return false;
    }
    if self.refused.get_untracked() {
      self.refused.set(false);
    }
    if self.stop.state().get_untracked().is_pending() {
      return false;
    }
    self.stop.run(Stopping { root, chat, what, by });
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

  /// What the rows show of the conversation shown, in the project at `root`; *Stop watching* is a person's press only
  /// ([`Self::stop_by_person`]).
  pub(super) fn live(&self, root: PathBuf) -> Live {
    let Held(held) = self.held.get_untracked();
    let chat = held.as_ref().map(|(chat, _)| *chat);
    let this = self.clone();
    Live {
      read: held.map(|(_, read)| read),
      stopping: self.stop.state().get().is_pending(),
      stop: Arc::new(move |target| {
        let Some(chat) = chat else {
          return;
        };
        let what = StopWhat::Targets(vec![NamedTarget { target, name: None }]);
        this.stop_by_person((root.clone(), chat), what, StopBy::Chip);
      }),
    }
  }

  /// A `/watching` or a *Stop watching* that applied nothing: why, until the person dismisses it or applies one again.
  /// A refused click of automation says that only a person stops watching.
  pub(super) fn problem(&self, ctx: &mut Ctx) -> Option<Element> {
    let (asked, stopped, refused) = (self.ask.state(), self.stop.state(), self.refused.clone());
    let body = if refused.get() {
      ctx.t("orchestrator.watching.personOnly")
    } else {
      let error = asked.get().error.or_else(|| stopped.get().error)?;
      message_text(ctx, &error)
    };
    let dismiss = ActionButton::new(
      "orchestrator-watching-dismiss",
      ButtonKind::Secondary,
      ctx.t("orchestrator.watching.dismiss"),
    )
    .on_click(move || {
      asked.set(FutureState::idle());
      stopped.set(FutureState::idle());
      refused.set(false);
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
