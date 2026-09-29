//! The task conversation panel (`kjsmd`): its heading with where the conversation stands, whom the
//! next message goes to (Orchestrator, which routes it to the agent that works next, or one agent),
//! who takes part, the messages (`J5MIT`) and the composer (`C3nu0z`) with what sending does now.
//! Once no agent reads messages (a merged or ended run, or one whose worktree was removed), the
//! recipient select is disabled with the composer (02.S2 `m3gLZ`).
//!
//! Sending stores the message once per draft (a retry after an unclear failure stores nothing new),
//! then tells the runner, which delivers it at the run's next invocation boundary. A draft stays
//! until it is stored; a message stored but not taken by the runner stays queued, and the panel says
//! so.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Select, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::ScrollState},
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  conversation::Recipient,
  id::{IdempotencyKey, RunId, TaskId},
};
use orchester_ipc::Command;
use parking_lot::Mutex;

use super::{
  composer::{Composer, composer},
  entry::entry,
};
use crate::{
  background,
  messages::Message,
  runner::Runners,
  tasks::{
    conversation::{Conversation, PanelState, send_in},
    run::control_message,
  },
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    field::{field_error, select_style},
    icon::Icon,
    message_text,
    status_label::status_label,
  },
};

/// Panel width (`kjsmd`) and the recipient select's height (`LDZMb`).
const PANEL_WIDTH: f32 = 440.0;
const RECIPIENT_HEIGHT: f32 = 36.0;

#[derive(Clone, Debug)]
pub struct ConversationProps {
  pub repo_root: PathBuf,
  pub task: TaskId,
  /// The task's latest run, which a new message is written for.
  pub run: Option<RunId>,
  pub conversation: Arc<Conversation>,
  /// Set to put the focus in the composer (Ask agent).
  pub focus: Signal<bool>,
  /// Bumped once a message is stored, so the task page reads the conversation again.
  pub reload: Signal<u64>,
}

impl PartialEq for ConversationProps {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root
      && self.task == other.task
      && self.run == other.run
      && self.conversation == other.conversation
  }
}

impl lurq::app::component::DevtoolsInspectable for ConversationProps {}

/// Whom the next message goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct To(Recipient);

impl lurq::app::component::DevtoolsInspectable for To {}

/// The key the draft is stored under; a new one once it is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DraftKey(IdempotencyKey);

impl lurq::app::component::DevtoolsInspectable for DraftKey {}

/// A message to store and deliver.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Outgoing {
  repo_root: PathBuf,
  key: IdempotencyKey,
  task: TaskId,
  run: Option<RunId>,
  recovery_after: Option<orchester_core::run::EventSequence>,
  recipient: Recipient,
  text: String,
}

impl lurq::app::component::DevtoolsInspectable for Outgoing {}

/// A stored message: the runner took it, or it stays queued because the runner could not be told.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Sent {
  Taken,
  Queued(Message),
}

impl lurq::app::component::DevtoolsInspectable for Sent {}

type SendAction = FutureAction<Outgoing, Sent, Message>;

fn send_action(ctx: &mut Ctx) -> SendAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |outgoing: Outgoing| {
    let runners = runners.clone();
    async move {
      let Outgoing {
        repo_root,
        key,
        task,
        run,
        recovery_after,
        recipient,
        text,
      } = outgoing;
      background::run(move || send_in(&repo_root, key, (task, run, recovery_after), recipient, &text))
        .await?
        .map_err(|error| {
          tracing::warn!(?error, "the message was not stored");
          error.message()
        })?;
      // Before the task has a run, its first step takes the message.
      let Some(run) = run else {
        return Ok(Sent::Taken);
      };
      Ok(match runners.control_run(Command::DeliverMessages { run }).await {
        Ok(()) => Sent::Taken,
        Err(error) => {
          tracing::warn!(%error, "the runner did not take the message");
          Sent::Queued(control_message(&error))
        }
      })
    }
  })
}

pub struct ConversationPanel {
  draft: Signal<String>,
  to: Signal<To>,
  key: Signal<DraftKey>,
  focused: Signal<bool>,
  input: ElementRef,
  scroll: ScrollState,
  /// Entries shown at the last render: a new one scrolls the list to the bottom.
  shown: Arc<Mutex<usize>>,
}

impl Component for ConversationPanel {
  type Props = ConversationProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ConversationProps>().clone();
    let action = send_action(ctx);
    let draft = ctx.signal(String::new());
    let key = ctx.signal(DraftKey(IdempotencyKey::generate()));
    let (cleared, renewed, reload) = (draft.clone(), key.clone(), props.reload);
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        cleared.set(String::new());
        renewed.set(DraftKey(IdempotencyKey::generate()));
        reload.update(|revision| *revision += 1);
      }
    });
    Self {
      draft,
      to: ctx.signal(To(Recipient::Orchestrator)),
      key,
      focused: ctx.signal(false),
      input: ElementRef::new(),
      scroll: ScrollState::new(),
      shown: Arc::new(Mutex::new(0)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ConversationProps>().clone();
    let action = send_action(ctx);
    if props.focus.get() {
      ctx.focus(&self.input);
      props.focus.set(false);
    }
    let conversation = &props.conversation;
    if !conversation
      .recipients
      .iter()
      .any(|choice| choice.recipient == self.to.get().0)
    {
      self.to.set(To(Recipient::Orchestrator));
    }
    self.follow(conversation.entries.len());
    let state = action.state().get();
    let sending = state.is_pending();
    let send = self.sender(&props, &action, sending);
    let feedback = match (&state.error, &state.data) {
      _ if sending => None,
      (Some(error), _) => Some(error.clone()),
      (None, Some(Sent::Queued(reason))) => Some(reason.clone()),
      _ => None,
    };
    let composer = composer(
      ctx,
      Composer {
        draft: &self.draft,
        focused: &self.focused,
        input: &self.input,
        send,
        sending,
        open: conversation.state.takes_messages(),
        continues_failed: conversation.state == PanelState::Failed,
      },
    );
    Column::new()
      .id("task-conversation-panel")
      .width(Dimension::Px(PANEL_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Step.role())
      .padding_left(ExtraSpacing::Step.role())
      .padding_right(SpacingSize::Sm)
      .spacing(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfacePanel)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(beside_gutter(heading(ctx, conversation.state)))
      .child(beside_gutter(self.recipient(ctx, conversation)))
      .child(beside_gutter(
        Text::new(&participants(ctx, conversation))
          .id("conversation-participants")
          .variant(ExtraTypography::Footnote.role())
          .into(),
      ))
      .child(self.messages(ctx, &props))
      .child(beside_gutter(composer))
      .with_children(feedback.map(|message| beside_gutter(field_error(&message_text(ctx, &message)))))
      .child(beside_gutter(
        Text::new(&explanation(ctx, conversation))
          .id("conversation-explanation")
          .variant(ExtraTypography::Hint.role())
          .width(Dimension::Pct(100.0))
          .into(),
      ))
  }
}

impl ConversationPanel {
  /// Sends the draft, when there is one and nothing is on its way.
  fn sender(
    &self,
    props: &ConversationProps,
    action: &SendAction,
    sending: bool,
  ) -> Option<Arc<dyn Fn() + Send + Sync>> {
    let text = self.draft.get();
    if sending || text.trim().is_empty() || !props.conversation.state.takes_messages() {
      return None;
    }
    let (action, outgoing) = (
      action.clone(),
      Outgoing {
        repo_root: props.repo_root.clone(),
        key: self.key.get().0,
        task: props.task,
        run: props.run,
        recovery_after: props.conversation.recovery_after,
        recipient: self.to.get().0,
        text,
      },
    );
    Some(Arc::new(move || action.run(outgoing.clone())))
  }

  /// "To: Orchestrator · this task" and the run's agents (`LDZMb`); disabled, faded as a disabled
  /// button is, once no agent reads messages.
  fn recipient(&self, ctx: &Ctx, conversation: &Conversation) -> Element {
    let options = conversation.recipients.iter().map(|choice| {
      let label = match &choice.agent {
        None => ctx.t("conversation.to.orchestrator"),
        Some(agent) => ctx.t_args("conversation.to.agent", [("agent", &**agent)]),
      };
      (To(choice.recipient), label)
    });
    let open = conversation.state.takes_messages();
    let select = Element::from(
      Select::new(self.to.clone())
        .options(options)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(RECIPIENT_HEIGHT))
        .style(select_style(ctx, RECIPIENT_HEIGHT))
        .focusable(open),
    )
    .id("conversation-recipient");
    if open {
      return select;
    }
    // The select takes no pointer; the field around it says it cannot be used.
    Column::new()
      .width(Dimension::Pct(100.0))
      .opacity(DISABLED_OPACITY)
      .cursor(CursorIcon::NotAllowed)
      .describe("state", "disabled")
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .pointer_events_none()
          .child(select),
      )
      .into()
  }

  /// The messages, oldest first, in a viewport whose scroll gutter takes the panel's inner right
  /// padding, so they end where the composer does.
  fn messages(&self, ctx: &Ctx, props: &ConversationProps) -> Element {
    let entries = &props.conversation.entries;
    let list: Element = if entries.is_empty() {
      let key = if props.conversation.state == PanelState::WorktreeRemoved {
        "conversation.emptyWorktreeRemoved"
      } else {
        "conversation.empty"
      };
      Text::new(&ctx.t(key))
        .id("conversation-empty")
        .variant(ExtraTypography::Detail.role())
        .width(Dimension::Pct(100.0))
        .into()
    } else {
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .with_children(entries.iter().map(|item| entry(ctx, item, &props.focus)))
        .into()
    };
    ScrollVertical::new(Column::new().width(Dimension::Pct(100.0)).child(list))
      .id("conversation-messages")
      .with_scroll_state(self.scroll.clone())
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .scrollbar(gutter_scrollbar())
      .into()
  }

  /// Keeps the newest message in view.
  fn follow(&self, entries: usize) {
    let mut shown = self.shown.lock();
    if entries > *shown {
      self.scroll.scroll_to_bottom_pending();
    }
    *shown = entries;
  }
}

/// A part of the panel outside the message list, ending where the list's gutter begins.
fn beside_gutter(child: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_right(ExtraSpacing::Stack.role())
    .child(child)
    .into()
}

/// "Task conversation" and where it stands (`A5WQs`).
fn heading(ctx: &Ctx, state: PanelState) -> Element {
  let (glyph, key) = match state {
    PanelState::NotStarted => (Icon::Clock3, "conversation.status.notStarted"),
    PanelState::Live => (Icon::Radio, "conversation.status.live"),
    PanelState::Waiting => (Icon::MessageCircle, "conversation.status.waiting"),
    PanelState::Paused => (Icon::Pause, "conversation.status.paused"),
    PanelState::Stopped => (Icon::OctagonPause, "conversation.status.stopped"),
    PanelState::Interrupted => (Icon::CirclePause, "conversation.status.interrupted"),
    // The run is still ready for review; only its agent cannot be asked.
    PanelState::Ready | PanelState::WorktreeRemoved => (Icon::GitPullRequest, "conversation.status.ready"),
    PanelState::Failed | PanelState::FailedWorktreeRemoved | PanelState::FailedNoAgent => {
      (Icon::CircleX, "conversation.status.failed")
    }
    PanelState::Merged => (Icon::GitMerge, "conversation.status.merged"),
    PanelState::Ended => (Icon::Square, "conversation.status.ended"),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("conversation.title")).variant(ExtraTypography::SectionHeading.role()))
    .child(Spacer::new().flex(1.0))
    .child(status_label(glyph, &ctx.t(key)).id("conversation-status"))
    .into()
}

/// "Builder working · Planner, Reviewer available".
fn participants(ctx: &Ctx, conversation: &Conversation) -> Arc<str> {
  if let Some(agent) = conversation.suspended_agent.as_deref() {
    let key = match conversation.state {
      PanelState::Paused => Some("conversation.participants.paused"),
      PanelState::Stopped => Some("conversation.participants.stopped"),
      PanelState::Interrupted => Some("conversation.participants.interrupted"),
      _ => None,
    };
    if let Some(key) = key {
      return ctx.t_args(key, [("agent", agent)]);
    }
  }
  let participants = &conversation.participants;
  let separator = ctx.t("common.listSeparator");
  let join = |agents: &[Arc<str>]| agents.iter().map(|agent| &**agent).collect::<Vec<_>>().join(&separator);
  if conversation.state == PanelState::WorktreeRemoved && !participants.available.is_empty() {
    return ctx.t_args(
      "conversation.participants.cannotContinue",
      [("agents", join(&participants.available).as_str())],
    );
  }
  let (busy, key_with, key_alone) = if participants.asking.is_empty() {
    (
      &participants.working,
      "conversation.participants.working",
      "conversation.participants.workingOnly",
    )
  } else {
    (
      &participants.asking,
      "conversation.participants.asking",
      "conversation.participants.askingOnly",
    )
  };
  match (busy.is_empty(), participants.available.is_empty()) {
    (true, true) => ctx.t("conversation.participants.none"),
    (true, false) => ctx.t_args(
      "conversation.participants.available",
      [("available", join(&participants.available).as_str())],
    ),
    (false, true) => ctx.t_args(key_alone, [("agents", join(busy).as_str())]),
    (false, false) => ctx.t_args(
      key_with,
      [
        ("agents", join(busy).as_str()),
        ("available", join(&participants.available).as_str()),
      ],
    ),
  }
}

/// What sending does now (`pDLcP`).
fn explanation(ctx: &Ctx, conversation: &Conversation) -> Arc<str> {
  let agent = conversation.next_agent.as_deref();
  match (conversation.state, agent) {
    (PanelState::NotStarted, _) => ctx.t("conversation.explanation.notStarted"),
    (PanelState::Waiting, Some(agent)) => ctx.t_args("conversation.explanation.answer", [("agent", agent)]),
    (PanelState::Live | PanelState::Waiting, _) => ctx.t("conversation.explanation.checkpoint"),
    (PanelState::Paused, _) => ctx.t("conversation.explanation.paused"),
    (PanelState::Stopped, _) => ctx.t("conversation.explanation.stopped"),
    (PanelState::Interrupted, _) => ctx.t("conversation.explanation.queued"),
    (PanelState::Ready, Some(agent)) => ctx.t_args("conversation.explanation.ready", [("agent", agent)]),
    (PanelState::Ready, None) => ctx.t("conversation.explanation.readyNoAgent"),
    (PanelState::Failed, _) => ctx.t("conversation.explanation.failed"),
    (PanelState::FailedWorktreeRemoved, _) => ctx.t("conversation.explanation.worktreeRemoved"),
    (PanelState::FailedNoAgent, _) => ctx.t("conversation.explanation.failedNoAgent"),
    (PanelState::WorktreeRemoved, _) => ctx.t("conversation.explanation.worktreeRemoved"),
    (PanelState::Merged, _) => ctx.t("conversation.explanation.merged"),
    (PanelState::Ended, _) => ctx.t("conversation.explanation.ended"),
  }
}
