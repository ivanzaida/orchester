//! A column of the board (`Column · Backlog` `v6Z67H` in `01 Board` `ntozw`): its head, 28 high (the status's 13 icon
//! in its tone, its name in 12/500 `text_secondary`, its count in 12 `text_muted`, how many of its cards need the
//! person as an 11 `warning` hand and count, then Fold and, in Backlog alone, Add a draft), and its cards 6 apart,
//! which scroll. While a lifted card is over it, a column that takes it is `accent_muted` in a `border_strong` stroke
//! with the drop's slot where the card would go (`Column · Todo` in `02 Dragging` `Yyeme`); Done offers *Mark as done*
//! at its top and, while Canceled is hidden, *Cancel task* at its foot; one that refuses it is `danger_muted` in a
//! `danger` stroke with why (`03 Drop refused` `ZaM50`). Without cards it says why (`Empty column, no filter`
//! `B8kYj`, `Search hides every card` `KhgzA`). A folded column is a 40 wide strip with its icon, count and name
//! (`Collapsed · Backlog` `F7EfQ`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding, transform::Transform2D},
};
use orchester_core::{id::TaskId, task::organization::WorkflowStatus};

use super::{
  actions::{BoardState, Drag, Focused, Refused, Selected, Target},
  card::{CardLook, CardModel},
  card_view::{CardProps, CardView},
  display::Display,
  drop::refusal_of,
  model::{ColumnKey, ColumnModel},
  quick_add::{QuickAdd, QuickAddProps},
  refusal::{refusal_card, refusal_note},
};
use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    pressable::pressable,
  },
};

/// A column's head (`Header` `Ob9rV`), a folded column's width (`Collapsed · Backlog` `F7EfQ`), and the drop slots
/// of Done (`Drop slot · Done` `reJKk`).
const HEAD_HEIGHT: f32 = 28.0;
const STRIP_WIDTH: f32 = 40.0;
/// The room of a folded column's name, turned on its side.
const STRIP_LABEL_ROOM: f32 = 96.0;
const SLOT_HEIGHT: f32 = 44.0;

/// What every column of a render shares.
pub struct Shared<'a> {
  pub display: &'a Display,
  pub selected: &'a Selected,
  pub focused: Focused,
  pub drag: Option<&'a Drag>,
  pub refused: Option<&'a Refused>,
  /// The task whose drop waits for the person's answer.
  pub waiting: Option<TaskId>,
  /// Cards lift, and have a menu.
  pub lifts: bool,
  pub menus: bool,
  /// Backlog's quick add: allowed, or locked for this reason; `None` where the columns are no statuses.
  pub quick_add: Option<Result<(), Arc<str>>>,
  /// The search or the filters hide some tasks; what clears them.
  pub filtered: bool,
  pub clear: Arc<dyn Fn() + Send + Sync>,
  pub project: &'a crate::project::ShownProject,
}

/// `column` holding `cards`, in row `lane` of the board when it has rows: its head shows and its cards scroll only
/// outside rows, where the row's head and the board's column heads say what it is.
pub fn column(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  (column, cards): (&ColumnModel, &[CardModel]),
  lane: Option<&str>,
) -> Element {
  let target = Target {
    column: column.key.clone(),
    lane: lane.map(str::to_owned),
  };
  let name = column.key.name();
  let id = match lane {
    Some(lane) => format!("tasks-board-column-{name}-{lane}"),
    None => format!("tasks-board-column-{name}"),
  };
  let over = shared.drag.filter(|drag| drag.over.as_ref() == Some(&target));
  let refusing = over.and_then(|drag| refusal_of(&drag.card, &target));
  let body = body(ctx, (state, shared), (column, cards), (&target, &id, over, refusing.is_some()));
  let frame = Column::new()
    .id(id.clone())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .describe("column", name)
    .ref_element(state.target_ref(target));
  let frame = match lane {
    Some(_) => frame.child(body),
    None => frame
      .height(Dimension::Pct(100.0))
      .child(head(ctx, (state, shared), column, &id))
      .child(ScrollVertical::new(body).flex(1.0).width(Dimension::Pct(100.0))),
  };
  match (over, refusing) {
    (Some(_), Some(_)) => lit(frame, PaletteColor::DangerMuted, PaletteColor::Danger).describe("drop", "refused"),
    (Some(_), None) => lit(frame, PaletteColor::AccentMuted, PaletteColor::BorderStrong).describe("drop", "takes"),
    _ => frame,
  }
  .into()
}

/// The column's cards, after a refusal it says, with the lifted card's slot while the drag is over it and Backlog's
/// foot; why it shows none, without them.
fn body(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  (column, cards): (&ColumnModel, &[CardModel]),
  (target, id, over, refusing): (&Target, &str, Option<&Drag>, bool),
) -> Column {
  let mut body = Column::new().width(Dimension::Pct(100.0)).spacing(ExtraSpacing::Snug.role());
  if let Some(refused) = shared.refused.filter(|refused| refused.column == column.key)
    && (over.is_some() || shared.drag.is_none())
  {
    body = body.child(refusal_card(ctx, refused, &column.label));
  } else if let (Some(drag), None) = (shared.drag, over)
    && let Some(refusal) = refusal_of(&drag.card, target)
  {
    body = body.child(refusal_note(ctx, &refusal, &format!("{id}-refused")));
  }
  body = match over.filter(|_| !refusing) {
    Some(drag) => body.with_children(slots(ctx, state, (shared, target), cards, drag)),
    None => body.with_children(card_views(ctx, shared, (target, cards))),
  };
  body = body.with_children(foot(ctx, shared, column, target.lane.as_deref()));
  if cards.is_empty() && shared.drag.is_none() && target.lane.is_none() {
    body = body.child(empty(ctx, shared, column));
  }
  body
}

/// A column the drag is over, in its tone: filled, in a stroke, 4 inside it at its sides and foot.
fn lit(frame: Column, fill: PaletteColor, stroke: PaletteColor) -> Column {
  frame
    .background(fill)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Lg)
    .padding(
      Padding::new()
        .left(SpacingSize::Xs)
        .right(SpacingSize::Xs)
        .bottom(SpacingSize::Xs),
    )
}

/// The cards of a column, each mounted as itself.
fn card_views(ctx: &mut Ctx, shared: &Shared<'_>, (place, cards): (&Target, &[super::card::CardModel])) -> Vec<Element> {
  cards
    .iter()
    .map(|card| {
      let id = card.row.id;
      let look = CardLook {
        selected: shared.selected.0.contains(&id),
        selecting: !shared.selected.0.is_empty(),
        lifted: shared.drag.is_some_and(|drag| drag.card.row.id == id),
        waiting: shared.waiting == Some(id),
        focused: shared.focused.0 == Some(id),
        acting: false,
      };
      // A task kept Not shared, or a list that is no longer the person's, does not lift.
      let lifts = shared.lifts && !card.row.not_shared;
      ctx.mount_keyed::<CardView>(
        &format!("task-card-{}-{}", card.row.key, place.column.name()),
        CardProps {
          card: card.clone(),
          display: shared.display.clone(),
          look,
          place: place.clone(),
          lifts,
          menu: shared.menus && !card.row.not_shared,
        },
      )
    })
    .collect()
}

/// The cards with the lifted card's slot where it would go, in task number order, and Done's own slots.
fn slots(
  ctx: &mut Ctx,
  state: &BoardState,
  (shared, target): (&Shared<'_>, &Target),
  cards: &[super::card::CardModel],
  drag: &Drag,
) -> Vec<Element> {
  if target.column == ColumnKey::Status(WorkflowStatus::Done) {
    let done = drop_slot(ctx, "tasks-board-slot-done", (Icon::CircleCheck, PaletteColor::Success), "task_close.menu.done");
    let mut parts = vec![done.into()];
    parts.extend(card_views(ctx, shared, (target, cards)));
    if !shared.display.show_canceled {
      let zone = Target {
        column: ColumnKey::Status(WorkflowStatus::Canceled),
        lane: target.lane.clone(),
      };
      let canceled = drop_slot(ctx, "tasks-board-slot-canceled", (Icon::CircleX, PaletteColor::TextMuted), "task_close.menu.cancel")
        .ref_element(state.target_ref(zone));
      parts.push(canceled.into());
    }
    return parts;
  }
  let number = key_number(&drag.card.row.key);
  let at = cards
    .iter()
    .position(|card| key_number(&card.row.key) > number)
    .unwrap_or(cards.len());
  let mut parts = card_views(ctx, shared, (target, cards));
  parts.insert(
    at,
    Column::new()
      .id("tasks-board-slot")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(drag.size.1))
      .rounded(RadiusSize::Md)
      .background(crate::theme::ExtraColor::Hover.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
      .into(),
  );
  parts
}

/// Done's slot *Mark as done* or its zone *Cancel task* (`reJKk`, `dYlr8`).
fn drop_slot(ctx: &Ctx, id: &str, (glyph, color): (Icon, PaletteColor), key: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SLOT_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(crate::theme::ExtraColor::Hover.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
    .child(icon(glyph, IconSize::Base, color))
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
}

/// The number of a task's key (`KON-12` is 12), its order in a column.
fn key_number(key: &str) -> u64 {
  key
    .rsplit_once('-')
    .and_then(|(_, number)| number.parse().ok())
    .unwrap_or(u64::MAX)
}

/// The column's head: its look, name and counts, Fold, and Backlog's Add a draft.
pub(super) fn head(ctx: &mut Ctx, (state, shared): (&BoardState, &Shared<'_>), column: &ColumnModel, id: &str) -> Row {
  let name = column.key.name();
  let fold = IconButton::new(
    format!("{id}-fold"),
    Icon::FoldHorizontal,
    ctx.t_args("tasks.board.column.fold", [("column", &*column.label)]),
  )
  .small()
  .on_click(fold_toggle(state, &name));
  // A locked quick add says why as its label, which its tooltip shows (`Add task` in `15 Not a member` `kMKKv`).
  let add = (lane_is_backlog(column) && shared.quick_add.is_some()).then(|| {
    match shared.quick_add.clone().and_then(Result::err) {
      Some(reason) => IconButton::new(format!("{id}-add"), Icon::Plus, reason).small().render(ctx),
      None => {
        let open = state.quick_open.clone();
        IconButton::new(format!("{id}-add"), Icon::Plus, ctx.t("tasks.board.quickAdd.open"))
          .small()
          .on_click(move || open.set(true))
          .render(ctx)
      }
    }
  });
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::new().left(SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(column.glyph, IconSize::Base, column.color.clone()))
    .child(
      Text::new(&column.label)
        .id(format!("{id}-name"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(
      Text::new(&column.cards.len().to_string())
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .with_children((column.needs_you > 0).then(|| needs_you(ctx, column.needs_you, &format!("{id}-needs-you"))))
    .child(Spacer::new().flex(1.0))
    .child(fold.render(ctx))
    .with_children(add)
}

/// How many cards need the person: an 11 `warning` hand and the count (`Needs you` `UnCKz`).
pub fn needs_you(ctx: &Ctx, count: usize, id: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Fine.role())
    .align_items(Alignment::Center)
    .describe(
      "a11y_name",
      ctx.t_args("tasks.board.column.needsYou", [("count", count.to_string())]),
    )
    .child(icon(Icon::Hand, IconSize::Xs, PaletteColor::Warning))
    .child(
      Text::new(&count.to_string())
        .variant(crate::theme::ExtraTypography::Note.role())
        .color(PaletteColor::Warning)
        .nowrap(),
    )
}

/// Whether `column` is Backlog, where quick add makes drafts.
fn lane_is_backlog(column: &ColumnModel) -> bool {
  column.key == ColumnKey::Status(WorkflowStatus::Backlog)
}

/// Backlog's foot: the quick add field while it is open, else *New task*, which opens it; locked like the head's.
fn foot(ctx: &mut Ctx, shared: &Shared<'_>, column: &ColumnModel, lane: Option<&str>) -> Option<Element> {
  if !lane_is_backlog(column) || lane.is_some() {
    return None;
  }
  let allowed = shared.quick_add.clone()?;
  Some(ctx.mount_keyed::<QuickAdd>(
    "tasks-board-quick-add",
    QuickAddProps {
      project: shared.project.clone(),
      locked: allowed.err(),
    },
  ))
}

/// Why a column shows no card: none of the view's tasks is in it, or the search and filters hide them.
fn empty(ctx: &Ctx, shared: &Shared<'_>, column: &ColumnModel) -> Column {
  let id = format!("tasks-board-column-{}-empty", column.key.name());
  let (glyph, title, detail) = if shared.filtered && column.total > 0 {
    let key = if column.total == 1 {
      "tasks.board.column.noMatch.one"
    } else {
      "tasks.board.column.noMatch.other"
    };
    (
      Icon::SearchX,
      ctx.t("tasks.board.column.noMatch.title"),
      ctx.t_args(key, [("column", column.label.to_string()), ("count", column.total.to_string())]),
    )
  } else {
    (
      Icon::Inbox,
      ctx.t("tasks.board.column.empty.title"),
      ctx.t_args("tasks.board.column.empty.detail", [("column", &*column.label)]),
    )
  };
  let clear = (shared.filtered && column.total > 0).then(|| {
    let clear = shared.clear.clone();
    pressable(move || clear())
      .id(format!("{id}-clear"))
      .child(
        Text::new(&ctx.t("tasks.board.column.noMatch.clear"))
          .variant(TypographyStyle::Description)
          .nowrap(),
      )
  });
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), ExtraSpacing::SettingsHead.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Lg, PaletteColor::TextMuted))
    .child(
      Text::new(&title)
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .width(Dimension::Pct(100.0))
        .text_align(lurq::layout::text_style::TextAlign::Center),
    )
    .child(
      Text::new(&detail)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0))
        .text_align(lurq::layout::text_style::TextAlign::Center),
    )
    .with_children(clear)
}

/// What folds the column `name` to a strip, or unfolds it, kept for the project.
fn fold_toggle(state: &BoardState, name: &str) -> impl Fn() + Send + Sync + 'static {
  let (display, name) = (state.display.clone(), name.to_owned());
  move || display.update(|display| display.toggle_collapsed(&name))
}

/// A folded column: a 40 wide strip, `surface_panel` in a `border` stroke, radius 6, 6 over and under its parts, 10
/// between them: Unfold, its icon, its count and its name turned on its side.
pub fn strip(ctx: &mut Ctx, state: &BoardState, column: &ColumnModel, stroke: PaletteColor) -> Element {
  let name = column.key.name();
  let id = format!("tasks-board-column-{name}");
  let unfold = IconButton::new(
    format!("{id}-unfold"),
    Icon::UnfoldHorizontal,
    ctx.t_args("tasks.board.column.unfold", [("column", &*column.label)]),
  )
  .small()
  .on_click(fold_toggle(state, &name));
  // lurq turns a node about its centre and lays it out unturned: the name gets a room as tall as a long name is wide.
  let label = Column::new()
    .width(Dimension::Px(STRIP_WIDTH))
    .height(Dimension::Px(STRIP_LABEL_ROOM))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(
      Text::new(&column.label)
        .id(format!("{id}-name"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .transform(Transform2D::rotate_deg(90.0)),
    );
  Column::new()
    .id(id.clone())
    .width(Dimension::Px(STRIP_WIDTH))
    .flex_shrink(0.0)
    .height(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .justify(Justify::Start)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, stroke))
    .describe("state", "folded")
    .ref_element(state.target_ref(Target {
      column: column.key.clone(),
      lane: None,
    }))
    .child(unfold.render(ctx))
    .child(icon(column.glyph, IconSize::Base, column.color.clone()))
    .child(
      Text::new(&column.cards.len().to_string())
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .child(label)
    .into()
}
