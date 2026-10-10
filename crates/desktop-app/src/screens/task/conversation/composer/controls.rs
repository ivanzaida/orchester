//! The row inside the composer's box (`Bar` `keqaj` in `QQ2ZM`), 6 apart: *Attach*, not built, at half
//! strength with a lock (`Attach · unavailable` in `mZIqi`: a 15 plus and a 9 lock in a 26 square); the
//! recipient as a pill (`Recipient` `AOjdU`: 24 high, `surface_raised` in a `border` stroke, radius 6, a 12
//! `bot` and the words in 11.5 `text_secondary`, 5 apart, 8 in from its sides; a lock at its end while the
//! recipient is fixed); and at the end the send button: `accent`, radius 6, 28 high, its icon in `on_accent`
//! — alone in a 28 square while an agent works (`Send button/split` `wPMDa`), else with its words in 12/500,
//! 10 in from its sides and 6 from the icon (`I4yc5z` in `YhiGC`). Where sending has two ways, a line and a
//! chevron split the button (`KkfI8`, `ZFD5V`: 1 by 16, then 20 wide, a 12 chevron) and open the send menu
//! over it. The permissions chip and the recipient menu are not part of this build (D4).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Placement, Popup, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::conversation::Recipient;
use orchester_workspace::tasks::conversation::{Conversation, composer::ComposerMode};

use super::{
  Press,
  words::{self, Way},
};
use crate::{
  theme::{
    ComposerColor, ComposerText, ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize,
  },
  ui::{
    icon::{Icon, glyph, icon},
    menu::{MenuItem, menu_panel},
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// *Attach*'s square (`hprBf`), the split button's chevron (`ZFD5V`) and the line before it (`KkfI8`); the
/// send menu's width, which the design does not draw (the old app's `ru504`).
const ATTACH_SIZE: f32 = 26.0;
const MENU_WIDTH: f32 = 320.0;
const OPTIONS_WIDTH: f32 = 20.0;
const DIVIDER_HEIGHT: f32 = 16.0;

/// How the send button can be pressed now.
#[derive(Clone)]
pub(super) enum Sending {
  /// Nothing is written yet.
  Empty,
  /// The message is being sent.
  Busy,
  /// Pressing sends: the primary way, and the second where the state has one.
  Ready { primary: Press, second: Option<Press> },
}

/// What the row shows.
pub(super) struct Bar<'a> {
  pub(super) mode: &'a ComposerMode,
  pub(super) conversation: &'a Conversation,
  pub(super) recipient: Recipient,
  pub(super) sending: Sending,
  /// The send menu is open.
  pub(super) menu: &'a Signal<bool>,
  /// The send button, which the menu opens over.
  pub(super) anchor: &'a ElementRef,
}

pub(super) fn bar(ctx: &mut Ctx, bar: &Bar<'_>) -> Element {
  let fixed = match bar.mode {
    ComposerMode::Answer { .. } if bar.mode.fixed_recipient().is_some() => {
      Some(ctx.t("composer.recipient.fixed.answer"))
    }
    ComposerMode::PlanChanges { .. } => Some(ctx.t("composer.recipient.fixed.plan")),
    _ => None,
  };
  let recipient = words::recipient(ctx, bar.mode, bar.conversation, bar.recipient);
  Row::new()
    .id("composer-bar")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(attach(ctx))
    .child(recipient_pill(ctx, (&recipient, recipient_icon(bar.mode)), fixed))
    .child(Spacer::new().flex(1.0))
    .child(send(ctx, bar))
    .into()
}

/// *Attach*: not built, so it stays at half strength with a lock, and its tooltip says so.
fn attach(ctx: &mut Ctx) -> Element {
  let reason = ctx.t("composer.attach");
  let square = inert(Button::empty())
    .id("composer-attach")
    .describe("state", "unavailable")
    .describe("reason", reason.clone())
    .size(Dimension::Px(ATTACH_SIZE), Dimension::Px(ATTACH_SIZE))
    .spacing(ExtraSpacing::Hair.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .opacity(DISABLED_OPACITY)
    .child(icon(Icon::Plus, IconSize::Nav, PaletteColor::TextMuted))
    .child(Text::new(&glyph(Icon::Lock)).variant(ComposerText::AttachLock.role()));
  Tooltip::new("composer-attach", reason).wrap(ctx, square)
}

/// The recipient's glyph: before the run starts, the compass of the first agent not started yet (`hJC7T`
/// in `maV9f`); else an agent at work, `bot`.
const fn recipient_icon(mode: &ComposerMode) -> Icon {
  match mode {
    ComposerMode::NotStarted => Icon::Compass,
    _ => Icon::Bot,
  }
}

/// The recipient pill: who gets the message and what it does with it; while it is fixed, a lock, and
/// why in its tooltip.
fn recipient_pill(ctx: &mut Ctx, (words, glyph): (&str, Icon), fixed: Option<Arc<str>>) -> Element {
  let pill = Row::new()
    .id("composer-recipient")
    .describe("fixed", fixed.is_some().to_string())
    .height(Dimension::Px(ControlHeight::Sm.value()))
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(words)
        .id("composer-recipient-text")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .with_children(
      fixed
        .is_some()
        .then(|| icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted)),
    );
  match fixed {
    Some(reason) => Tooltip::new("composer-recipient", reason.clone()).wrap(ctx, pill.describe("reason", reason)),
    None => pill.into(),
  }
}

/// The send button, split with the send menu where the state has two ways of sending.
fn send(ctx: &mut Ctx, bar: &Bar<'_>) -> Element {
  let (label, glyph) = words::primary(ctx, bar.mode);
  let alone = matches!(bar.mode, ComposerMode::Steer(_));
  let ways = words::menu(ctx, bar.mode);
  let face = face(&label, glyph, alone, &bar.sending);
  let primary: Element = match &bar.sending {
    Sending::Ready { primary, .. } => {
      let press = primary.clone();
      pressable(move || press())
        .id("composer-send")
        .describe("state", "enabled")
        .hovered_style(Style::new().background(PaletteColor::AccentHover))
        .child(face)
        .into()
    }
    Sending::Busy => inert(Button::empty())
      .id("composer-send")
      .describe("state", "loading")
      .cursor(CursorIcon::Progress)
      .child(face)
      .into(),
    Sending::Empty => inert(Button::empty())
      .id("composer-send")
      .describe("state", "disabled")
      .child(face)
      .into(),
  };
  // Faded while nothing is written: its tooltip says so.
  let tip = match bar.sending {
    Sending::Empty => Tooltip::new("composer-send", ctx.t("composer.send.empty")),
    Sending::Busy | Sending::Ready { .. } => {
      Tooltip::new("composer-send", label.clone()).shortcut(ctx.t("composer.keys.enter"))
    }
  };
  let primary = tip.wrap(ctx, primary);
  let split = ways.map(|ways| (options(ctx, bar), ways));
  let menu = split.as_ref().map(|(_, ways)| send_menu(ctx, bar, ways));
  let enabled = matches!(bar.sending, Sending::Ready { .. });
  Row::new()
    .id("composer-send-button")
    .describe("action", label.to_string())
    .ref_element(bar.anchor.clone())
    .height(Dimension::Px(ControlHeight::Md.value()))
    .align_items(Alignment::Center)
    .background(PaletteColor::Accent)
    .rounded(RadiusSize::Md)
    .clip()
    .opacity(if enabled { 1.0 } else { DISABLED_OPACITY })
    .child(primary)
    .with_children(split.map(|(options, _)| options))
    .with_children(menu)
    .into()
}

/// The send button's icon (the loader while it sends), and its words unless it stands alone.
fn face(label: &str, glyph: Icon, alone: bool, sending: &Sending) -> Row {
  let glyph = if matches!(sending, Sending::Busy) {
    Icon::LoaderCircle
  } else {
    glyph
  };
  let on_accent = ExtraColor::OnAccent.role();
  if alone {
    return Row::new()
      .size(
        Dimension::Px(ControlHeight::Md.value()),
        Dimension::Px(ControlHeight::Md.value()),
      )
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .child(icon(glyph, IconSize::Nav, on_accent).id("composer-send-icon"));
  }
  Row::new()
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, on_accent.clone()).id("composer-send-icon"))
    .child(
      Text::new(label)
        .id("composer-send-label")
        .variant(TypographyStyle::Button)
        .color(on_accent)
        .nowrap(),
    )
}

/// The line and the chevron that open the send menu.
fn options(ctx: &mut Ctx, bar: &Bar<'_>) -> Element {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  let divider = Row::new()
    .size(Dimension::Px(line), Dimension::Px(DIVIDER_HEIGHT))
    .background(ComposerColor::SplitDivider.role());
  let chevron = Row::new()
    .size(Dimension::Px(OPTIONS_WIDTH), Dimension::Px(ControlHeight::Md.value()))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::ChevronDown, IconSize::Sm, ExtraColor::OnAccent.role()));
  let label = ctx.t("composer.menu.options");
  let open = bar.menu.clone();
  let chevron: Element = if matches!(bar.sending, Sending::Ready { .. }) {
    pressable(move || open.update(|open| *open = !*open))
      .id("composer-send-options")
      .describe("a11y_name", label)
      .hovered_style(Style::new().background(PaletteColor::AccentHover))
      .child(chevron)
      .into()
  } else {
    inert(Button::empty()).id("composer-send-options").child(chevron).into()
  };
  Row::new()
    .align_items(Alignment::Center)
    .child(divider)
    .child(chevron)
    .into()
}

/// The send menu over the button: the primary way (checked, Enter), then the second (Ctrl/Cmd+Enter), or
/// the second way unavailable with why.
fn send_menu(ctx: &mut Ctx, bar: &Bar<'_>, (first, second): &(Way, Way)) -> Element {
  let Sending::Ready { primary, second: press } = &bar.sending else {
    return Column::new().into();
  };
  let first = item("composer-send-first", first, Some(primary.clone()))
    .chosen(true)
    .render(ctx, bar.menu);
  let second = match press {
    Some(press) if !second.unavailable => item("composer-send-second", second, Some(press.clone()))
      .shortcut(words::second_keys(ctx))
      .render(ctx, bar.menu),
    _ => item("composer-send-second", second, None).render(ctx, bar.menu),
  };
  let panel = menu_panel("composer-send-menu")
    .width(Dimension::Px(MENU_WIDTH))
    .child(first)
    .child(second);
  Popup::new(bar.anchor.clone(), panel)
    .open(bar.menu.clone())
    .placement(Placement::TopEnd)
    // Away from the button: lurq moves an overlay above its anchor up by a positive offset.
    .offset(0.0, ExtraSpacing::Snug.value())
    .into()
}

/// One way of sending as a row of the menu; without `press` it cannot be chosen, and says why.
fn item(id: &str, way: &Way, press: Option<Press>) -> MenuItem {
  match press {
    Some(press) if !way.unavailable => MenuItem::new(id, way.label.clone(), move || press()).detail(way.detail.clone()),
    _ => MenuItem::unavailable(id, way.label.clone(), way.detail.clone()),
  }
}
