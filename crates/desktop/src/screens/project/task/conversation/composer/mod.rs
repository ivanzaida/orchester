//! The composer at the bottom of the chat (`docs/design/orchester-workspace.md`, "Composer
//! (approved …)", `YJfPo`): one rounded box, the message on top and a row inside it — *Attach*
//! (unavailable), the run's permissions as a chip, the recipient as a quiet selector and a round
//! send button ([`controls`]). What it answers or changes stands above the box, and one quiet line
//! under it while it changes what happens ([`text`]). Once no agent reads messages, the box keeps
//! only the lock and the reason.
//!
//! Send and Enter send now while the agent the message is for works mid-turn, and press the state's
//! action otherwise; the chevron beside Send, shown only then, offers *Send after this turn* for that
//! message, as Ctrl+Enter (Cmd+Enter on macOS) does; nothing is remembered. Shift+Enter adds a line.
//! When it first shows a question that waits, as the tab opens on it or once it arrives, the field
//! takes the focus, unless the person is in the tab's search field. The draft stays until the message
//! is stored, once per draft.

mod controls;
mod text;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::keyboard_event::KeyboardEvent,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  conversation::{Recipient, Timing},
  id::{IdempotencyKey, MessageId},
  run::{PlanDecision, PlanVersion, ReviewNote},
};
use parking_lot::Mutex;

use self::text::{context_line, hint, placeholder, quiet};
use super::{
  super::plan_review::{Decision as PlanRequest, DecisionAction, decision_action as plan_decision},
  column,
  send::{Outgoing, SendAction, Sent, send_action},
};
use crate::{
  tasks::{
    conversation::{Conversation, composer::ComposerMode},
    detail::TaskDetail,
    plan::PlanReview,
  },
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    field::{field_error, styled_input},
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// The most lines the message grows to before it scrolls.
const MAX_LINES: usize = 6;
/// The closed box's height (`W9LMuM`).
const CLOSED_HEIGHT: f32 = 52.0;

#[derive(Clone)]
pub(super) struct ComposerProps {
  pub(super) repo_root: std::path::PathBuf,
  pub(super) detail: Arc<TaskDetail>,
  pub(super) conversation: Arc<Conversation>,
  /// The message being written; *Edit in composer* puts one back into it.
  pub(super) draft: Signal<String>,
  /// Set to put the focus in the field (Ask agent, Reply).
  pub(super) focus: Signal<bool>,
  /// The tab's search field holds the focus, the only other field on the tab: a question that
  /// arrives then leaves it there.
  pub(super) searching: Signal<bool>,
  /// Bumped once a message is stored, so the task is read again.
  pub(super) reload: Signal<u64>,
  /// Called once a message was sent: the chat goes to its end.
  pub(super) sent: Arc<dyn Fn() + Send + Sync>,
  /// How wide the panel beside the chat is: the rail, its strip or Details.
  pub(super) beside: f32,
}

impl PartialEq for ComposerProps {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root
      && self.detail == other.detail
      && self.conversation == other.conversation
      && self.beside == other.beside
  }
}

impl lurq::app::component::DevtoolsInspectable for ComposerProps {}

/// Whom the next message goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct To(Recipient);

impl lurq::app::component::DevtoolsInspectable for To {}

/// The key the draft is stored under; a new one once it is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DraftKey(IdempotencyKey);

impl lurq::app::component::DevtoolsInspectable for DraftKey {}

/// A press of Send, of a send menu option or of a key.
type Press = Arc<dyn Fn() + Send + Sync>;

/// What a mode answers or changes, which its context line names and the person can leave.
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

/// The context the person left with its line's × or Esc: while it is the one the run's state
/// offers, the composer writes an ordinary message instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Left(Option<Context>);

impl lurq::app::component::DevtoolsInspectable for Left {}

/// What sending does now; `None` where it cannot be pressed.
struct Presses {
  /// Send and Enter: now while the agent works mid-turn, else the state's action.
  send: Option<Press>,
  /// *Send after this turn* and Ctrl/Cmd+Enter, while the agent works mid-turn.
  after_turn: Option<Press>,
  /// Leaves the mode's context: its line's × and Esc.
  leave: Option<Press>,
}

pub(super) struct ComposerDock {
  to: Signal<To>,
  /// The recipient last chosen for the person; while `to` is still it, it follows the run.
  defaulted: Signal<Option<To>>,
  key: Signal<DraftKey>,
  focused: Signal<bool>,
  input: ElementRef,
  /// The last question that waited, which the field took the focus for unless the person was
  /// searching: once per question.
  answering: Mutex<Option<MessageId>>,
  left: Signal<Left>,
  controls: controls::Controls,
}

impl Component for ComposerDock {
  type Props = ComposerProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ComposerProps>().clone();
    let action = send_action(ctx);
    plan_decision(ctx);
    let key = ctx.signal(DraftKey(IdempotencyKey::generate()));
    let (cleared, renewed, reload, sent) = (props.draft.clone(), key.clone(), props.reload.clone(), props.sent);
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        cleared.set(String::new());
        renewed.set(DraftKey(IdempotencyKey::generate()));
        reload.update(|revision| *revision += 1);
        sent();
      }
    });
    Self {
      to: ctx.signal(To(Recipient::Orchestrator)),
      defaulted: ctx.signal(None),
      key,
      focused: ctx.signal(false),
      input: ElementRef::new(),
      answering: Mutex::new(None),
      left: ctx.signal(Left::default()),
      controls: controls::Controls::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ComposerProps>().clone();
    let action = send_action(ctx);
    let plan = plan_decision(ctx);
    if props.focus.get() {
      // Reply, a question's hint and Ask agent ask for the box as the run's state offers it: a
      // question the person left is answered again.
      if self.left.get_untracked().0.is_some() {
        self.left.set(Left::default());
      }
      ctx.focus(&self.input);
      props.focus.set(false);
    }
    let conversation = &props.conversation;
    let run = props.detail.run.as_ref();
    self.follow_default(conversation, run);
    let recipient = self.to.get().0;
    let offered = ComposerMode::of(conversation, run, recipient);
    // The question was answered or ended, the plan decided: what was left is gone with it.
    if let Some(context) = self.left.get_untracked().0
      && Context::of(&offered) != Some(context)
    {
      self.left.set(Left::default());
    }
    let left = Context::of(&offered).is_some_and(|context| self.left.get().0 == Some(context));
    let leave = Context::of(&offered).filter(|_| !left).map(|context| {
      let chosen = self.left.clone();
      Arc::new(move || chosen.set(Left(Some(context)))) as Press
    });
    let mode = if left {
      offered.without_context(conversation, recipient)
    } else {
      offered
    };
    self.focus_question(ctx, &props, &mode);
    let feedback = feedback(&action, &plan);
    let content = if mode == ComposerMode::Closed {
      vec![closed(ctx, &hint(ctx, &mode, conversation))]
    } else {
      let sending = action.state().get().is_pending() || plan.state().get().is_pending();
      let presses = Presses {
        leave,
        ..self.presses(&props, (&mode, recipient), (&action, &plan), sending)
      };
      let composer = self.composer(ctx, &props, (&mode, recipient), &presses, sending);
      context_line(ctx, (&mode, &props.detail), presses.leave.clone())
        .into_iter()
        .chain(std::iter::once(composer))
        .chain(feedback.map(|message| field_error(&message_text(ctx, &message))))
        .chain(quiet(ctx, &mode, conversation).map(|line| quiet_line(&line)))
        .collect()
    };
    Column::new()
      .id("conversation-dock")
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::ComposerTop.role())
      .padding_bottom(ExtraSpacing::ComposerEdge.role())
      .child(column(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .with_children(content)
          .into(),
      ))
  }
}

impl ComposerDock {
  /// Points the recipient at whom the run's state addresses ([`ComposerMode::default_recipient`]),
  /// unless the person chose another.
  fn follow_default(&self, conversation: &Conversation, run: Option<&crate::tasks::detail::RunView>) {
    let mode = ComposerMode::of(conversation, run, Recipient::Orchestrator);
    let default = mode.default_recipient(conversation);
    // A question the person left no longer holds the recipient to its asker.
    let left = Context::of(&mode).is_some_and(|context| self.left.get_untracked().0 == Some(context));
    let fixed = mode.fixed_recipient().is_some() && !left;
    let current = self.to.get_untracked();
    let offered = conversation
      .recipients
      .iter()
      .any(|choice| choice.recipient == current.0);
    if fixed || !offered || self.defaulted.get_untracked().is_none_or(|last| last == current) {
      if current.0 != default {
        self.to.set(To(default));
      }
      if self.defaulted.get_untracked() != Some(To(default)) {
        self.defaulted.set(Some(To(default)));
      }
    }
  }

  /// The field takes the focus once for each question that waits, unless the person is searching.
  fn focus_question(&self, ctx: &mut Ctx, props: &ComposerProps, mode: &ComposerMode) {
    let ComposerMode::Answer { question, .. } = mode else {
      return;
    };
    let mut answering = self.answering.lock();
    if *answering != Some(*question) {
      *answering = Some(*question);
      if !props.searching.get_untracked() {
        ctx.focus(&self.input);
      }
    }
  }

  fn presses(
    &self,
    props: &ComposerProps,
    (mode, recipient): (&ComposerMode, Recipient),
    (action, plan): (&SendAction, &DecisionAction),
    sending: bool,
  ) -> Presses {
    let text = props.draft.get();
    let open = props.conversation.state.takes_messages() && *mode != ComposerMode::Closed;
    if sending || text.trim().is_empty() || !open {
      return Presses {
        send: None,
        after_turn: None,
        leave: None,
      };
    }
    if let ComposerMode::PlanChanges { .. } = mode {
      return Presses {
        send: request_changes(props, plan, &text),
        after_turn: None,
        leave: None,
      };
    }
    let send = |timing: Timing| -> Press {
      let outgoing = Outgoing {
        repo_root: props.repo_root.clone(),
        key: self.key.get().0,
        task: props.detail.task.id(),
        run: props.detail.run.as_ref().map(crate::tasks::detail::RunView::id),
        recovery_after: props.conversation.recovery_after,
        recipient,
        timing,
        text: text.clone(),
      };
      let action = action.clone();
      Arc::new(move || action.run(outgoing.clone()))
    };
    match mode {
      ComposerMode::Steer(working) if working.mid_turn => Presses {
        send: Some(send(Timing::Now)),
        after_turn: Some(send(Timing::AfterTurn)),
        leave: None,
      },
      _ => Presses {
        send: Some(send(Timing::AfterTurn)),
        after_turn: None,
        leave: None,
      },
    }
  }

  /// The box: the message, then its row of controls; `$muted` around it while it has the focus.
  fn composer(
    &self,
    ctx: &mut Ctx,
    props: &ComposerProps,
    (mode, recipient): (&ComposerMode, Recipient),
    presses: &Presses,
    sending: bool,
  ) -> Element {
    let field = self.field(ctx, props, mode, presses);
    let row = self.controls.row(
      ctx,
      &controls::RowProps {
        props,
        mode,
        to: (&self.to, recipient),
        presses,
        sending,
      },
    );
    let border = if self.focused.get() {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::Border
    };
    Column::new()
      .id("conversation-composer")
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Group.role())
      .padding_right(ExtraSpacing::Stack.role())
      .padding_bottom(ExtraSpacing::Inline.role())
      .padding_left(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, border))
      .rounded(ExtraRadius::Composer.role())
      .ref_element(self.controls.box_anchor.clone())
      .child(field)
      .child(row)
      .into()
  }

  /// The message field: it grows with its lines up to [`MAX_LINES`]. Enter sends, Shift+Enter adds
  /// a line, Ctrl/Cmd+Enter sends after this turn while the agent works mid-turn; Esc closes the
  /// send menu, then leaves the context line's mode.
  fn field(&self, ctx: &Ctx, props: &ComposerProps, mode: &ComposerMode, presses: &Presses) -> Element {
    let (gained, lost) = (self.focused.clone(), self.focused.clone());
    let placeholder = placeholder(ctx, mode, &props.conversation);
    let (send, after_turn, leave) = (presses.send.clone(), presses.after_turn.clone(), presses.leave.clone());
    let menu = self.controls.menu.clone();
    styled_input(ctx, props.draft.clone(), TypographyStyle::Body)
      .id("conversation-input")
      .textarea()
      .rows(1, MAX_LINES)
      .width(Dimension::Pct(100.0))
      .placeholder(&placeholder)
      .ref_element(self.input.clone())
      .on_focus(move || gained.set(true))
      .on_blur(move || lost.set(false))
      .on_key_down(move |event: KeyboardEvent| {
        if event.key == "Escape" {
          if menu.get_untracked() {
            menu.set(false);
            event.prevent_default();
          } else if let Some(leave) = &leave {
            leave();
            event.prevent_default();
          }
          return;
        }
        if event.key != "Enter" || event.shift {
          return;
        }
        // Enter sends; it never also adds a newline, even when there is nothing to send.
        event.prevent_default();
        let press = if event.ctrl || event.meta {
          after_turn.clone().or_else(|| send.clone())
        } else {
          send.clone()
        };
        if let Some(press) = press {
          press();
        }
      })
      .into()
  }
}

/// Once no agent reads messages (`W9LMuM` in `l7cEtA`): no controls, the lock and the reason.
fn closed(ctx: &Ctx, reason: &str) -> Element {
  Row::new()
    .id("conversation-composer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(CLOSED_HEIGHT))
    .padding_horizontal(ExtraSpacing::ComposerEdge.role())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .rounded(ExtraRadius::Composer.role())
    .child(icon(Icon::Lock, IconSize::Entry, PaletteColor::TextSecondary))
    .child(
      Row::new()
        .id("conversation-input-closed")
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(
          Text::new(&ctx.t_args("runs.conversation.composer.closedReason", [("reason", reason)]))
            .id("conversation-explanation")
            .variant(TypographyStyle::FieldLabel)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// The quiet line under the box (`okcPt`): 10 `$subtle`.
fn quiet_line(line: &str) -> Element {
  Text::new(line)
    .id("conversation-explanation")
    .variant(ExtraTypography::Hint.role())
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
    .into()
}

/// *Request changes* with the draft as the changes the plan needs.
fn request_changes(props: &ComposerProps, plan: &DecisionAction, text: &str) -> Option<Press> {
  let run = props.detail.run.as_ref()?;
  let review = PlanReview::of(run)?;
  let note = ReviewNote::new(text).ok()?;
  let waiting = review.waiting();
  let request = PlanRequest {
    run: review.run,
    execution: waiting.execution,
    version: waiting.version,
    decision: PlanDecision::ChangesRequested { note },
  };
  let plan = plan.clone();
  Some(Arc::new(move || plan.run(request.clone())))
}

/// Why the last send did not go through, or that the runner did not take it and it stays queued.
fn feedback(action: &SendAction, plan: &DecisionAction) -> Option<crate::messages::Message> {
  let state = action.state().get();
  let plan = plan.state().get();
  if state.is_pending() || plan.is_pending() {
    return None;
  }
  match (state.error, state.data) {
    (Some(error), _) => Some(error),
    (None, Some(Sent::Queued(reason))) => Some(reason),
    _ => plan.error,
  }
}
