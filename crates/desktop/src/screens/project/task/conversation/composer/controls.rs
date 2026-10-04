//! The row inside the composer's box (`GtPaZ` in `BRMpj`): *+* (Attach, unavailable, with its
//! tooltip), the run's permissions as a chip, then at the right the recipient as a quiet selector
//! (*Builder* · *working*, chevron; a lock when it is fixed) and the round send button, whose icon
//! says what happens (arrow-up sends now, clock-3 queues) and whose tooltip says what sending does.
//! While the agent works mid-turn a chevron beside Send opens the send menu (`ru504`): *Send now ·
//! Enter* and *Send after this turn · Ctrl+Enter*, for this message only.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, Select, SelectOption, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, HitTestBehavior, SelectPartStyle, Style, border::Border, dimension::Dimension},
};
use orchester_core::conversation::Recipient;

use super::{
  super::super::permissions::{Look, RunPermissionsProps, RunPermissionsRow},
  ComposerProps, Presses, To,
  text::{primary_label, recipient_words, tip},
};
use crate::{
  tasks::conversation::composer::{ComposerMode, Queued},
  theme::{
    CONVERSATION_COMPOSER_ONE_LINE_MIN_WINDOW, DISABLED_OPACITY, ExtraRadius, ExtraShadow, ExtraSpacing,
    ExtraTypography,
  },
  ui::{
    field::select_style,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// The row's pills (`uBBXe`, `opOGv`), the send button and its chevron (`U0cjVo`, `sAQod`), the send
/// menu (`ru504`) and the tooltips' widths (`k565Es`, `TW93O`).
const PILL_HEIGHT: f32 = 28.0;
const SEND_SIZE: f32 = 32.0;
const CHEVRON_WIDTH: f32 = 24.0;
const MENU_WIDTH: f32 = 320.0;
const SEND_TIP_WIDTH: f32 = 300.0;
const ATTACH_TIP_WIDTH: f32 = 220.0;
/// How far above the box the send menu and the tooltips sit.
const POPUP_GAP: f32 = 6.0;

/// The row's own state: the send menu and what the pointer is over.
pub(super) struct Controls {
  pub(super) menu: Signal<bool>,
  send_hovered: Signal<bool>,
  attach_hovered: Signal<bool>,
  send_anchor: ElementRef,
  attach_anchor: ElementRef,
}

/// What the row shows and sends.
pub(super) struct RowProps<'a> {
  pub(super) props: &'a ComposerProps,
  pub(super) mode: &'a ComposerMode,
  /// Whom the message goes to, and its signal.
  pub(super) to: (&'a Signal<To>, Recipient),
  pub(super) presses: &'a Presses,
  pub(super) sending: bool,
}

impl Controls {
  pub(super) fn new(ctx: &mut lurq::app::ctx::Ctx) -> Self {
    Self {
      menu: ctx.signal(false),
      send_hovered: ctx.signal(false),
      attach_hovered: ctx.signal(false),
      send_anchor: ElementRef::new(),
      attach_anchor: ElementRef::new(),
    }
  }

  pub(super) fn row(&self, ctx: &mut Ctx, row: &RowProps<'_>) -> Element {
    // Reading the window's width re-renders the row as the window resizes.
    let narrow = ctx.window().logical_width() - row.props.beside < CONVERSATION_COMPOSER_ONE_LINE_MIN_WINDOW;
    let chip = permissions_chip(ctx, row.props, narrow);
    let menu = self.send_menu(ctx, row);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(self.attach(ctx))
      .with_children(chip)
      .child(Spacer::new().flex(1.0))
      .child(recipient(ctx, row))
      .child(self.send(ctx, row))
      .with_children(menu)
      .into()
  }

  /// *+*: attachments are not built; its tooltip says so (`TW93O`).
  fn attach(&self, ctx: &Ctx) -> Element {
    let (enter, leave) = (self.attach_hovered.clone(), self.attach_hovered.clone());
    let tip = self.attach_hovered.get().then(|| {
      tooltip(
        &self.attach_anchor,
        (
          "conversation-attach-tip",
          &ctx.t("runs.conversation.composer.attachUnavailable"),
        ),
        (ATTACH_TIP_WIDTH, Placement::TopStart),
      )
    });
    Row::new()
      .id("conversation-attach")
      .size(Dimension::Px(PILL_HEIGHT), Dimension::Px(PILL_HEIGHT))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(ExtraRadius::Round.role())
      .cursor(CursorIcon::NotAllowed)
      .describe("state", "disabled")
      .ref_element(self.attach_anchor.clone())
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(
        Row::new()
          .opacity(DISABLED_OPACITY)
          .child(icon(Icon::Plus, IconSize::Option, PaletteColor::TextPrimary)),
      )
      .with_children(tip)
      .into()
  }

  /// The chevron while the agent works mid-turn, and the send button; pointed at, its tooltip.
  fn send(&self, ctx: &Ctx, row: &RowProps<'_>) -> Element {
    let mid_turn = matches!(row.mode, ComposerMode::Steer(working) if working.mid_turn);
    let chevron = mid_turn.then(|| {
      let menu = self.menu.clone();
      Element::from(
        pressable(surface().id("conversation-send-options"), move || {
          menu.update(|open| *open = !*open)
        })
        .size(Dimension::Px(CHEVRON_WIDTH), Dimension::Px(SEND_SIZE))
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .rounded(ExtraRadius::Round.role())
        .background(PaletteColor::AccentMuted)
        .child(icon(Icon::ChevronDown, IconSize::Event, PaletteColor::TextPrimary)),
      )
    });
    let (glyph, outcome) = if row.sending {
      (Icon::Loader, "sending")
    } else if queues(row.mode) {
      (Icon::Clock3, "queues")
    } else {
      (Icon::ArrowUp, "now")
    };
    let button = Row::new()
      .size(Dimension::Px(SEND_SIZE), Dimension::Px(SEND_SIZE))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(ExtraRadius::Round.role())
      .background(PaletteColor::Accent)
      .child(icon(glyph, IconSize::Option, PaletteColor::TextInverse).id(format!("conversation-send-icon-{outcome}")));
    let button: Element = match &row.presses.send {
      Some(press) => {
        let press = press.clone();
        pressable(surface().id("conversation-send"), move || press())
          .describe("action", primary_label(ctx, row.mode).to_string())
          .child(button)
          .into()
      }
      None => Column::new()
        .id("conversation-send")
        .opacity(DISABLED_OPACITY)
        .cursor(CursorIcon::NotAllowed)
        .describe("state", "disabled")
        .child(button)
        .into(),
    };
    let (enter, leave) = (self.send_hovered.clone(), self.send_hovered.clone());
    let tip = (self.send_hovered.get() && !self.menu.get()).then(|| {
      tooltip(
        &self.send_anchor,
        ("conversation-send-tip", &tip(ctx, row.mode, &row.props.conversation)),
        (SEND_TIP_WIDTH, Placement::TopEnd),
      )
    });
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .ref_element(self.send_anchor.clone())
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .with_children(chevron)
      .child(button)
      .with_children(tip)
      .into()
  }

  /// The send menu (`ru504`) while it is open: *Send now* (the default, checked) and *Send after this
  /// turn*, each for this message only.
  fn send_menu(&self, ctx: &Ctx, row: &RowProps<'_>) -> Option<Element> {
    let ComposerMode::Steer(working) = row.mode else {
      return None;
    };
    if !working.mid_turn {
      return None;
    }
    let agent = &*working.agent;
    let enter = ctx.t("runs.conversation.menu.enter");
    let ctrl_enter = ctx.t(if cfg!(target_os = "macos") {
      "runs.conversation.menu.cmdEnter"
    } else {
      "runs.conversation.menu.ctrlEnter"
    });
    let options = vec![
      self.option(
        ("conversation-send-now", true),
        (
          ctx.t("runs.conversation.composer.sendNow"),
          ctx.t_args("runs.conversation.menu.nowMeta", [("agent", agent)]),
          enter,
        ),
        row.presses.send.clone(),
      ),
      self.option(
        ("conversation-send-after-turn", false),
        (
          ctx.t("runs.conversation.composer.sendAfterTurn"),
          ctx.t_args("runs.conversation.menu.afterTurnMeta", [("agent", agent)]),
          ctrl_enter,
        ),
        row.presses.after_turn.clone(),
      ),
    ];
    let panel = Column::new()
      .id("conversation-send-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .padding(ExtraSpacing::SelectMenu.role())
      .spacing(ExtraSpacing::Hair.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::SelectMenu.role())
      .box_shadow(ExtraShadow::Popover.role())
      .with_children(options);
    Some(
      Popup::new(self.send_anchor.clone(), panel)
        .open(self.menu.clone())
        .placement(Placement::TopEnd)
        .offset(0.0, -POPUP_GAP)
        .into(),
    )
  }

  /// One option of the send menu: what it does, for whom, its key; `current` is checked. Without a
  /// press (nothing written yet) it is faded and does nothing.
  fn option(
    &self,
    (id, current): (&'static str, bool),
    (label, meta, key): (Arc<str>, Arc<str>, Arc<str>),
    press: Option<Arc<dyn Fn() + Send + Sync>>,
  ) -> Element {
    let copy = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Hair.role())
      .child(
        Text::new(&label)
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      )
      .child(Text::new(&meta).variant(ExtraTypography::ReadBack.role()));
    let check = icon(Icon::Check, IconSize::Event, PaletteColor::TextPrimary).opacity(if current { 1.0 } else { 0.0 });
    let content = Row::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Indicator.role())
      .padding_horizontal(ExtraSpacing::Inline.role())
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .child(copy)
      .child(
        Text::new(&key)
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .child(check);
    let content = if current {
      content.background(PaletteColor::AccentMuted)
    } else {
      content
    };
    let Some(press) = press else {
      return Column::new()
        .id(id)
        .width(Dimension::Pct(100.0))
        .opacity(DISABLED_OPACITY)
        .child(content)
        .into();
    };
    let menu = self.menu.clone();
    pressable(surface().id(id), move || {
      menu.set(false);
      press();
    })
    .width(Dimension::Pct(100.0))
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .rounded(RadiusSize::Sm)
    .child(content)
    .into()
  }
}

/// Whether sending now queues the message instead of handing it over at once.
fn queues(mode: &ComposerMode) -> bool {
  match mode {
    ComposerMode::Steer(working) => !working.mid_turn,
    ComposerMode::Queue(Queued::Checkpoint) => false,
    ComposerMode::Queue(_) | ComposerMode::NotStarted => true,
    ComposerMode::Answer { .. }
    | ComposerMode::PlanChanges { .. }
    | ComposerMode::FollowUp
    | ComposerMode::Continue
    | ComposerMode::Closed => false,
  }
}

/// The run's permissions chip, for the step its agent works or last worked on; `None` for a tool
/// step, a task without a run, or once the profile version is gone.
fn permissions_chip(ctx: &mut Ctx, props: &ComposerProps, short: bool) -> Option<Element> {
  let run = props.detail.run.as_ref()?;
  let step = run.current_step()?;
  let chip = RunPermissionsProps {
    permissions: step.permissions.clone()?,
    step: step.clone(),
    run: run.id(),
    run_number: run.run.number().get(),
    live: !run.run.status().is_terminal(),
    repo_root: props.repo_root.clone(),
    look: Look::Chip { short },
  };
  Some(ctx.mount_keyed::<RunPermissionsRow>(&format!("conversation-permissions-{}", step.key), chip))
}

/// The recipient as a quiet selector (`opOGv`): *Builder* and what it does with a message, then a
/// chevron, or a lock while the recipient is fixed (`BeXbh` in `K1oNwn`). Its menu names every
/// agent with the same words.
fn recipient(ctx: &Ctx, row: &RowProps<'_>) -> Element {
  let (to, current) = row.to;
  let conversation = &row.props.conversation;
  let fixed = matches!(row.mode, ComposerMode::Answer { .. } | ComposerMode::PlanChanges { .. });
  let options: Vec<SelectOption<To>> = conversation
    .recipients
    .iter()
    .map(|choice| {
      let (name, state) = recipient_words(ctx, conversation, row.mode, choice);
      SelectOption::new(To(choice.recipient), name).detail(state)
    })
    .collect();
  let (name, state) = conversation
    .recipients
    .iter()
    .find(|choice| choice.recipient == current)
    .map(|choice| recipient_words(ctx, conversation, row.mode, choice))
    .unwrap_or_default();
  let end = if fixed {
    (Icon::Lock, PaletteColor::TextMuted)
  } else {
    (Icon::ChevronDown, PaletteColor::TextSecondary)
  };
  let style = select_style(ctx, PILL_HEIGHT)
    .trigger(
      SelectPartStyle::new()
        .rounded(ExtraRadius::Round.role())
        .min_height(PILL_HEIGHT),
    )
    .trigger_hovered(SelectPartStyle::new().background(PaletteColor::AccentMuted))
    .trigger_focused(SelectPartStyle::new().background(PaletteColor::AccentMuted))
    .trigger_open(SelectPartStyle::new().background(PaletteColor::AccentMuted));
  let select = Element::from(
    Select::new(to.clone())
      .options(options)
      .height(Dimension::Px(PILL_HEIGHT))
      .style(style)
      .focusable(!fixed)
      .trigger(move |_| pill(&name, &state, end.clone())),
  )
  .id("conversation-recipient");
  let select = Column::new()
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .describe("recipient", format!("{current:?}"))
    .child(select);
  if fixed {
    return select.pointer_events_none().into();
  }
  select.into()
}

/// The selector's face: the name (12/500), its state (`$muted`) and its end icon.
fn pill(name: &str, state: &str, (end, tone): (Icon, PaletteColor)) -> Element {
  Row::new()
    .height(Dimension::Px(PILL_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(Text::new(name).variant(ExtraTypography::NoticeHeading.role()).nowrap())
    .child(
      Text::new(state)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(icon(end, IconSize::Small, tone))
    .into()
}

/// A tooltip above `anchor` (`k565Es`): `$hover`, `$border`, radius 7, its text at 11 / 1.5,
/// wrapping within `width`; the pointer passes through it.
fn tooltip(anchor: &ElementRef, (id, text): (&'static str, &str), (width, placement): (f32, Placement)) -> Element {
  let panel = Column::new()
    .max_width(Dimension::Px(width))
    .padding_vertical(SpacingSize::Sm)
    .padding_horizontal(ExtraSpacing::Inline.role())
    .background(PaletteColor::AccentMuted)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Text::new(text)
        .id(id)
        .variant(ExtraTypography::CellLabel.role())
        .color(PaletteColor::TextPrimary),
    );
  Popup::new(anchor.clone(), panel)
    .open_when(true)
    .placement(placement)
    .offset(0.0, -POPUP_GAP)
    .hit_test(HitTestBehavior::None)
    .dismiss_on_outside_click(false)
    .dismiss_on_escape(false)
    .into()
}
