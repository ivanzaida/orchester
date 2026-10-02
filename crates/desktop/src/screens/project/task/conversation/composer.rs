//! The composer (`C3nu0z`), docked under the timeline (hairline above, padding 12 / 32 / 14, in the
//! timeline's column): the message, the recipient and *Attach* (unavailable: attachments are not
//! built) in its action row, the state's buttons, and one line under it that says what sending does
//! now (`docs/design/orchester-workspace.md`, "Composer"). While an agent works and the message is
//! for it, *Send now* and *Send after this turn*; Ctrl+Enter (Cmd+Enter on macOS) sends after this
//! turn then, the safer default, and presses the state's primary action otherwise. Enter adds a
//! line. The draft stays until the message is stored, once per draft.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::keyboard_event::KeyboardEvent,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  conversation::{Recipient, Timing},
  id::IdempotencyKey,
  run::{PlanDecision, ReviewNote},
};

use super::{
  super::plan_review::{Decision as PlanRequest, DecisionAction, decision_action as plan_decision},
  column,
  composer_text::{hint, primary_label, recipient_label},
  send::{Outgoing, SendAction, Sent, send_action},
};
use crate::{
  display,
  tasks::{
    conversation::{Conversation, composer::ComposerMode},
    detail::TaskDetail,
    plan::PlanReview,
  },
  theme::{DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, select_style, styled_input},
    icon::Icon,
    message_text,
  },
};

/// Buttons and the recipient select of the action row (`lUFjz`, `EEQJC`), the select's width, and
/// the most lines the message grows to before it scrolls.
const BUTTON_HEIGHT: f32 = 30.0;
const RECIPIENT_WIDTH: f32 = 260.0;
const MAX_LINES: usize = 6;

#[derive(Clone)]
pub(super) struct ComposerProps {
  pub(super) repo_root: std::path::PathBuf,
  pub(super) detail: Arc<TaskDetail>,
  pub(super) conversation: Arc<Conversation>,
  /// The message being written; *Edit in composer* puts one back into it.
  pub(super) draft: Signal<String>,
  /// Set to put the focus in the field (Ask agent, Reply).
  pub(super) focus: Signal<bool>,
  /// Bumped once a message is stored, so the task is read again.
  pub(super) reload: Signal<u64>,
  /// Called once a message was sent: the timeline goes to its end.
  pub(super) sent: Arc<dyn Fn() + Send + Sync>,
}

impl PartialEq for ComposerProps {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root && self.detail == other.detail && self.conversation == other.conversation
  }
}

impl lurq::app::component::DevtoolsInspectable for ComposerProps {}

/// Whom the next message goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct To(Recipient);

impl lurq::app::component::DevtoolsInspectable for To {}

/// How the message on its way was sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SentAs(Timing);

impl lurq::app::component::DevtoolsInspectable for SentAs {}

/// The key the draft is stored under; a new one once it is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DraftKey(IdempotencyKey);

impl lurq::app::component::DevtoolsInspectable for DraftKey {}

pub(super) struct ComposerDock {
  to: Signal<To>,
  /// The recipient last chosen for the person; while `to` is still it, it follows the run.
  defaulted: Signal<Option<To>>,
  key: Signal<DraftKey>,
  focused: Signal<bool>,
  /// How the message on its way was sent, for the button that shows it under way.
  timing: Signal<SentAs>,
  input: ElementRef,
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
      timing: ctx.signal(SentAs(Timing::AfterTurn)),
      input: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ComposerProps>().clone();
    let action = send_action(ctx);
    let plan = plan_decision(ctx);
    if props.focus.get() {
      ctx.focus(&self.input);
      props.focus.set(false);
    }
    let conversation = &props.conversation;
    let run = props.detail.run.as_ref();
    self.follow_default(conversation, run);
    let recipient = self.to.get().0;
    let mode = ComposerMode::of(conversation, run, recipient);
    let sending = action.state().get().is_pending() || plan.state().get().is_pending();
    let presses = self.presses(&props, (&mode, recipient), (&action, &plan), sending);
    let feedback = feedback(&action, &plan);
    let field = self.field(ctx, &props, &mode, presses.shortcut.clone());
    let row = self.action_row(ctx, conversation, (&mode, recipient), &presses, sending);
    let border = if self.focused.get() {
      Border::inside(BorderSize::Md, PaletteColor::BorderFocus)
    } else {
      Border::inside(BorderSize::Sm, PaletteColor::Border)
    };
    let composer = Column::new()
      .id("conversation-composer")
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Stack.role())
      .spacing(ExtraSpacing::Group.role())
      .background(ExtraColor::Surface.role())
      .border(border)
      .rounded(ExtraRadius::Notice.role())
      .with_children(context_line(ctx, &mode))
      .child(field)
      .child(row);
    let hint = Text::new(&hint(ctx, &mode, conversation))
      .id("conversation-explanation")
      .variant(ExtraTypography::Hint.role())
      .width(Dimension::Pct(100.0));
    Column::new()
      .id("conversation-dock")
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Stack.role())
      .padding_bottom(ExtraSpacing::Group.role())
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(column(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(composer)
          .with_children(feedback.map(|message| field_error(&message_text(ctx, &message))))
          .child(hint)
          .into(),
      ))
  }
}

/// What the composer's buttons and its shortcut do now; `None` where they cannot be pressed.
struct Presses {
  /// The state's primary action: *Send*, *Send answer*, *Request changes*, …
  primary: Option<Arc<dyn Fn() + Send + Sync>>,
  /// *Send now*, while an agent works and its connection takes a message mid-turn.
  now: Option<Arc<dyn Fn() + Send + Sync>>,
  /// Ctrl/Cmd+Enter: after this turn while an agent works, the primary action otherwise.
  shortcut: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl ComposerDock {
  /// Points the recipient at whom the run's state addresses (the agent at work, the one that
  /// asked), unless the person chose another.
  fn follow_default(&self, conversation: &Conversation, run: Option<&crate::tasks::detail::RunView>) {
    let mode = ComposerMode::of(conversation, run, Recipient::Orchestrator);
    let default = mode
      .fixed_recipient()
      .or_else(|| {
        conversation
          .working
          .first()
          .map(|working| Recipient::Agent(working.profile))
      })
      .unwrap_or(Recipient::Orchestrator);
    let fixed = mode.fixed_recipient().is_some();
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
        primary: None,
        now: None,
        shortcut: None,
      };
    }
    if let ComposerMode::PlanChanges { .. } = mode {
      let press = request_changes(props, plan, &text);
      return Presses {
        primary: press.clone(),
        now: None,
        shortcut: press,
      };
    }
    let send = |timing: Timing| -> Arc<dyn Fn() + Send + Sync> {
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
      let (action, sent_as) = (action.clone(), self.timing.clone());
      Arc::new(move || {
        sent_as.set(SentAs(timing));
        action.run(outgoing.clone());
      })
    };
    match mode {
      ComposerMode::Steer(working) => Presses {
        primary: Some(send(Timing::AfterTurn)),
        now: working.mid_turn.then(|| send(Timing::Now)),
        shortcut: Some(send(Timing::AfterTurn)),
      },
      _ => {
        let press = send(Timing::AfterTurn);
        Presses {
          primary: Some(press.clone()),
          now: None,
          shortcut: Some(press),
        }
      }
    }
  }

  /// The message field: it grows with its lines up to [`MAX_LINES`]. Once no agent reads messages,
  /// the field says so instead (02.S2 `m3gLZ`).
  fn field(
    &self,
    ctx: &Ctx,
    props: &ComposerProps,
    mode: &ComposerMode,
    shortcut: Option<Arc<dyn Fn() + Send + Sync>>,
  ) -> Element {
    if *mode == ComposerMode::Closed {
      return Text::new(&ctx.t("conversation.composer.closed"))
        .id("conversation-input-closed")
        .variant(ExtraTypography::ReviewNote.role())
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0))
        .into();
    }
    let (gained, lost) = (self.focused.clone(), self.focused.clone());
    let placeholder = match mode {
      ComposerMode::PlanChanges { .. } => ctx.t("runs.plan.notePlaceholder"),
      _ => ctx.t("conversation.composer.placeholder"),
    };
    // `UXzbp` is a 12px normal field line; ReviewNote's 1.6 leading inflated the empty composer.
    styled_input(ctx, props.draft.clone(), TypographyStyle::FieldLabel)
      .id("conversation-input")
      .textarea()
      .rows(1, MAX_LINES)
      .width(Dimension::Pct(100.0))
      .placeholder(&placeholder)
      .ref_element(self.input.clone())
      .on_focus(move || gained.set(true))
      .on_blur(move || lost.set(false))
      .on_key_down(move |event: KeyboardEvent| {
        if event.key == "Enter" && (event.ctrl || event.meta) {
          // The shortcut sends; it never also adds a newline, even when there is nothing to send.
          event.prevent_default();
          if let Some(send) = &shortcut {
            send();
          }
        }
      })
      .into()
  }

  /// The recipient and *Attach* at the left, the state's buttons at the right.
  fn action_row(
    &self,
    ctx: &Ctx,
    conversation: &Conversation,
    (mode, recipient): (&ComposerMode, Recipient),
    presses: &Presses,
    sending: bool,
  ) -> Element {
    let attach = ActionButton::new(ctx.t("conversation.composer.attach"))
      .icon(Icon::Paperclip)
      .height(Dimension::Px(BUTTON_HEIGHT))
      .id("conversation-attach");
    let primary_icon = match mode {
      ComposerMode::Steer(_) => Icon::Clock3,
      ComposerMode::PlanChanges { .. } => Icon::Undo2,
      ComposerMode::Continue => Icon::Play,
      _ => Icon::ArrowUp,
    };
    let under_way = sending && (!matches!(mode, ComposerMode::Steer(_)) || self.timing.get().0 == Timing::AfterTurn);
    let label = if under_way {
      ctx.t("conversation.composer.sending")
    } else {
      primary_label(ctx, mode)
    };
    let mut primary = ActionButton::new(label)
      .icon(primary_icon)
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .id("conversation-send");
    primary = match mode {
      // Send after this turn is the secondary of the two; Send now is the primary.
      ComposerMode::Steer(_) => primary,
      _ => primary.primary(),
    };
    primary = match &presses.primary {
      Some(press) => {
        let press = press.clone();
        primary.on_click(move || press())
      }
      None => primary.disabled(),
    };
    let now = match mode {
      ComposerMode::Steer(working) => Some(self.send_now(ctx, working.mid_turn, presses, sending)),
      _ => None,
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .child(self.recipient(ctx, conversation, (mode, recipient)))
      .child(attach.render(ctx))
      .child(Spacer::new().flex(1.0))
      .child(primary.render(ctx))
      .with_children(now)
      .into()
  }

  /// *Send now*: unavailable (with a lock) on a connection that takes no message mid-turn.
  fn send_now(&self, ctx: &Ctx, mid_turn: bool, presses: &Presses, sending: bool) -> Element {
    let label = if sending && self.timing.get().0 == Timing::Now {
      ctx.t("conversation.composer.sending")
    } else {
      ctx.t("runs.conversation.composer.sendNow")
    };
    let button = ActionButton::new(label)
      .icon(Icon::Zap)
      .primary()
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .id("conversation-send-now");
    match (&presses.now, mid_turn) {
      (Some(press), _) => {
        let press = press.clone();
        button.on_click(move || press()).render(ctx)
      }
      // No press makes the button unavailable, with its lock.
      (None, false) => button.render(ctx),
      (None, true) => button.disabled().render(ctx),
    }
  }

  /// "To: Builder · working" and the other recipients (`LDZMb`); fixed to the agent that asked
  /// while a question waits, and faded as a disabled button is once no agent reads messages.
  fn recipient(
    &self,
    ctx: &Ctx,
    conversation: &Conversation,
    (mode, recipient): (&ComposerMode, Recipient),
  ) -> Element {
    let options = conversation
      .recipients
      .iter()
      .map(|choice| (To(choice.recipient), recipient_label(ctx, conversation, mode, choice)));
    let usable = !matches!(
      mode,
      ComposerMode::Closed | ComposerMode::Answer { .. } | ComposerMode::PlanChanges { .. }
    );
    let select = Element::from(
      Select::new(self.to.clone())
        .options(options)
        .width(Dimension::Px(RECIPIENT_WIDTH))
        .height(Dimension::Px(BUTTON_HEIGHT))
        .style(select_style(ctx, BUTTON_HEIGHT))
        .focusable(usable),
    )
    .id("conversation-recipient");
    let select: Element = Column::new()
      .describe("recipient", format!("{recipient:?}"))
      .child(select)
      .into();
    if usable {
      return select;
    }
    // The select takes no pointer; the field around it says it cannot be used.
    Column::new()
      .opacity(DISABLED_OPACITY)
      .cursor(CursorIcon::NotAllowed)
      .describe("state", "disabled")
      .child(Column::new().pointer_events_none().child(select))
      .into()
  }
}

/// *Request changes* with the draft as the changes the plan needs.
fn request_changes(props: &ComposerProps, plan: &DecisionAction, text: &str) -> Option<Arc<dyn Fn() + Send + Sync>> {
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

/// *Answering Builder's question from 12:09*, *Changes to plan v1 · sent to Planner*.
fn context_line(ctx: &Ctx, mode: &ComposerMode) -> Option<Element> {
  let text = match mode {
    ComposerMode::Answer { agent, asked_at, .. } => ctx.t_args(
      "runs.conversation.composer.answering",
      [("agent", &**agent), ("time", display::clock(*asked_at).as_str())],
    ),
    ComposerMode::PlanChanges { agent, version } => ctx.t_args(
      "runs.conversation.composer.planChanges",
      [("version", version.to_string().as_str()), ("agent", &**agent)],
    ),
    _ => return None,
  };
  Some(
    Text::new(&text)
      .id("conversation-composer-context")
      .variant(TypographyStyle::Caption)
      .into(),
  )
}
