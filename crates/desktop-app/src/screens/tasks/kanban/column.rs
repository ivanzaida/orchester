//! A column of the board (`Column · Backlog` `v6Z67H` in `01 Board` `ntozw`): its head, 28 high (the status's 13 icon
//! in its tone, its name in 12/500 `text_secondary`, its count in 12 `text_muted`, how many of its cards need the
//! person as an 11 `warning` hand and count, then Fold and, in Backlog alone, Add a draft), and its cards 6 apart.
//! The cards do not scroll in their column: on the board without rows they scroll together with every other column's
//! in the board's one scroll, under the heads, which stay (`columns`); with rows, the rows scroll together (`lanes`).
//! Nothing a drag or a question shows moves a card: while a lifted card is over a column that takes it, the column is
//! lit and a line shows where the card would go (`02 Dragging` `goJvp`); what refuses it, and Done's *Mark as done*
//! and *Cancel task*, are drawn in a layer over the cards (`overlay`, `refusal`, `03 Drop refused` `bR7zd`). A move
//! that waits for the person's answer shows its card in the column it would go to, at its top (`KON-16` `y7FQj9` in
//! `04 Confirm in place` `ksvIz`), where the question is asked (`question`). Without cards it says why (`Empty
//! column, no filter` `B8kYj`, `Search hides every card` `KhgzA`). A folded column is a 40 wide strip with its icon,
//! count and name (`Collapsed · Backlog` `F7EfQ`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Stack, Text},
  layout::{Alignment, StackAlignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding, transform::Transform2D},
};
use orchester_core::task::organization::WorkflowStatus;

use super::{
  actions::{BoardState, Drag, Focused, Refused, Selected, Target},
  card::{CardLook, CardModel},
  card_view::{CardProps, CardView},
  display::Display,
  model::{ColumnKey, ColumnModel},
  overlay::{self, Insertion, Lit},
  question::AskedCard,
  quick_add::{QuickAdd, QuickAddProps},
};
use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    pressable::pressable,
  },
};

/// A column's head (`Header` `Ob9rV`), and a folded column's width (`Collapsed · Backlog` `F7EfQ`).
pub(super) const HEAD_HEIGHT: f32 = 28.0;
pub(super) const STRIP_WIDTH: f32 = 40.0;
/// The room of a folded column's name, turned on its side.
const STRIP_LABEL_ROOM: f32 = 96.0;

/// What every column of a render shares.
pub struct Shared<'a> {
  pub display: &'a Display,
  pub selected: &'a Selected,
  pub focused: Focused,
  pub drag: Option<&'a Drag>,
  pub refused: Option<&'a Refused>,
  /// The card a question is asked at, and the column its move waits in.
  pub asked: Option<&'a AskedCard>,
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

/// `column` holding `cards` in row `lane` of the board with rows, where the row's head and the board's column heads
/// say what it is.
pub fn column(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  (column, cards): (&ColumnModel, &[CardModel]),
  lane: &str,
) -> Element {
  let target = Target {
    column: column.key.clone(),
    lane: Some(lane.to_owned()),
  };
  let name = column.key.name();
  let id = format!("tasks-board-column-{name}-{lane}");
  let feedback = overlay::feedback(shared, (&target, cards));
  let body = body(ctx, (state, shared), (column, cards), (&target, feedback.insertion));
  // The layer's own drop targets (Done's *Cancel task*) go before the column's, so a drop over them takes them.
  let layer = feedback
    .layer
    .as_ref()
    .map(|layer| overlay::layer(ctx, state, &id, layer));
  let cards = Stack::new()
    .width(Dimension::Pct(100.0))
    .stack_align(StackAlignment::TopStart)
    .child(body)
    .with_children(layer);
  lit(frame(state, (&id, name), target).child(cards), feedback.lit).into()
}

/// A column of the board without rows, in the board's three layers (`columns`).
pub struct Parts {
  /// Its head, lit while a lifted card is over it: the column's drop target over its whole visible height.
  pub frame: Element,
  /// Its cards, which scroll with every other column's.
  pub cards: Element,
  /// What a drag or a refusal draws over its cards, over the part of them that shows.
  pub over: Option<Element>,
}

/// `column` holding `cards` on the board without rows, in its layers.
pub fn parts(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  (column, cards): (&ColumnModel, &[CardModel]),
) -> Parts {
  let target = Target {
    column: column.key.clone(),
    lane: None,
  };
  let name = column.key.name();
  let id = format!("tasks-board-column-{name}");
  let feedback = overlay::feedback(shared, (&target, cards));
  let body = body(ctx, (state, shared), (column, cards), (&target, feedback.insertion));
  // The layer's own drop targets (Done's *Cancel task*) go before the column's, so a drop over them takes them.
  let over = feedback
    .layer
    .as_ref()
    .map(|layer| overlay::layer(ctx, state, &id, layer));
  let frame = frame(state, (&id, name), target)
    .height(Dimension::Pct(100.0))
    .child(head(ctx, (state, shared), column, (&id, HeadIn::Column)));
  Parts {
    frame: lit(frame, feedback.lit).into(),
    cards: Column::new()
      .id(format!("{id}-cards"))
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .child(body)
      .into(),
    over,
  }
}

/// The column `id`'s frame, the drop target of `target`.
fn frame(state: &BoardState, (id, name): (&str, String), target: Target) -> Column {
  Column::new()
    .id(id.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .describe("column", name)
    .ref_element(state.target_ref(target))
}

/// The column's cards (first, the card whose move to it waits for the person's answer), the insertion line among
/// them while a lifted card is over it, and Backlog's foot; why it shows none, without them.
fn body(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  (column, cards): (&ColumnModel, &[CardModel]),
  (target, insertion): (&Target, Option<Insertion>),
) -> Column {
  let waiting = shared
    .asked
    .filter(|asked| target.lane.is_none() && asked.to.as_ref() == Some(&column.key));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .with_children(waiting.map(|asked| waiting_card(ctx, (state, shared), asked, target)))
    .with_children(card_views(ctx, (state, shared), (target, cards), insertion))
    .with_children(foot(ctx, shared, column, target.lane.as_deref()))
    .with_children((cards.is_empty() && waiting.is_none() && target.lane.is_none()).then(|| empty(ctx, shared, column)))
}

/// A column the drag is over, in its tone: filled, in a stroke. Its cards keep their width.
fn lit(frame: Column, lit: Option<Lit>) -> Column {
  let (fill, stroke, drop) = match lit {
    Some(Lit::Refuses) => (PaletteColor::DangerMuted, PaletteColor::Danger, "refused"),
    Some(Lit::Takes) => (PaletteColor::AccentMuted, PaletteColor::BorderStrong, "takes"),
    None => return frame,
  };
  frame
    .background(fill)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Lg)
    .describe("drop", drop)
}

/// The cards of a column, each mounted as itself, the line where a lifted card would go among them. The card a
/// question is asked at where it is holds the question's anchor; one whose move waits elsewhere leaves its place as
/// an outline.
fn card_views(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  (place, cards): (&Target, &[CardModel]),
  insertion: Option<Insertion>,
) -> Vec<Element> {
  let last = cards.len().saturating_sub(1);
  cards
    .iter()
    .enumerate()
    .map(|(at, card)| {
      let id = card.row.id;
      let asked = shared.asked.filter(|asked| asked.card.row.id == id);
      let look = CardLook {
        selected: shared.selected.0.contains(&id),
        selecting: !shared.selected.0.is_empty(),
        lifted: shared.drag.is_some_and(|drag| drag.card.row.id == id) || asked.is_some_and(|asked| asked.to.is_some()),
        waiting: asked.is_some_and(|asked| asked.to.is_none()),
        focused: shared.focused.0 == Some(id),
        acting: false,
      };
      // A task kept Not shared, or a list that is no longer the person's, does not lift.
      let lifts = shared.lifts && !card.row.not_shared;
      let view = ctx.mount_keyed::<CardView>(
        &format!("task-card-{}-{}", card.row.key, place.column.name()),
        CardProps {
          card: card.clone(),
          display: shared.display.clone(),
          look,
          place: place.clone(),
          lifts,
          menu: shared.menus && !card.row.not_shared,
          pending: false,
        },
      );
      let view = if look.waiting { anchored(state, view) } else { view };
      match insertion {
        Some(Insertion::Before(before)) if before == at => overlay::with_line(view, true),
        Some(Insertion::After) if at == last => overlay::with_line(view, false),
        _ => view,
      }
    })
    .collect()
}

/// The card whose move to this column waits for the person's answer, at the column's top, holding the question's
/// anchor: it neither lifts nor opens, and has no menu.
fn waiting_card(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  asked: &AskedCard,
  place: &Target,
) -> Element {
  let look = CardLook {
    waiting: true,
    ..CardLook::default()
  };
  let view = ctx.mount_keyed::<CardView>(
    &format!("task-card-{}-{}-waiting", asked.card.row.key, place.column.name()),
    CardProps {
      card: asked.card.clone(),
      display: shared.display.clone(),
      look,
      place: place.clone(),
      lifts: false,
      menu: false,
      pending: true,
    },
  );
  anchored(state, view)
}

/// `view` holding the anchor the board's question opens under.
fn anchored(state: &BoardState, view: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .ref_element(state.question_anchor.clone())
    .child(view)
    .into()
}

/// Where a column's head is: over its own cards, or over the board's rows, where columns do not fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HeadIn {
  Column,
  Lanes,
}

/// The column's head: its look, name and counts, Fold over its own cards (not over rows, `Heads` in `10 Swimlanes`
/// `i96kI`), and Backlog's Add a draft.
pub(super) fn head(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  column: &ColumnModel,
  (id, place): (&str, HeadIn),
) -> Row {
  let name = column.key.name();
  let fold = (place == HeadIn::Column).then(|| {
    IconButton::new(
      format!("{id}-fold"),
      Icon::FoldHorizontal,
      ctx.t_args("tasks.board.column.fold", [("column", &*column.label)]),
    )
    .small()
    .on_click(fold_toggle(state, &name))
    .render(ctx)
  });
  // A locked quick add says why as its label, which its tooltip shows (`Add task` in `15 Not a member` `kMKKv`).
  let add = (lane_is_backlog(column) && shared.quick_add.is_some()).then(|| {
    match shared.quick_add.clone().and_then(Result::err) {
      Some(reason) => IconButton::new(format!("{id}-add"), Icon::Plus, reason)
        .small()
        .render(ctx),
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
    .with_children(fold)
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
      ctx.t_args(
        key,
        [
          ("column", column.label.to_string()),
          ("count", column.total.to_string()),
        ],
      ),
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
    pressable(move || clear()).id(format!("{id}-clear")).child(
      Text::new(&ctx.t("tasks.board.column.noMatch.clear"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
  });
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::SettingsHead.role(),
    ))
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
