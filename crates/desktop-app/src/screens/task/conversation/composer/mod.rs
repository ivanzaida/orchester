//! The composer under the chat (`Composer` `QQ2ZM` in `D1PYAe`; its answer and plan-changes modes in
//! `YhiGC`, its queued state in `B67XdS`; `docs/design/kontur-handover.md`, "The composer"): 32 in from the
//! tab's sides and 14 over its bottom, one box — `surface_input` in a `border_strong` stroke, radius 10, 10
//! by 12 inside, 8 between the message and its row ([`controls`]) — with what the message answers or
//! changes over it ([`context`]) and one quiet line under it where that changes the outcome ([`words`]),
//! 6 apart. The run's state decides what sending does (`ComposerMode`); once no agent reads messages and
//! none can again, the box keeps only a lock and the reason.
//!
//! Enter sends the state's way (now, while the agent at work takes a message mid-turn); Ctrl+Enter (Cmd+Enter
//! on macOS) sends the second way where the state has one (*Send after this turn*, *Send without resuming*)
//! and the state's way elsewhere; Shift+Enter adds a line, and an Enter that confirms an input method's
//! composition only confirms it. Esc closes the send menu, then leaves a question's or a plan's mode.
//! The draft is the tab's ([`ComposerLink`]): it stays until the message is stored, so a failed send keeps
//! it, with *Try again*; it is stored once per draft (its idempotency key changes only after a store), so
//! a retry never sends twice. A message its agent's CLI did not take offers *Send after this turn* and
//! *Edit in composer* under it ([`undelivered`]).

mod context;
mod controls;
mod feedback;
mod presses;
mod send;
mod undelivered;
mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  conversation::Recipient,
  id::{IdempotencyKey, MessageId},
  project::TaskKey,
  run::PlanVersion,
};
use orchester_workspace::{
  messages::Message,
  tasks::{
    conversation::{Conversation, composer::ComposerMode, follow::Sent},
    detail::TaskDetail,
  },
};
use parking_lot::Mutex;

pub(super) use self::undelivered::undelivered;
use self::{
  controls::{Bar, Sending},
  send::{Outgoing, PlanAction, SendAction, plan_action, send_action},
};
use super::{TaskContext, timeline::Follow};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The most lines the message grows to before its lines scroll.
const MAX_LINES: usize = 6;

/// A press of the send button, of a send menu's way, of a key or of a link.
pub(super) type Press = Arc<dyn Fn() + Send + Sync>;

/// The composer's draft and the requests to focus it, which the tab keeps while it shows the task, so
/// the chat's messages can put a message back into it (*Edit in composer*).
#[derive(Clone)]
pub(super) struct ComposerLink {
  draft: Signal<String>,
  focus: Signal<bool>,
}

impl ComposerLink {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      draft: ctx.signal(String::new()),
      focus: ctx.signal(false),
    }
  }

  /// Puts `text` into the composer, with the focus in it.
  fn edit(&self, text: String) {
    self.draft.set(text);
    self.focus.set(true);
  }
}

/// What the composer and the chat's messages act on: the task `key` of `project` as the tab shows it, and
/// the chat, which follows a message the person sent.
#[derive(Clone)]
pub(super) struct Composing {
  link: ComposerLink,
  follow: Follow,
  project: ShownProject,
  key: TaskKey,
  detail: Arc<TaskDetail>,
}

impl Composing {
  /// Goes to `view` of the task, in the active tab.
  fn open(&self, ctx: &mut Ctx, view: TaskView) -> impl Fn() + Send + Sync + 'static {
    let navigation = Navigation::of(ctx);
    let route = Route::Project(
      self.project.open.project.id(),
      ProjectPage::Task(self.key.clone(), view),
    );
    move || navigation.go(route.clone())
  }
}

/// The composer for `task` as the page read it and `conversation` as the tab reads it, under the chat that
/// `follow` keeps at its end; it also lets the chat's messages reach the composer, so the tab calls it
/// before the chat.
pub(super) fn dock(
  ctx: &mut Ctx,
  (link, follow): (&ComposerLink, &Follow),
  task: &TaskContext,
  conversation: &Conversation,
) -> Element {
  let composing = Composing {
    link: link.clone(),
    follow: follow.clone(),
    project: task.project.clone(),
    key: task.key.clone(),
    detail: task.detail.clone(),
  };
  ctx.provide(composing.clone());
  ctx.mount_keyed::<Composer>(
    &format!("composer-{}", task.key),
    DockProps {
      composing,
      conversation: Arc::new(conversation.clone()),
    },
  )
}

#[derive(Clone)]
struct DockProps {
  composing: Composing,
  conversation: Arc<Conversation>,
}

/// Compared by what it shows; the draft by its signal.
impl PartialEq for DockProps {
  fn eq(&self, other: &Self) -> bool {
    self.composing.project == other.composing.project
      && self.composing.key == other.composing.key
      && self.composing.detail == other.composing.detail
      && self.composing.link.draft.id() == other.composing.link.draft.id()
      && self.conversation == other.conversation
  }
}

impl lurq::app::component::DevtoolsInspectable for DockProps {}

/// The idempotency key the draft is stored under; a new one once it is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DraftKey(IdempotencyKey);

impl lurq::app::component::DevtoolsInspectable for DraftKey {}

/// What a mode answers or changes, which its line names and the person can leave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Context {
  Question(MessageId),
  Plan(PlanVersion),
}

impl Context {
  fn of(mode: &ComposerMode) -> Option<Self> {
    match mode {
      ComposerMode::Answer { question, .. } => Some(Self::Question(*question)),
      ComposerMode::PlanChanges { version, .. } => Some(Self::Plan(*version)),
      _ => None,
    }
  }
}

/// The context the person left with its line's × or Esc: while it is the one the run's state offers, the
/// composer writes an ordinary message instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Left(Option<Context>);

impl lurq::app::component::DevtoolsInspectable for Left {}

struct Composer {
  key: Signal<DraftKey>,
  left: Signal<Left>,
  menu: Signal<bool>,
  focused: Signal<bool>,
  input: ElementRef,
  anchor: ElementRef,
  /// The message last sent, which *Try again* sends again with the draft as it is then.
  last: Arc<Mutex<Option<Outgoing>>>,
  /// The question the field last took the focus for: once per question.
  answering: Mutex<Option<MessageId>>,
}

impl Component for Composer {
  type Props = DockProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<DockProps>().clone();
    let (sending, plan) = (send_action(ctx), plan_action(ctx));
    let key = ctx.signal(DraftKey(IdempotencyKey::generate()));
    let last: Arc<Mutex<Option<Outgoing>>> = Arc::default();
    let summary = props.composing.open(ctx, TaskView::Summary);
    let (draft, feed, renewed, sent) = (
      props.composing.link.draft.clone(),
      props.composing.project.feed.clone(),
      key.clone(),
      last.clone(),
    );
    let follow = props.composing.follow.clone();
    ctx.watch(&sending.state(), move |state| {
      let Some(stored) = state.data.as_ref().filter(|_| !state.is_pending()) else {
        return;
      };
      // What was typed while it was sent stays.
      if sent
        .lock()
        .as_ref()
        .is_some_and(|last| last.text == draft.get_untracked())
      {
        draft.set(String::new());
      }
      renewed.set(DraftKey(IdempotencyKey::generate()));
      feed.saved();
      follow.follow_again();
      // The message waits for the raised limit, which the person sets on the Summary.
      if *stored == Sent::AwaitsLimit {
        summary();
      }
    });
    let (draft, feed) = (props.composing.link.draft.clone(), props.composing.project.feed.clone());
    ctx.watch(&plan.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        draft.set(String::new());
        feed.saved();
      }
    });
    Self {
      key,
      left: ctx.signal(Left::default()),
      menu: ctx.signal(false),
      focused: ctx.signal(false),
      input: ElementRef::new(),
      anchor: ElementRef::new(),
      last,
      answering: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DockProps>().clone();
    // The actions first, in the order `create` made them: lurq keeps them by the order they are asked for.
    let (sending, plan) = (send_action(ctx), plan_action(ctx));
    let link = &props.composing.link;
    if link.focus.get() {
      ctx.focus(&self.input);
      link.focus.set(false);
    }
    let detail = &props.composing.detail;
    let content = if detail.latest_unreadable.is_some() {
      vec![closed(&ctx.t("composer.closed.unreadable"))]
    } else {
      self.content(ctx, &props, (&sending, &plan))
    };
    Column::new()
      .id("composer")
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(ExtraSpacing::Inset.role())
      .spacing(ExtraSpacing::Snug.role())
      .with_children(content)
  }
}

impl Composer {
  /// The mode the run's state offers for its default recipient, without the context the person left.
  fn mode(&self, props: &DockProps) -> (ComposerMode, Recipient, Option<Press>) {
    let conversation = &props.conversation;
    let run = props.composing.detail.run.as_ref();
    let recipient = ComposerMode::of(conversation, run, Recipient::Orchestrator).default_recipient(conversation);
    let offered = ComposerMode::of(conversation, run, recipient);
    let context = Context::of(&offered);
    // The question was answered or ended, the plan decided: what was left is gone with it.
    let left = self.left.get();
    if left.0.is_some() && left.0 != context {
      self.left.set(Left::default());
    }
    if context.is_some() && left.0 == context {
      return (offered.without_context(conversation, recipient), recipient, None);
    }
    let leave = context.map(|context| {
      let left = self.left.clone();
      Arc::new(move || left.set(Left(Some(context)))) as Press
    });
    (offered, recipient, leave)
  }

  fn content(&self, ctx: &mut Ctx, props: &DockProps, (sending, plan): (&SendAction, &PlanAction)) -> Vec<Element> {
    let (mode, recipient, leave) = self.mode(props);
    if mode == ComposerMode::Closed {
      let reason = Message::new(props.conversation.state.closed_reason());
      return vec![closed(&message_text(ctx, &reason))];
    }
    self.focus_question(ctx, &mode);
    let busy = sending.state().get().is_pending() || plan.state().get().is_pending();
    let presses = if busy {
      Sending::Busy
    } else {
      self.presses(props, (&mode, recipient), (sending, plan))
    };
    let field = self.field(ctx, props, &mode, (&presses, leave.clone()));
    let bar = controls::bar(
      ctx,
      &Bar {
        mode: &mode,
        conversation: &props.conversation,
        recipient,
        sending: presses.clone(),
        menu: &self.menu,
        anchor: &self.anchor,
      },
    );
    let stroke = if self.focused.get() {
      PaletteColor::BorderFocus
    } else {
      PaletteColor::BorderStrong
    };
    let composer = Column::new()
      .id("composer-box")
      .describe("mode", mode_name(&mode))
      .width(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Loose.role())
      .padding_horizontal(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Lg)
      .child(field)
      .child(bar);
    let feedback = feedback::line(ctx, (sending, plan), &presses);
    let note = words::note(ctx, &mode, &props.conversation).map(|note| quiet(&note));
    context::line(ctx, &props.composing, (&mode, recipient), leave)
      .into_iter()
      .chain(std::iter::once(composer.into()))
      .chain(feedback)
      .chain(note)
      .collect()
  }

  /// The field takes the focus once for each question that waits.
  fn focus_question(&self, ctx: &Ctx, mode: &ComposerMode) {
    let ComposerMode::Answer { question, .. } = mode else {
      return;
    };
    let mut answering = self.answering.lock();
    if *answering != Some(*question) {
      *answering = Some(*question);
      ctx.focus(&self.input);
    }
  }

  /// The message field: it grows with its lines up to [`MAX_LINES`].
  fn field(
    &self,
    ctx: &Ctx,
    props: &DockProps,
    mode: &ComposerMode,
    (presses, leave): (&Sending, Option<Press>),
  ) -> Element {
    let theme = ctx.theme();
    let resolve = |color| {
      FormTextRole {
        typography: TypographyStyle::Body,
        color,
      }
      .resolve(&theme.typography(), &theme.palette())
    };
    let (gained, lost) = (self.focused.clone(), self.focused.clone());
    let placeholder = words::placeholder(ctx, mode, &props.conversation);
    let keys = presses::keys(presses, leave, self.menu.clone());
    TextInput::styled(props.composing.link.draft.clone(), resolve(PaletteColor::TextPrimary))
      .placeholder_style(resolve(PaletteColor::TextMuted))
      .caret_color(PaletteColor::TextPrimary)
      .id("composer-input")
      .describe("a11y_name", placeholder.clone())
      .textarea()
      .rows(1, MAX_LINES)
      .width(Dimension::Pct(100.0))
      .placeholder(&placeholder)
      .ref_element(self.input.clone())
      .on_focus(move || gained.set(true))
      .on_blur(move || lost.set(false))
      .on_key_down(keys)
      .into()
  }
}

/// The quiet line under the box (`SpaXX`): 11.5 `text_muted`.
fn quiet(note: &str) -> Element {
  Text::new(note)
    .id("composer-note")
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
    .into()
}

/// Once no agent reads messages: the box keeps a lock and the reason, and nothing to write in.
fn closed(reason: &str) -> Element {
  Row::new()
    .id("composer-closed")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Loose.role())
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(reason)
        .id("composer-closed-reason")
        .variant(TypographyStyle::Description)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The name `describe` reports the mode by, for tests and QA.
const fn mode_name(mode: &ComposerMode) -> &'static str {
  match mode {
    ComposerMode::Steer(_) => "steer",
    ComposerMode::Queue(_) => "queue",
    ComposerMode::Answer { .. } => "answer",
    ComposerMode::PlanChanges { .. } => "plan-changes",
    ComposerMode::FollowUp => "follow-up",
    ComposerMode::Continue => "continue",
    ComposerMode::Resume { .. } => "resume",
    ComposerMode::RaiseLimit => "raise-limit",
    ComposerMode::NotStarted => "not-started",
    ComposerMode::RunAgain => "run-again",
    ComposerMode::Closed => "closed",
  }
}
