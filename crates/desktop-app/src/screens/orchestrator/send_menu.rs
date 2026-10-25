//! The send button while the Orchestrator answers (ORC-92), as the task's composer splits it (`Send button/split`
//! `wPMDa`): the 28 `accent` square sends *after this answer*, the default; a line and a chevron (`KkfI8`, `ZFD5V`)
//! open the send menu over it, whose rows (`Menu item/detail` `j6ZUhL`) are the three ways of sending while it answers:
//! *Send after this answer* (chosen, Ctrl/Cmd+Enter), *Send now* and *Stop and send*, each with what it does. The
//! design draws no menu for the chat; it is the task composer's, with the chat's words.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Button, Placement, Popup, Row},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension},
};

use crate::{
  theme::{ComposerColor, ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu_panel},
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// The menu's width, as the task composer's send menu (`composer::controls`), and the chevron's room and the line
/// before it (`ZFD5V`, `KkfI8`).
const MENU_WIDTH: f32 = 320.0;
const OPTIONS_WIDTH: f32 = 20.0;
const DIVIDER_HEIGHT: f32 = 16.0;

/// A press of the send button or of a row of its menu.
pub(super) type Press = Arc<dyn Fn() + Send + Sync>;

/// The three ways of sending while the Orchestrator answers.
#[derive(Clone)]
pub(super) struct Ways {
  pub(super) after_answer: Press,
  pub(super) now: Press,
  pub(super) stop_and_send: Press,
}

/// The split send button, with the menu over it while `open` holds; its chevron is `anchor`. Without `ways` nothing is
/// written yet: the button is faded, as the task composer's, and its tooltip says why (`empty`).
pub(super) fn split(
  ctx: &mut Ctx,
  (ways, empty): (Option<&Ways>, &Arc<str>),
  (open, anchor): (&Signal<bool>, &ElementRef),
) -> Element {
  let side = Dimension::Px(ControlHeight::Md.value());
  let label = ctx.t("orchestrator.composer.afterAnswer");
  let face = Row::new()
    .size(side, side)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::ArrowUp, IconSize::Md, ExtraColor::OnAccent.role()));
  let square = match ways {
    Some(ways) => {
      let after = ways.after_answer.clone();
      pressable(move || after())
        .describe("state", "enabled")
        .hovered_style(Style::new().background(PaletteColor::AccentHover))
    }
    None => inert(Button::empty()).describe("state", "disabled"),
  }
  .id("orchestrator-send")
  .describe("a11y_name", label.clone())
  .child(face);
  let tip = match ways {
    Some(_) => Tooltip::new("orchestrator-send", label).shortcut(ctx.t(if cfg!(target_os = "macos") {
      "orchestrator.keys.cmdEnter"
    } else {
      "orchestrator.keys.ctrlEnter"
    })),
    None => Tooltip::new("orchestrator-send", empty.clone()),
  };
  let square = tip.wrap(ctx, square);
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  let divider = Row::new()
    .size(Dimension::Px(line), Dimension::Px(DIVIDER_HEIGHT))
    .background(ComposerColor::SplitDivider.role());
  let chevron = Row::new()
    .size(Dimension::Px(OPTIONS_WIDTH), side)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::ChevronDown, IconSize::Sm, ExtraColor::OnAccent.role()));
  let chevron = match ways {
    Some(_) => {
      let toggle = open.clone();
      pressable(move || toggle.update(|open| *open = !*open))
        .hovered_style(Style::new().background(PaletteColor::AccentHover))
    }
    None => inert(Button::empty()),
  }
  .id("orchestrator-send-options")
  .ref_element(anchor.clone())
  .describe("a11y_name", ctx.t("orchestrator.composer.options"))
  .child(chevron);
  let menu = ways.map(|ways| menu(ctx, ways, (open, anchor)));
  Row::new()
    .id("orchestrator-send-button")
    .describe("action", "after-answer")
    .height(side)
    .align_items(Alignment::Center)
    .background(PaletteColor::Accent)
    .rounded(RadiusSize::Md)
    .clip()
    .opacity(if ways.is_some() { 1.0 } else { DISABLED_OPACITY })
    .child(square)
    .child(divider)
    .child(chevron)
    .with_children(menu)
    .into()
}

/// The send menu over the button: *Send after this answer* (chosen, Ctrl/Cmd+Enter), *Send now*, *Stop and send*.
fn menu(ctx: &mut Ctx, ways: &Ways, (open, anchor): (&Signal<bool>, &ElementRef)) -> Element {
  let keys = ctx.t(if cfg!(target_os = "macos") {
    "orchestrator.keys.cmdEnter"
  } else {
    "orchestrator.keys.ctrlEnter"
  });
  let item = |id: &str, (label, detail): (&str, &str), press: &Press| {
    let press = press.clone();
    MenuItem::new(id, ctx.t(label), move || press()).detail(ctx.t(detail))
  };
  let after = item(
    "orchestrator-send-after-answer",
    (
      "orchestrator.composer.afterAnswer",
      "orchestrator.composer.afterAnswerDetail",
    ),
    &ways.after_answer,
  )
  .chosen(true)
  .shortcut(keys);
  let now = item(
    "orchestrator-send-now",
    ("orchestrator.composer.now", "orchestrator.composer.nowDetail"),
    &ways.now,
  );
  let stop = item(
    "orchestrator-stop-and-send",
    (
      "orchestrator.composer.stopAndSend",
      "orchestrator.composer.stopAndSendDetail",
    ),
    &ways.stop_and_send,
  );
  let panel = menu_panel("orchestrator-send-menu")
    .width(Dimension::Px(MENU_WIDTH))
    .child(after.render(ctx, open))
    .child(now.render(ctx, open))
    .child(stop.render(ctx, open));
  Popup::new(anchor.clone(), panel)
    .open(open.clone())
    .placement(Placement::TopEnd)
    // Away from the button: lurq moves an overlay above its anchor up by a positive offset.
    .offset(0.0, ExtraSpacing::Snug.value())
    .into()
}
