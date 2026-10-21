//! The composer under the chat (`Composer` `QQ2ZM` in `D1PYAe`; its answer and plan-changes modes in
//! `YhiGC`, its queued state in `B67XdS`; `docs/design/kontur-handover.md`, "The composer"): 32 in from the
//! tab's sides and 14 over its bottom, one box — `surface_input` in a `border_strong` stroke, radius 10, 10
//! by 12 inside, 8 between the message and its row ([`controls`]) — with what the message answers or
//! changes over it ([`context`]) and one quiet line under it where that changes the outcome ([`words`]),
//! 6 apart. The run's state decides what sending does (`ComposerMode`, offered by `outgoing::offer`, and
//! decided again from the task read when the message is sent); once no agent reads messages and none can
//! again, the box keeps only a lock and the reason.
//!
//! Enter sends the state's way (now, while the agent at work takes a message mid-turn); Ctrl+Enter (Cmd+Enter
//! on macOS) sends the second way where the state has one (*Send after this turn*, *Send without resuming*)
//! and the state's way elsewhere; Shift+Enter adds a line, and an Enter that confirms an input method's
//! composition only confirms it. Esc closes the send menu, then leaves a question's or a plan's mode.
//! The draft and the sends are the task page's ([`ComposerState`]), so a tab switch or a new run keeps them:
//! the draft stays until its message is stored, so a failed send keeps it, with *Try again*; its words are
//! stored once under their key, so a retry never sends twice. A message its agent's CLI did not take offers
//! *Send after this turn* and *Edit in composer* under it ([`undelivered`]).

mod context;
mod controls;
mod feedback;
mod kept;
mod presses;
mod state;
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
use orchester_core::{conversation::Recipient, id::MessageId, project::TaskKey};
use orchester_workspace::{
  messages::Message,
  tasks::{
    conversation::{
      Conversation, PanelState,
      composer::ComposerMode,
      outgoing::{Context, offer},
    },
    detail::{RunView, TaskDetail},
  },
};
use parking_lot::Mutex;

pub use self::state::ComposerState;
pub(super) use self::undelivered::undelivered;
use self::{
  controls::{Bar, Sending},
  presses::Target,
};
use super::TaskContext;
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{ProjectPage, Route, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    chat::follow::Follow,
    composer::message_scrollbar,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The most lines the message grows to before its lines scroll.
const MAX_LINES: usize = 6;

/// A press of the send button, of a send menu's way, of a key or of a link.
pub(super) type Press = Arc<dyn Fn() + Send + Sync>;

/// What the composer and the chat's messages act on: the task `key` of `project` as the tab shows it, the
/// composer's state the page keeps, and the chat, which follows a message the person sent.
#[derive(Clone)]
pub(super) struct Composing {
  state: ComposerState,
  follow: Follow,
  project: ShownProject,
  key: TaskKey,
  detail: Arc<TaskDetail>,
  /// The latest run is recorded as running while no runner looks after it.
  lost: bool,
  /// Where the latest run's conversation stands.
  panel: PanelState,
  /// The composer takes words: its run can be read and its state is not closed.
  writable: bool,
}

impl Composing {
  /// What opens `view` of the task where a press asks (`navigation::link`).
  fn open(&self, ctx: &mut Ctx, view: TaskView) -> Open {
    Navigation::of(ctx).to(Route::Project(
      self.project.open.project.id(),
      ProjectPage::task(self.key.clone(), view, self.detail.not_shared()),
    ))
  }
}

/// The composer for `task` as the page read it and `conversation` as the tab reads it, under the chat that
/// `follow` keeps at its end; it also lets the chat's messages reach the composer, so the tab calls it
/// before the chat.
pub(super) fn dock(ctx: &mut Ctx, follow: &Follow, task: &TaskContext, conversation: &Conversation) -> Element {
  let state = ctx
    .use_context::<ComposerState>()
    .expect("the task page keeps the composer's state before its tabs");
  let run = task.detail.run.as_ref();
  let open = offer(conversation, run, None).0 != ComposerMode::Closed;
  let composing = Composing {
    state,
    follow: follow.clone(),
    project: task.project.clone(),
    key: task.key.clone(),
    detail: task.detail.clone(),
    lost: task.lost(),
    panel: conversation.state,
    writable: open && task.detail.latest_unreadable.is_none(),
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

/// Compared by what it shows; the composer's state by its draft.
impl PartialEq for DockProps {
  fn eq(&self, other: &Self) -> bool {
    self.composing.project == other.composing.project
      && self.composing.key == other.composing.key
      && self.composing.detail == other.composing.detail
      && self.composing.lost == other.composing.lost
      && self.composing.state.draft.id() == other.composing.state.draft.id()
      && self.conversation == other.conversation
  }
}

impl lurq::app::component::DevtoolsInspectable for DockProps {}

struct Composer {
  menu: Signal<bool>,
  focused: Signal<bool>,
  input: ElementRef,
  anchor: ElementRef,
  /// The question the field last took the focus for: once per question.
  answering: Mutex<Option<MessageId>>,
}

impl Component for Composer {
  type Props = DockProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<DockProps>().clone();
    let follow = props.composing.follow.clone();
    ctx.watch(&props.composing.state.send.state(), move |result| {
      // A failure keeps the data of the send before it: only a send that went through moves the chat.
      if result.is_fulfilled() {
        follow.follow_again();
      }
    });
    Self {
      menu: ctx.signal(false),
      focused: ctx.signal(false),
      input: ElementRef::new(),
      anchor: ElementRef::new(),
      answering: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DockProps>().clone();
    let state = &props.composing.state;
    if state.focus.get() {
      ctx.focus(&self.input);
      state.focus.set(false);
    }
    let content = if props.composing.detail.latest_unreadable.is_some() {
      vec![closed(&ctx.t("composer.closed.unreadable"))]
    } else {
      self.content(ctx, &props)
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
  /// The mode the run's state offers for its default recipient, without the context the person left;
  /// the context they left, and how to leave the one offered.
  fn mode(&self, props: &DockProps) -> (ComposerMode, Recipient, (Option<Context>, Option<Press>)) {
    let conversation = &props.conversation;
    let run = props.composing.detail.run.as_ref();
    let (offered, recipient) = offer(conversation, run, None);
    let context = Context::of(&offered);
    // The question was answered or ended, the plan decided: what was left is gone with it.
    let left = props.composing.state.left.get();
    if left.is_some() && left != context {
      props.composing.state.left.set(None);
    }
    if context.is_some() && left == context {
      let (mode, recipient) = offer(conversation, run, left);
      return (mode, recipient, (left, None));
    }
    let leave = context.map(|context| {
      let left = props.composing.state.left.clone();
      Arc::new(move || left.set(Some(context))) as Press
    });
    (offered, recipient, (None, leave))
  }

  fn content(&self, ctx: &mut Ctx, props: &DockProps) -> Vec<Element> {
    let (mode, recipient, (left, leave)) = self.mode(props);
    if mode == ComposerMode::Closed {
      let reason = Message::new(props.conversation.state.closed_reason());
      return vec![closed(&message_text(ctx, &reason))];
    }
    self.focus_question(ctx, &mode);
    let composing = &props.composing;
    let target = Target {
      project: composing.project.open.clone(),
      task: composing.detail.task.id(),
      written_for: composing.detail.run.as_ref().map(RunView::id),
      revision: composing.detail.task.revision(),
      lost: composing.lost,
      left,
    };
    let presses = presses::presses(&composing.state, &target, (&mode, recipient));
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
    let feedback = feedback::line(ctx, &composing.state, (&props.conversation, mode_name(&mode)), &presses);
    let note = words::note(ctx, &mode, &props.conversation).map(|note| quiet(&note));
    context::line(ctx, composing, (&mode, recipient), leave)
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
    let keys = presses::keys(presses, (leave, self.input.clone()), self.menu.clone());
    TextInput::styled(props.composing.state.draft.clone(), resolve(PaletteColor::TextPrimary))
      .placeholder_style(resolve(PaletteColor::TextMuted))
      .caret_color(PaletteColor::TextPrimary)
      .id("composer-input")
      .describe("a11y_name", placeholder.clone())
      .textarea()
      .rows(1, MAX_LINES)
      .scrollbar(message_scrollbar(ctx))
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

/// The name `describe` reports the mode by, for tests and QA; a stored message's queued reason belongs to
/// the mode it was sent in.
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
