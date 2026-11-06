//! `/watching` (ORC-775, `06 · /watching` `Ff7AD` in `Orchestrator watch · redesign`): *What I watch* in the slash
//! menu, offered while the conversation leads something. Chosen, it keeps the person's command
//! (`orchester_workspace::orchestrator::watch::watching_in`) off the UI thread, with no agent turn and nothing spent,
//! also while an answer is under way. Its row shows the command's bubble and Orchester's answer under it
//! ([`answer`]), built from what the conversation leads now (`orchester_workspace::orchestrator::watching`), read
//! again as runs and tasks change: an earlier `/watching` shows the live state too. *Stop watching* on a
//! subscription's head row ends it at once, as × on its header chip does. A command or a stop that applied nothing
//! says why over the composer.

mod answer;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, FutureState},
  },
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
use parking_lot::Mutex;

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

/// *Stop watching* on the head row of `target`, a subscription of conversation `chat` of the project at `root`.
#[derive(Clone)]
pub(super) struct Stopping {
  root: PathBuf,
  chat: ChatId,
  target: WatchTarget,
}

impl DevtoolsInspectable for Stopping {}

/// The conversation shown and what it was read to lead, as the screen's last pass read them.
type Held = Option<(ChatId, Read<Arc<Watching>>)>;

/// What the screen keeps for `/watching`: keeping the command, stopping a subscription from the answer, and what the
/// conversation shown leads, which the slash menu and the rows of the same pass read.
#[derive(Clone)]
pub(super) struct WhatIWatch {
  ask: FutureAction<Asked, (), Message>,
  stop: FutureAction<Stopping, (), Message>,
  held: Arc<Mutex<Held>>,
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
    let stop = ctx.future_action(move |Stopping { root, chat, target }: Stopping| {
      let runners = runners.clone();
      async move {
        let at = root.clone();
        background::run(move || {
          let what = StopWhat::Target(NamedTarget { target, name: None });
          stop_in(&at, &Chat::named(chat, "", Timestamp::now()), &what, StopBy::Chip)
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
      held: Arc::default(),
    }
  }

  /// Holds what `reads` found for the conversation shown, for the rest of this pass.
  pub(super) fn hold(&self, reads: &Reads) {
    let held = reads.target.zip(reads.watching.clone());
    *self.held.lock() = held;
  }

  /// The conversation shown, while it was read to lead something.
  fn watches(&self) -> Option<ChatId> {
    let held = self.held.lock();
    let (chat, read) = held.as_ref()?;
    read
      .ready()
      .filter(|watching| !watching.subscriptions.is_empty())
      .map(|_| *chat)
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

  /// What the rows show of the conversation shown, in the project at `root`.
  pub(super) fn live(&self, root: PathBuf) -> Live {
    let held = self.held.lock().clone();
    let chat = held.as_ref().map(|(chat, _)| *chat);
    let stop = self.stop.clone();
    Live {
      read: held.map(|(_, read)| read),
      stopping: stop.state().get().is_pending(),
      stop: Arc::new(move |target| {
        let Some(chat) = chat else {
          return;
        };
        if !stop.state().get_untracked().is_pending() {
          stop.run(Stopping {
            root: root.clone(),
            chat,
            target,
          });
        }
      }),
    }
  }

  /// A `/watching` or a *Stop watching* that applied nothing: why, until the person dismisses it or applies one again.
  pub(super) fn problem(&self, ctx: &mut Ctx) -> Option<Element> {
    let (asked, stopped) = (self.ask.state(), self.stop.state());
    let error = asked.get().error.or_else(|| stopped.get().error)?;
    let dismiss = ActionButton::new(
      "orchestrator-watching-dismiss",
      ButtonKind::Secondary,
      ctx.t("orchestrator.watching.dismiss"),
    )
    .on_click(move || {
      asked.set(FutureState::idle());
      stopped.set(FutureState::idle());
    });
    Some(
      Notice::new(
        "orchestrator-watching-problem",
        NoticeTone::Danger,
        ctx.t("orchestrator.watching.failed"),
      )
      .icon(Icon::CircleAlert)
      .body(message_text(ctx, &error))
      .action(dismiss)
      .into(),
    )
  }
}
