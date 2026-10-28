//! The board's questions, asked at the card and never in the board's layout, so nothing on it moves (`Popover · stop
//! run` `XdfNz` in `04 Confirm in place` `ksvIz`, and the variants of `12 Moves and confirmations` `aUNO8`, `JUaF2`): a
//! popover 340 wide under the card, its right edge on the column's (its left in the first column), flipped to stay in
//! the window. A move waits in its target column as the card at 50 % in a `border_focus` stroke (`KON-16` `y7FQj9`),
//! its place in its own column kept as an outline; a question about a card that stays (*Delete KON-25?*) is asked at
//! the card where it is, and one about the selected cards over the selection bar. The popover is `surface_raised` in a
//! `border_strong` stroke, radius 10, 12 over and under its parts and 14 at its sides, 10 between them, lifted by the
//! menu's shadow: a 15 mark, the question in 13/600 and what it does in 12.5 `text_secondary`; *Enter confirms · Esc
//! keeps it* 25 in; and its buttons at its end, two or, for Run again, three (ORC-144: *Don't move*, *Move to Todo*,
//! *Run again*; the design draws two, so the third takes the same buttons' styles). Esc, a press outside or the keep
//! button put everything back; Enter confirms.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::TaskId, task::organization::WorkflowStatus};
use orchester_workspace::tasks::close::Closing;

use super::{
  actions::{Ask, BoardState},
  card::CardModel,
  model::ColumnKey,
};
use crate::{
  screens::task_close::{self, Closer, Subject},
  theme::{ExtraSpacing, ExtraTypography, IconSize, TasksSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    kbd::key_cap,
    message::message_text,
    popover::{PopoverSide, popover, popover_panel},
  },
};

/// A question's width (`XdfNz`).
pub const WIDTH: f32 = 340.0;

/// Where the board asks what it asks now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asking {
  /// At the card of `task`: in the column `to` the move would put it in, where it waits while the question is open;
  /// where it is when the move goes to no column the board shows, or the task stays.
  Card { task: TaskId, to: Option<ColumnKey> },
  /// Over the selection bar: a question about the selected cards.
  Bulk,
}

/// The card a question is asked at, and the column it waits in.
#[derive(Clone, Debug, PartialEq)]
pub struct AskedCard {
  pub card: CardModel,
  pub to: Option<ColumnKey>,
}

/// What the board asks now, and where: the closer's stop and delete questions first, then the board's own. `shows`
/// says whether a column is on the board, unfolded, where a card can wait.
pub fn asking(state: &BoardState, closer: &Closer, shows: &dyn Fn(&ColumnKey) -> bool) -> Option<Asking> {
  let at = |task: TaskId, status: WorkflowStatus| {
    let to = ColumnKey::Status(status);
    Asking::Card {
      task,
      to: shows(&to).then_some(to),
    }
  };
  if let Some((task, closing)) = closer.stop_asked() {
    // A cancel waits in Canceled while it shows, else in Done, whose *Cancel task* zone took it (`JUaF2`).
    let canceled = ColumnKey::Status(WorkflowStatus::Canceled);
    return Some(match closing {
      Closing::Canceled if shows(&canceled) => Asking::Card {
        task: task.id,
        to: Some(canceled),
      },
      Closing::Done | Closing::Canceled => at(task.id, WorkflowStatus::Done),
    });
  }
  if let Some(task) = closer.delete_asked() {
    return Some(Asking::Card {
      task: task.id,
      to: None,
    });
  }
  Some(match state.asked.get()? {
    // A card in Todo already (Run again from its menu) is asked where it is.
    Ask::RunAgain { task, .. } if task.status == WorkflowStatus::Todo => Asking::Card {
      task: task.id,
      to: None,
    },
    Ask::StartsNow { task, .. } | Ask::RunAgain { task, .. } => at(task.id, WorkflowStatus::Todo),
    Ask::Unmerged(task) => at(task.id, WorkflowStatus::Done),
    Ask::BulkQueue { .. }
    | Ask::BulkRunAgain { .. }
    | Ask::BulkUnmerged { .. }
    | Ask::BulkClose { .. }
    | Ask::BulkDelete(_) => Asking::Bulk,
  })
}

/// The panel of what is asked now: the closer's questions, then the board's own.
pub fn panel(ctx: &mut Ctx, (state, closer): (&BoardState, &Closer)) -> Option<Element> {
  if let Some((task, closing)) = closer.stop_asked() {
    return Some(stop(ctx, closer, (&task, closing)).into());
  }
  if let Some(task) = closer.delete_asked() {
    return Some(task_close::confirmation_in(
      ctx,
      closer,
      &task,
      ("tasks-close", Some(WIDTH)),
    ));
  }
  let asked = state.asked.get()?;
  Some(super::ask::ask_panel(ctx, (state, closer), &asked).into())
}

/// `panel` in its popover: at the card for a card's question, over the selection bar for the selected cards'. Under a
/// card it ends at the card's end, or, for a card in the board's first column (`first`), starts at its start, so it
/// never covers the sidebar.
pub fn at_anchor(state: &BoardState, (asking, first): (&Asking, bool), panel: Element) -> Element {
  let (anchor, side) = match asking {
    Asking::Card { .. } if first => (state.question_anchor.clone(), PopoverSide::Below),
    Asking::Card { .. } => (state.question_anchor.clone(), PopoverSide::BelowEnd),
    Asking::Bulk => (state.bulk_anchor.clone(), PopoverSide::Above),
  };
  popover(anchor, state.question_open.clone(), side, panel)
    .switch_on_anchor_press(false)
    .into()
}

/// What a question shows: its mark in its tone, its words, what Enter does and what Esc keeps, what more it says (why
/// it failed), and its buttons: keep, a third choice between them when the question has one, and confirm. Its parts'
/// ids start with `id`.
pub struct Parts {
  pub id: &'static str,
  pub glyph: Icon,
  pub tone: PaletteColor,
  pub title: Arc<str>,
  pub detail: Arc<str>,
  pub confirms: Arc<str>,
  pub keeps: Arc<str>,
  pub more: Vec<Element>,
  pub keep: Element,
  pub between: Option<Element>,
  pub confirm: Element,
}

/// A question's popover panel.
pub fn question(ctx: &Ctx, parts: Parts) -> Column {
  let id = parts.id;
  let keys = Row::new()
    .padding(Padding::new().left(TasksSpacing::QuestionKeys.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(key_cap(&format!("{id}-enter"), &ctx.t("tasks.board.keys.enter")))
    .child(note(&parts.confirms))
    .child(key_cap(&format!("{id}-esc"), &ctx.t("tasks.board.keys.esc")))
    .child(note(&parts.keeps));
  popover_panel(id, WIDTH)
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .box_shadow(ShadowStyle::Md)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Start)
        .child(icon(parts.glyph, IconSize::Nav, parts.tone))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .spacing(SpacingSize::Xs)
            .child(
              Text::new(&parts.title)
                .id(format!("{id}-title"))
                .variant(ExtraTypography::FormSection.role())
                .width(Dimension::Pct(100.0)),
            )
            .child(
              Text::new(&parts.detail)
                .id(format!("{id}-text"))
                .variant(ExtraTypography::Lead.role())
                .width(Dimension::Pct(100.0)),
            ),
        ),
    )
    .with_children(parts.more)
    .child(keys)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Spacer::new().flex(1.0))
        .child(parts.keep)
        .with_children(parts.between)
        .child(parts.confirm),
    )
}

/// *Stop the run of KON-16?* with *Keep it running* and *Stop run and mark as done* (or *and cancel task*), and why
/// stopping it failed.
fn stop(ctx: &Ctx, closer: &Closer, (task, closing): (&Subject, Closing)) -> Column {
  let pending = closer.busy();
  let key = &*task.key;
  let (explanation, confirm) = match closing {
    Closing::Done => ("task_close.stop.done", "task_close.stop.confirmDone"),
    Closing::Canceled => ("task_close.stop.canceled", "task_close.stop.confirmCanceled"),
  };
  let failed = closer
    .failure_of(task.id)
    .map(|message| field_error("tasks-close-stop-error", &message_text(ctx, &message)).into());
  let keep = ActionButton::new(
    "tasks-close-stop-keep",
    ButtonKind::Ghost,
    ctx.t("task_close.stop.keep"),
  )
  .in_form();
  let keep = if pending {
    keep.disabled()
  } else {
    let closer = closer.clone();
    keep.on_click(move || closer.keep())
  };
  let stop = ActionButton::new("tasks-close-stop-confirm", ButtonKind::Danger, ctx.t(confirm))
    .icon(Icon::CircleStop)
    .in_form();
  let stop = if pending {
    stop.loading()
  } else {
    let closer = closer.clone();
    stop.on_click(move || {
      closer.confirm_stop();
    })
  };
  question(
    ctx,
    Parts {
      id: "tasks-close-stop",
      glyph: Icon::CircleStop,
      tone: PaletteColor::Warning,
      title: ctx.t_args("task_close.stop.title", [("task", key)]),
      detail: ctx.t_args(explanation, [("task", key)]),
      confirms: ctx.t("tasks.board.ask.confirms"),
      keeps: ctx.t("tasks.board.ask.keepsRunning"),
      more: failed.into_iter().collect(),
      keep: keep.into(),
      between: None,
      confirm: stop.into(),
    },
  )
}

fn note(words: &str) -> Text {
  Text::new(words).variant(ExtraTypography::Note.role()).nowrap()
}
