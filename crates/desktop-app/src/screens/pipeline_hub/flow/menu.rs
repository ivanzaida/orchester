//! The ⋯ on a card (`More` `q0bQ2G` in `Card · 03 Implement · hover` `GnnMB`, its menu `Step menu` `kzSvl` in P4
//! `naN3G`): the moves that dragging does, without dragging, then *Add a step after*, *Duplicate* and *Remove step*.
//! *Move up* and *Move down* are always listed (with their keys, Alt ↑ and Alt ↓); at the end of the line, or when a
//! move would break a failure route, the row is faded and says why. *Run in parallel with …* lists only the neighbours
//! the step can join, *Run on its own* only a step in a parallel group, and *Wait for some steps only…* only a step
//! with a stage before it whose *Waits for* row is not shown yet (P16b `P1853`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::MouseEvent,
    theme::{PaletteColor, RadiusSize},
  },
  components::{Button, Column, Placement, Popup},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension},
};
use orchester_core::{
  name::Name,
  resource::{MoveOffer, StepKey, StepMoves},
};
use orchester_workspace::pipelines::flow::{PipelineFlow, StepCard};

use super::{
  reorder::{Cards, Drive, Move, copy_name, returned_to_by},
  state::{Held, Reorder},
};
use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu_panel, separator},
    pressable::Focus,
    tooltip::Tooltip,
  },
};

/// The menu's width (`Step menu` `kzSvl`).
const MENU_WIDTH: f32 = 250.0;

/// What opens a step the menu made (its copy, or the step it added): the open step's head opens it, as its own
/// *Duplicate* does; a card's menu leaves the flow as it is.
pub(in crate::screens::pipeline_hub) type OpensMade = Arc<dyn Fn(StepKey) + Send + Sync>;

/// What a menu is made from.
pub(super) struct Menu<'a> {
  pub drive: &'a Drive,
  pub flow: &'a PipelineFlow,
  pub cards: &'a Cards,
  pub opens: Option<OpensMade>,
  /// What opens the step whose *Waits for* row *Wait for some steps only…* shows: the card's menu opens it; the open
  /// step's own menu has it open already.
  pub to_step: Option<OpensMade>,
}

/// `Step menu`'s ⋯ button on `card` (22 square, radius 4, the `hover` fill under the pointer), which opens or closes
/// the menu at `anchor`.
pub(super) fn more_button(ctx: &mut Ctx, reorder: &Reorder, card: &StepCard, anchor: ElementRef) -> Element {
  let (id, key) = (format!("pipeline-hub-flow-step-{}-more", card.key), card.key.clone());
  let (open, menu_for, refused) = (
    reorder.menu_open.clone(),
    reorder.menu_for.clone(),
    reorder.refused.clone(),
  );
  let (gained, lost) = (reorder.focused.clone(), reorder.focused.clone());
  let (held, held_by) = (card.key.clone(), card.key.clone());
  let label = ctx.t_args("pipeline_hub.flow.move.more", [("name", card.name.as_ref())]);
  let side = Dimension::Px(ControlHeight::Helper.value());
  let button = Focus::Keyboard
    .apply(Button::empty())
    .id(id.clone())
    .ref_element(anchor)
    .width(side)
    .height(side)
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm)
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("a11y_name", label.clone())
    .on_focus(move || gained.set(Held(Some(held.clone()))))
    .on_blur(move || {
      lost.update(|focused| {
        if focused.is(&held_by) {
          *focused = Held::default();
        }
      });
    })
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      refused.set(None);
      let opened = open.get_untracked() && menu_for.get_untracked().is(&key);
      menu_for.set(Held(Some(key.clone())));
      open.set(!opened);
    })
    .child(icon(Icon::Ellipsis, IconSize::Md, PaletteColor::TextSecondary).id(format!("{id}-icon")));
  if reorder.menu_open.get() && reorder.menu_for.get().is(&card.key) {
    return button.into();
  }
  Tooltip::new(id, label).wrap(ctx, button)
}

/// The popup of `card`'s menu, open while the flow's menu is open for it.
pub(super) fn popup(ctx: &mut Ctx, menu: &Menu<'_>, card: &StepCard, anchor: ElementRef) -> Popup {
  let panel = panel(ctx, menu, card);
  Popup::new(anchor, panel)
    .open(menu.drive.reorder.menu_open.clone())
    .placement(Placement::BottomEnd)
    .offset(0.0, ExtraSpacing::Snug.value())
}

fn panel(ctx: &mut Ctx, menu: &Menu<'_>, card: &StepCard) -> Column {
  let open = &menu.drive.reorder.menu_open;
  let id = |part: &str| format!("pipeline-hub-flow-menu-{part}");
  let mut rows: Vec<Element> = Vec::new();
  match menu.drive.current.possible_moves(&card.key) {
    Ok(moves) => {
      rows.extend(move_rows(ctx, menu, card, &moves));
      rows.extend(wait_some(ctx, menu, card));
    }
    Err(error) => rows.push(
      MenuItem::unavailable(
        id("moves"),
        ctx.t("pipeline_hub.flow.move.up"),
        menu.cards.reason(ctx, &error, Some(&card.key)),
      )
      .icon(Icon::ArrowUp)
      .render(ctx, open),
    ),
  }
  rows.push(separator(ctx));
  rows.push(add_after(ctx, menu, card).render(ctx, open));
  let (drive, key, opens) = (menu.drive.clone(), card.key.clone(), menu.opens.clone());
  let copy = copy_name(ctx, &card.name);
  rows.push(
    MenuItem::new(id("duplicate"), ctx.t("pipeline_hub.flow.move.duplicate"), move || {
      open_made(opens.as_ref(), drive.duplicate(&key, copy.clone()));
    })
    .icon(Icon::Copy)
    .render(ctx, open),
  );
  rows.push(remove_row(ctx, menu, card).render(ctx, open));
  menu_panel("pipeline-hub-flow-menu")
    .width(Dimension::Px(MENU_WIDTH))
    .with_children(rows)
}

/// *Wait for some steps only…* (P16b `P1853`): the step opens with its *Waits for* row, which names the steps it waits
/// for. Only for a step with a stage before it whose row is not shown yet: one that waits for part of that stage, or
/// whose row the person opened or changed, has the row already.
fn wait_some(ctx: &mut Ctx, menu: &Menu<'_>, card: &StepCard) -> Option<Element> {
  let first = menu
    .flow
    .stages
    .first()
    .is_some_and(|stage| stage.cards.iter().any(|other| other.key == card.key));
  if first || card.waits.is_some() || menu.drive.edit.shows_waits(&card.key) {
    return None;
  }
  let (drive, key, to_step) = (menu.drive.clone(), card.key.clone(), menu.to_step.clone());
  let item = MenuItem::new(
    "pipeline-hub-flow-menu-wait-some",
    ctx.t("pipeline_hub.flow.move.waitSome"),
    move || {
      if drive.locked() {
        return;
      }
      drive.edit.show_waits(&key);
      if let Some(to_step) = &to_step {
        to_step(key.clone());
      }
    },
  );
  Some(item.icon(Icon::GitMerge).render(ctx, &menu.drive.reorder.menu_open))
}

/// *Remove step*, faded with why when another step returns to this one on failure.
fn remove_row(ctx: &Ctx, menu: &Menu<'_>, card: &StepCard) -> MenuItem {
  let (id, label) = ("pipeline-hub-flow-menu-remove", ctx.t("pipeline_hub.flow.move.remove"));
  let item = match returned_to_by(&menu.drive.current, &card.key) {
    Some(step) => MenuItem::unavailable(
      id,
      label,
      ctx.t_args(
        "pipeline_hub.flow.move.reason.removeTarget",
        [("step", menu.cards.label(ctx, &step).to_string())],
      ),
    ),
    None => {
      let (drive, key) = (menu.drive.clone(), card.key.clone());
      MenuItem::new(id, label, move || drive.remove(&key))
    }
  };
  item.icon(Icon::Trash2).danger()
}

/// *Move up*, *Move down*, the neighbours to run in parallel with and *Run on its own*, then the line before the rest.
fn move_rows(ctx: &mut Ctx, menu: &Menu<'_>, card: &StepCard, moves: &StepMoves) -> Vec<Element> {
  let open = &menu.drive.reorder.menu_open;
  let mut rows: Vec<Element> = Vec::new();
  let item = |ctx: &mut Ctx, (part, key): (&str, &str), offer: &MoveOffer, change: Move, glyph: Icon| {
    let id = format!("pipeline-hub-flow-menu-{part}");
    let label = ctx.t(key);
    let item = match offer {
      MoveOffer::Available => {
        let (drive, step) = (menu.drive.clone(), card.key.clone());
        MenuItem::new(id, label, move || drive.apply(&step, &change, false))
      }
      MoveOffer::Unavailable => MenuItem::unavailable(id, label, edge(ctx, part)),
      MoveOffer::Refused(error) => MenuItem::unavailable(id, label, menu.cards.reason(ctx, error, Some(&card.key))),
    };
    item.icon(glyph)
  };
  // A step of a parallel group leaves it when it moves up or down (D-023): the row says so.
  let leaves_group = menu
    .flow
    .stages
    .iter()
    .any(|stage| stage.cards.len() > 1 && stage.cards.iter().any(|other| other.key == card.key));
  let (up_words, down_words) = if leaves_group {
    (
      "pipeline_hub.stepFlow.upOutOfGroup",
      "pipeline_hub.stepFlow.downOutOfGroup",
    )
  } else {
    ("pipeline_hub.flow.move.up", "pipeline_hub.flow.move.down")
  };
  let up = item(ctx, ("up", up_words), &moves.up, Move::Up, Icon::ArrowUp).shortcut(keys(ctx, "up"));
  let down = item(ctx, ("down", down_words), &moves.down, Move::Down, Icon::ArrowDown).shortcut(keys(ctx, "down"));
  rows.push(up.render(ctx, open));
  rows.push(down.render(ctx, open));
  let joins: Vec<Element> = moves
    .parallel_with
    .iter()
    .enumerate()
    .filter(|(_, choice)| choice.offer != MoveOffer::Unavailable)
    .map(|(index, choice)| {
      let id = format!("pipeline-hub-flow-menu-parallel-{index}");
      let label = ctx.t_args(
        "pipeline_hub.flow.move.parallelWith",
        [("with", menu.cards.group(ctx, &choice.with).to_string())],
      );
      let row = match (&choice.offer, choice.with.first()) {
        (MoveOffer::Available, Some(neighbour)) => {
          let (drive, step, neighbour) = (menu.drive.clone(), card.key.clone(), neighbour.clone());
          MenuItem::new(id, label, move || {
            drive.apply(&step, &Move::Beside(neighbour.clone()), false);
          })
        }
        (MoveOffer::Refused(error), _) => {
          MenuItem::unavailable(id, label, menu.cards.reason(ctx, error, Some(&card.key)))
        }
        _ => MenuItem::unavailable(id, label, ctx.t("pipeline_hub.flow.move.reason.broken")),
      };
      row.icon(Icon::Split).render(ctx, open)
    })
    .collect();
  let mut group = joins;
  if moves.on_its_own.is_available() {
    let (drive, step) = (menu.drive.clone(), card.key.clone());
    group.push(
      MenuItem::new(
        "pipeline-hub-flow-menu-own",
        ctx.t("pipeline_hub.flow.move.onItsOwn"),
        move || drive.apply(&step, &Move::OnItsOwn, false),
      )
      .icon(Icon::ArrowRight)
      .render(ctx, open),
    );
  }
  if !group.is_empty() {
    rows.push(separator(ctx));
    rows.extend(group);
  }
  rows
}

/// The keys of the move `part` (`up` or `down`), as the platform writes them: Alt on Windows, ⌥ on macOS.
fn keys(ctx: &Ctx, part: &str) -> Arc<str> {
  ctx.t(match (part, cfg!(target_os = "macos")) {
    ("up", false) => "pipeline_hub.flow.move.upKeys",
    ("up", true) => "pipeline_hub.flow.move.upKeysMac",
    (_, false) => "pipeline_hub.flow.move.downKeys",
    (_, true) => "pipeline_hub.flow.move.downKeysMac",
  })
}

/// Why *Move up* or *Move down* does nothing: nothing runs before or after the step.
fn edge(ctx: &Ctx, part: &str) -> Arc<str> {
  ctx.t(if part == "up" {
    "pipeline_hub.flow.move.nothingBefore"
  } else {
    "pipeline_hub.flow.move.nothingAfter"
  })
}

/// *Add a step after*: a step with no profile yet, named *New step*, in series after the card's stage.
fn add_after(ctx: &mut Ctx, menu: &Menu<'_>, card: &StepCard) -> MenuItem {
  let (drive, flow, key, opens) = (
    menu.drive.clone(),
    menu.flow.clone(),
    card.key.clone(),
    menu.opens.clone(),
  );
  let name = new_step_name(ctx);
  MenuItem::new(
    "pipeline-hub-flow-menu-add",
    ctx.t("pipeline_hub.flow.move.addAfter"),
    move || open_made(opens.as_ref(), drive.add(name.clone(), Some(&key), &flow)),
  )
  .icon(Icon::Plus)
}

/// Opens `made`, a step the menu made, where the menu says so.
fn open_made(opens: Option<&OpensMade>, made: Option<StepKey>) {
  if let (Some(opens), Some(made)) = (opens, made) {
    opens(made);
  }
}

/// The name an added step starts with.
pub(super) fn new_step_name(ctx: &Ctx) -> Name {
  Name::new(&ctx.t("pipeline_hub.flow.move.newStep")).expect("the translated name of a new step is a valid name")
}
