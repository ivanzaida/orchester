//! *Edit* and *Rewind to here* on an earlier message of the Orchestrator chat (ORC-333; ORC-330's `GYAPg`, `Pe6Di`,
//! `QJb0b`, `z7sgJ`, `Zm7kq`, `docs/design/conversation-rewind.md`). The person's message's toolbar offers *Edit*,
//! Copy and `…`, an answer's Copy and `…`, whose menu holds *Rewind to here* and *Fork from here* ([`toolbar`]).
//! *Edit* turns the bubble into a field in place ([`editing`]), the messages sending would drop dimmed under a line
//! that counts them; *Rewind to here* and an edit that drops messages ask first in a modal ([`dialog`]). The
//! conversation then shows where it went back to and what the Orchestrator did after it, which stays ([`line`]); a
//! rewind before the person's message puts its words back in the composer. None of it while the Orchestrator answers,
//! compacts its session or a message waits: the person stops the answer first. A rewind that failed changes nothing
//! and the dialog, or the edit field when the edit went without it, says why; an edit the conversation went back for
//! but that was not sent is back in the composer, and the chat says so (`orchester_workspace::orchestrator::rewind`).
//! Only a person goes back: synthetic input (an MCP client's) is refused, and the dialog or the edit field says so.
//! *Fork from here* (ORC-334, [`fork`]) drops nothing: it opens a new conversation with the messages up to the point,
//! at once and while the Orchestrator answers too, and the fork shows where it came from ([`forked`]).

mod dialog;
pub(super) mod editing;
mod fork;
pub(super) mod forked;
pub(super) mod line;
pub(super) mod toolbar;

use std::sync::Arc;

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
  id::{ChatId, ChatMessageId},
  orchestrator::{ChatEntry, Dropped, RewindPoint, TurnMode},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    chat::ChatView,
    queue::TakenBack,
    rewind::rewind_in,
    send::{Outgoing, SendError},
  },
  workspace::OpenProject,
};
use parking_lot::Mutex;

pub(super) use self::dialog::dialog;
use super::View;
use crate::{
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// The person's message being edited in place: its id, its entry and when it was kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Editing {
  pub(super) message: ChatMessageId,
  pub(super) seq: i64,
  pub(super) at: Timestamp,
}

impl DevtoolsInspectable for Editing {}

/// What the dialog asks about: going back in `chat` to `point`, from the message or answer of `at`, which drops
/// `dropped`; for an edit, the words sent once it went back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Asked {
  chat: ChatId,
  point: RewindPoint,
  at: Timestamp,
  dropped: Dropped,
  edit: Option<String>,
}

impl DevtoolsInspectable for Asked {}

/// A rewind of `chat` of `project` to `point`, then `edit` sent when it is one.
#[derive(Clone)]
pub(super) struct Rewind {
  project: OpenProject,
  chat: ChatId,
  point: RewindPoint,
  edit: Option<Outgoing>,
}

impl DevtoolsInspectable for Rewind {}

/// A rewind went back: the words that go back to the composer, when it went back to before the person's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Went {
  words: Option<String>,
}

impl DevtoolsInspectable for Went {}

/// Why a rewind or an edit did not go through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Failed {
  /// Nothing changed, for this reason.
  Unchanged(Message),
  /// The conversation went back, but the edit was not sent, for `why`; its words go back to the composer.
  NotSent { why: Message, words: String },
}

impl DevtoolsInspectable for Failed {}

pub(super) type RewindAction = FutureAction<Rewind, Went, Failed>;

/// The conversation shown, as the screen last read it: its title, which a fork is named after, and its entries, from
/// which a question's counts are taken when it is asked.
struct Holding {
  chat: ChatId,
  title: Arc<str>,
  entries: Arc<[ChatEntry]>,
}

type Held = Arc<Mutex<Option<Holding>>>;

/// What the screen keeps for Edit, *Rewind to here* and *Fork from here*.
#[derive(Clone)]
pub(super) struct Rewinding {
  held: Held,
  pub(super) editing: Signal<Option<Editing>>,
  /// The words the field of the message being edited holds.
  pub(super) words: Signal<String>,
  pub(super) asked: Signal<Option<Asked>>,
  /// The dialog shows; *Cancel*, Esc and the scrim end the question.
  pub(super) open: Signal<bool>,
  /// Synthetic input (an MCP client's) asked to go back and was refused: the dialog or the edit field says only a
  /// person does.
  pub(super) refused: Signal<bool>,
  pub(super) action: RewindAction,
  /// Keeps a fork ([`fork`]).
  pub(super) fork: fork::ForkAction,
}

impl Rewinding {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let i18n = ctx.i18n().clone();
    let action: RewindAction = ctx.future_action(move |rewind: Rewind| {
      let (services, i18n) = (services.clone(), i18n.clone());
      async move {
        let Rewind {
          project,
          chat,
          point,
          edit,
        } = rewind;
        let root = project.repo_root.clone();
        let rewound = background::run(move || rewind_in(&root, chat, point))
          .await
          .and_then(|rewound| rewound)
          .map_err(Failed::Unchanged)?;
        services.runners.chat_changed(&project.repo_root);
        let Some(outgoing) = edit else {
          return Ok(Went { words: rewound.words });
        };
        match super::send::deliver(&services, i18n, &project, &outgoing).await {
          // A message whose turn is not confirmed is kept, and shows so in the conversation.
          Ok(()) | Err(SendError::Unconfirmed(_)) => Ok(Went { words: None }),
          Err(SendError::NotSent(why)) => Err(Failed::NotSent {
            why,
            words: outgoing.text,
          }),
        }
      }
    });
    Self {
      held: Arc::new(Mutex::new(None)),
      editing: ctx.signal(None),
      words: ctx.signal(String::new()),
      asked: ctx.signal(None),
      open: ctx.signal(false),
      refused: ctx.signal(false),
      action,
      fork: fork::action(ctx),
    }
  }

  /// Keeps the title and entries of the conversation `shown`, or that none is shown; an edit of another
  /// conversation's message ends.
  pub(super) fn hold(&self, shown: Option<&ChatView>) {
    let chat = shown.map(|shown| shown.chat.id());
    {
      let mut held = self.held.lock();
      let same = matches!((&*held, shown), (Some(holding), Some(shown))
        if holding.chat == shown.chat.id() && *holding.title == *shown.chat.title()
          && *holding.entries == *shown.entries);
      if !same {
        *held = shown.map(|shown| Holding {
          chat: shown.chat.id(),
          title: Arc::from(shown.chat.title()),
          entries: Arc::from(shown.entries.as_slice()),
        });
      }
    }
    let editing = self.editing.get_untracked();
    let gone = editing
      .is_some_and(|editing| shown.is_none_or(|shown| !shown.entries.iter().any(|entry| entry.seq == editing.seq)));
    if gone || (chat.is_none() && editing.is_some()) {
      self.editing.set(None);
    }
  }

  /// What dropping the held conversation's entries from `from` on drops, and the conversation; `None` while none is
  /// held.
  fn dropped(&self, from: i64) -> Option<(ChatId, Dropped)> {
    let held = self.held.lock();
    let holding = held.as_ref()?;
    Some((holding.chat, Dropped::of(&holding.entries, from)))
  }

  /// How many messages sending the edit of the message kept as entry `seq` drops after it.
  pub(super) fn after(&self, seq: i64) -> u32 {
    self
      .dropped(seq)
      .map_or(0, |(_, dropped)| dropped.count().saturating_sub(1))
  }

  /// Asks whether to go back to `point`, which drops the entries from `from` on, from the message or answer of `at`.
  /// Nothing while a rewind is under way.
  pub(super) fn ask(&self, point: RewindPoint, from: i64, at: Timestamp) {
    if self.pending() {
      return;
    }
    let Some((chat, dropped)) = self.dropped(from) else {
      return;
    };
    self.action.state().set(FutureState::idle());
    self.refused.set(false);
    self.asked.set(Some(Asked {
      chat,
      point,
      at,
      dropped,
      edit: None,
    }));
    self.open.set(true);
  }

  /// Starts editing the person's message `editing` in place, which says `words`; why an earlier rewind changed nothing
  /// is no longer shown.
  pub(super) fn edit(&self, editing: Editing, words: &str) {
    if self.pending() {
      return;
    }
    if matches!(self.action.state().get_untracked().error, Some(Failed::Unchanged(_))) {
      self.action.state().set(FutureState::idle());
    }
    self.refused.set(false);
    self
      .words
      .set(orchester_workspace::mentions::facts::said(words).to_owned());
    self.editing.set(Some(editing));
  }

  /// Sends the edit of the message being edited, as a new message of `project` that goes `(mode, plan_only)`: at
  /// once when nothing follows it, else once the person confirmed what it drops. Nothing while its words say nothing
  /// or a rewind is under way; synthetic input is refused, as it is in the dialog, since what an edit drops is gone.
  pub(super) fn send_edit(&self, project: &OpenProject, (mode, plan_only): (TurnMode, bool)) {
    if self.pending() {
      return;
    }
    let words = self.words.get_untracked();
    let Some(editing) = self.editing.get_untracked().filter(|_| !words.trim().is_empty()) else {
      return;
    };
    let Some((chat, dropped)) = self.dropped(editing.seq) else {
      return;
    };
    self.action.state().set(FutureState::idle());
    let point = RewindPoint::Before(editing.message);
    if dropped.count() <= 1 {
      if self.refuse() {
        return;
      }
      self.run(project, (chat, point), Some(outgoing(chat, words, (mode, plan_only))));
      return;
    }
    self.refused.set(false);
    self.asked.set(Some(Asked {
      chat,
      point,
      at: editing.at,
      dropped,
      edit: Some(words),
    }));
    self.open.set(true);
  }

  /// Goes back as `asked` says, the edit sent as `mode` says: a person's press or Enter only.
  fn confirm(&self, project: &OpenProject, asked: &Asked, mode: (TurnMode, bool)) {
    if self.pending() || self.refuse() {
      return;
    }
    let edit = asked.edit.clone().map(|words| outgoing(asked.chat, words, mode));
    self.run(project, (asked.chat, asked.point), edit);
  }

  /// Whether a rewind is under way.
  fn pending(&self) -> bool {
    self.action.state().get_untracked().is_pending()
  }

  /// Refuses synthetic input (an MCP client's): what a rewind drops cannot be brought back and no MCP tool rewinds
  /// either, so only a person goes back, and the screen says so. Whether it refused.
  fn refuse(&self) -> bool {
    let synthetic = is_synthetic_input();
    if synthetic {
      tracing::warn!("synthetic input asked to rewind a conversation; not taken");
    }
    self.refused.set(synthetic);
    synthetic
  }

  /// Goes back, unless a rewind is already under way: a second Send or Enter never replaces the first.
  fn run(&self, project: &OpenProject, (chat, point): (ChatId, RewindPoint), edit: Option<Outgoing>) {
    if self.pending() {
      return;
    }
    self.action.run(Rewind {
      project: project.clone(),
      chat,
      point,
      edit,
    });
  }
}

/// The edit `words` as a new message of `chat`, sent by the person or automation, going `(mode, plan_only)`.
fn outgoing(chat: ChatId, words: String, (mode, plan_only): (TurnMode, bool)) -> Outgoing {
  Outgoing {
    chat,
    message: ChatMessageId::generate(),
    text: words,
    attachments: Vec::new(),
    skill: None,
    sent_by: super::sender(),
    mode,
    plan_only,
  }
}

/// Once a rewind went back, the question and the edit end, the chat follows its end again, and the words it gave back
/// go to the composer; an edit that was not sent goes there too. A fork kept opens ([`fork::watch`]).
pub(super) fn watch(ctx: &mut Ctx, view: &View) {
  fork::watch(ctx, view);
  let view = view.clone();
  ctx.watch(&view.rewind.action.state(), move |state| {
    let words = match (&state.data, &state.error) {
      (Some(went), _) if state.is_fulfilled() => went.words.clone(),
      (_, Some(Failed::NotSent { words, .. })) if state.is_rejected() => Some(words.clone()),
      _ => return,
    };
    view.rewind.open.set(false);
    view.rewind.editing.set(None);
    view.follow.follow_again();
    if let Some(words) = words {
      view.take_back(&TakenBack {
        words,
        attachments: Vec::new(),
      });
    }
  });
}

/// An edit that the conversation went back for but that was not sent: why, and that its words are in the composer,
/// until the person dismisses it or goes back again; else a fork that failed ([`fork::problem`]).
pub(super) fn problem(ctx: &mut Ctx, view: &View) -> Option<Element> {
  let state = view.rewind.action.state();
  let Some(Failed::NotSent { why, .. }) = state.get().error else {
    return fork::problem(ctx, view);
  };
  let dismiss = ActionButton::new(
    "orchestrator-edit-dismiss",
    ButtonKind::Secondary,
    ctx.t("orchestrator.edit.dismiss"),
  )
  .on_click(move || state.set(FutureState::idle()));
  Some(
    Notice::new(
      "orchestrator-edit-not-sent",
      NoticeTone::Danger,
      ctx.t("orchestrator.edit.notSent"),
    )
    .icon(Icon::CircleAlert)
    .body(ctx.t_args(
      "orchestrator.edit.notSentBody",
      [("why", message_text(ctx, &why).to_string())],
    ))
    .action(dismiss)
    .into(),
  )
}
