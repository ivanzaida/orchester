//! A run of a lane (`Row · KON-9` `mTyPp` in 05 `S1pTL`): its place in its lane, its state's 16 glyph, its task's key
//! and title with its run, the line that says where it stands, since when, its action and the chevron that opens its
//! task. 60 high, 16 at its sides, 14 between its parts, over a `border` line but for the lane's last row. Under the
//! state line, the files a dirty checkout holds or why the run left. A waiting run moves up, to the top or out of the
//! queue with small buttons; a run waiting for approval opens its review; a blocked one has *Retry*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  merge_queue::{Blocked, QueueState},
};
use orchester_workspace::{
  display::{clock, short_revision},
  merge_queue::{Move, QueueRow},
};

use super::{actions::QueueActions, words};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  theme::{DeliverySpacing, DeliveryText, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// A row's height while it says no more than its state line (`mTyPp`).
const ROW_HEIGHT: f32 = 60.0;
/// The room of a row's place, so the glyphs of a lane line up (`Place` `AiuNZ`).
const PLACE_WIDTH: f32 = 8.0;

/// What every row of the queue shares: its project, the person's checkout, and what its buttons do.
#[derive(Clone)]
pub(super) struct Rows {
  pub project: ProjectId,
  pub checkout: Arc<str>,
  pub actions: QueueActions,
}

/// Where a row is in its lane.
pub(super) struct Placed<'a> {
  /// Its place among the lane's runs in the queue, from 1; none for a run merged or out of it.
  pub place: Option<usize>,
  /// The task key of the lane's run before it in the queue.
  pub after: Option<&'a str>,
  /// Whether the lane's run being delivered is blocked, which holds the runs behind it.
  pub held: bool,
  /// Whether it is the lane's last row, with no line under it.
  pub last: bool,
}

/// `row` of the queue, at `placed` in its lane.
pub(super) fn row(ctx: &mut Ctx, rows: &Rows, row: &QueueRow, placed: &Placed<'_>) -> Element {
  let id = format!("merge-queue-row-{}", row.run_number);
  let look = look(ctx, row, placed);
  let mut shown = Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Sm))
    .spacing(DeliverySpacing::LaneRow.role())
    .align_items(Alignment::Center)
    .describe("state", state_name(&row.entry.state))
    .child(
      Text::new(&placed.place.map(|place| place.to_string()).unwrap_or_default())
        .id(format!("{id}-place"))
        .variant(DeliveryText::Count.role())
        .min_width(Dimension::Px(PLACE_WIDTH))
        .nowrap(),
    )
    .child(icon(look.icon, IconSize::Lg, look.color.clone()).id(format!("{id}-glyph")))
    .child(words_column(ctx, &id, rows, row, &look))
    .child(
      Text::new(&look.since)
        .id(format!("{id}-since"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .with_children(action(ctx, &id, rows, row))
    .child(open_task(ctx, &id, rows, row));
  if !placed.last {
    shown = shown.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  shown.into()
}

/// How a row shows its state: its glyph, its line and their color, and since when.
struct Look {
  icon: Icon,
  /// The glyph's color; the line takes it too unless it is a waiting run's.
  color: PaletteColor,
  line: Arc<str>,
  line_color: PaletteColor,
  since: Arc<str>,
}

fn look(ctx: &Ctx, row: &QueueRow, placed: &Placed<'_>) -> Look {
  let base = words::base(row);
  let since = ctx.t_args("delivery.queue.since", [("time", clock(row.entry.since))]);
  let tinted = |icon, color: PaletteColor, line: Arc<str>| Look {
    icon,
    color: color.clone(),
    line,
    line_color: color,
    since: since.clone(),
  };
  let delivering = |line| tinted(Icon::LoaderCircle, PaletteColor::Info, line);
  match &row.entry.state {
    QueueState::Syncing => delivering(ctx.t_args("delivery.queue.lane.syncing", [("base", base)])),
    QueueState::Rechecking { .. } => delivering(ctx.t("delivery.queue.lane.rechecking")),
    QueueState::Integrating { .. } => delivering(ctx.t("delivery.queue.lane.integrating")),
    QueueState::Queued | QueueState::Removed => Look {
      icon: Icon::Hourglass,
      color: PaletteColor::TextMuted,
      line: waiting(ctx, placed),
      line_color: PaletteColor::TextSecondary,
      since: since.clone(),
    },
    QueueState::WaitingForApproval => tinted(Icon::CirclePause, PaletteColor::Warning, approval(ctx, row)),
    QueueState::Blocked(blocked) => tinted(
      Icon::CirclePause,
      PaletteColor::Warning,
      blocked_line(ctx, blocked, base),
    ),
    QueueState::Merged(delivered) => Look {
      icon: Icon::GitMerge,
      color: PaletteColor::TextMuted,
      line: ctx.t_args(
        "delivery.queue.lane.merged",
        [
          ("base", base),
          ("revision", short_revision(delivered.revision.as_str())),
        ],
      ),
      line_color: PaletteColor::TextSecondary,
      since: at(ctx, row),
    },
    QueueState::Left(left) => {
      let reason = words::left_reason(ctx, left);
      let (icon, color, key) = if left.failed() {
        (Icon::CircleX, PaletteColor::Danger, "delivery.queue.lane.left")
      } else {
        (Icon::Hand, PaletteColor::Warning, "delivery.queue.lane.needsYou")
      };
      Look {
        since: at(ctx, row),
        ..tinted(icon, color, ctx.t_args(key, [("reason", reason)]))
      }
    }
  }
}

/// When a run was merged or left.
fn at(ctx: &Ctx, row: &QueueRow) -> Arc<str> {
  ctx.t_args("delivery.queue.lane.at", [("time", clock(row.entry.since))])
}

/// A waiting run: held behind a blocked one, next, or after the run before it.
fn waiting(ctx: &Ctx, placed: &Placed<'_>) -> Arc<str> {
  match placed.after {
    _ if placed.held => ctx.t("delivery.queue.blocked.held"),
    Some(key) => ctx.t_args("delivery.queue.lane.waiting", [("key", key)]),
    None => ctx.t("delivery.queue.lane.next"),
  }
}

/// A run waiting for approval: where it was checked, or that its base moved since.
fn approval(ctx: &Ctx, row: &QueueRow) -> Arc<str> {
  let base = words::base(row);
  match &row.entry.checked {
    _ if row.base_moved => ctx.t_args("delivery.queue.lane.approvalMoved", [("base", base)]),
    Some(checked) => ctx.t_args(
      "delivery.queue.lane.approvalChecked",
      [("base", base), ("revision", short_revision(checked.base.as_str()))],
    ),
    None => ctx.t("delivery.queue.lane.approval"),
  }
}

fn blocked_line(ctx: &Ctx, blocked: &Blocked, base: &str) -> Arc<str> {
  match blocked {
    Blocked::Uncommitted { .. } => ctx.t("delivery.queue.lane.blockedUncommitted"),
    Blocked::OtherBranch { .. } => ctx.t_args("delivery.queue.lane.blockedOtherBranch", [("base", base)]),
    Blocked::InProgress => ctx.t("delivery.queue.lane.blockedInProgress"),
    Blocked::HeldLocally => ctx.t("delivery.queue.lane.blockedHeld"),
  }
}

/// The task's key, title and run (`Title line` `ARu1s`), and under them the state line; then the files a dirty
/// checkout holds.
fn words_column(ctx: &Ctx, id: &str, rows: &Rows, row: &QueueRow, look: &Look) -> Column {
  let run = ctx.t_args("delivery.queue.lane.run", [("run", row.run_number.to_string())]);
  let title = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .min_width(Dimension::Px(0.0))
    .child(
      Text::new(&row.task_key.to_string())
        .variant(DeliveryText::Count.role())
        .nowrap(),
    )
    .child(
      Text::new(&row.task_title)
        .id(format!("{id}-title"))
        .variant(ExtraTypography::Strong.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&run)
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  Column::new()
    .id(format!("{id}-run"))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(title)
    .child(
      Text::new(&look.line)
        .id(format!("{id}-state-line"))
        .variant(ExtraTypography::Property.role())
        .color(look.line_color.clone()),
    )
    .with_children(uncommitted(ctx, id, rows, row))
}

/// The files a dirty checkout holds, under a blocked run's state line.
fn uncommitted(ctx: &Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Option<Element> {
  let QueueState::Blocked(Blocked::Uncommitted { files }) = &row.entry.state else {
    return None;
  };
  let label = ctx.t_args("delivery.queue.blocked.uncommittedIn", [("path", &*rows.checkout)]);
  Some(
    Row::new()
      .id(format!("{id}-uncommitted"))
      .spacing(SpacingSize::Sm)
      .wrap()
      .child(Text::new(&label).variant(ExtraTypography::Note.role()))
      .with_children(
        files
          .iter()
          .map(|file| Text::new(file).variant(DeliveryText::File.role())),
      )
      .into(),
  )
}

fn task_page(ctx: &mut Ctx, rows: &Rows, row: &QueueRow) -> crate::navigation::link::Open {
  Navigation::of(ctx).to(Route::Project(
    rows.project,
    ProjectPage::Task(row.task_key.clone(), TaskView::Summary),
  ))
}

/// The chevron that opens the run's task (`J7blWp`).
fn open_task(ctx: &mut Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Element {
  let page = task_page(ctx, rows, row);
  IconButton::new(
    format!("{id}-open-task"),
    Icon::ChevronRight,
    ctx.t("delivery.queue.lane.openTask"),
  )
  .compact()
  .opens(page)
  .render(ctx)
}

/// What the person does with the run where it stands: move or remove a waiting run, review one waiting for approval,
/// retry a blocked one.
fn action(ctx: &mut Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Vec<Element> {
  match &row.entry.state {
    QueueState::Queued => waiting_buttons(ctx, id, rows, row),
    QueueState::WaitingForApproval => {
      let page = task_page(ctx, rows, row);
      vec![
        ActionButton::new(
          format!("{id}-review"),
          ButtonKind::Secondary,
          ctx.t("delivery.queue.action.reviewAndMerge"),
        )
        .icon(Icon::Check)
        .small()
        .opens(page)
        .into(),
      ]
    }
    QueueState::Blocked(_) => vec![
      rows
        .actions
        .retries(
          ActionButton::new(
            format!("{id}-retry"),
            ButtonKind::Secondary,
            ctx.t("delivery.queue.action.retry"),
          )
          .icon(Icon::RotateCcw)
          .small(),
        )
        .into(),
    ],
    _ => Vec::new(),
  }
}

/// *Move up*, *Move to top* and *Remove from the queue* as small glyphs, their words in their tooltips; the first
/// waiting run of its lane moves nowhere.
fn waiting_buttons(ctx: &mut Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Vec<Element> {
  let run = row.entry.run;
  let first = row.place == Some(1);
  let mut moving = |(suffix, glyph, key, how): (&str, Icon, &str, Move)| {
    let button = IconButton::new(format!("{id}-{suffix}"), glyph, ctx.t(key)).compact();
    let button = if first {
      button
    } else {
      let actions = rows.actions.clone();
      button.on_click(move || actions.move_run(run, how))
    };
    button.render(ctx)
  };
  let up = moving(("move-up", Icon::ArrowUp, "delivery.queue.lane.moveUp", Move::Up));
  let top = moving((
    "move-to-top",
    Icon::ArrowUpToLine,
    "delivery.queue.lane.moveToTop",
    Move::ToTop,
  ));
  let actions = rows.actions.clone();
  let remove = IconButton::new(format!("{id}-remove"), Icon::X, ctx.t("delivery.queue.lane.remove"))
    .compact()
    .on_click(move || actions.remove(run))
    .render(ctx);
  vec![up, top, remove]
}

const fn state_name(state: &QueueState) -> &'static str {
  match state {
    QueueState::Queued => "queued",
    QueueState::Syncing => "syncing",
    QueueState::Rechecking { .. } => "rechecking",
    QueueState::Integrating { .. } => "integrating",
    QueueState::WaitingForApproval => "waiting_for_approval",
    QueueState::Blocked(_) => "blocked",
    QueueState::Left(_) => "left",
    QueueState::Merged(_) => "merged",
    QueueState::Removed => "removed",
  }
}
