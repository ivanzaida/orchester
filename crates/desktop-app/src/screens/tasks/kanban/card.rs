//! A card of the board (`KON-12` `l5gcS` in `01 Board` `ntozw`): `surface_panel` in a `border` stroke, radius 6, 8
//! over and 9 under its parts, 10 at its sides, 5 between them. Its key and its age (mono 11 `text_muted`), its title
//! in 13/500, its run chip (the run's status in the list's words with the step it is at, 12/500 in its tone, on
//! `warning_muted` while it needs input, and the mark of who runs its agent), its readiness phrase (11.5, in its tone),
//! and its chips: priority, points, module and tags, each 20 high in a `border` stroke, radius 4. Display options
//! turn each of them but the title off. A selected card is `accent_muted` in an `accent` stroke with its check; the
//! card a drag lifted leaves its place as an outline.

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::AgentProvider,
  id::{ProjectId, RunId, TaskId},
  run::TaskPhase,
  sharing::TaskRevision,
  task::Task,
  time::Timestamp,
};
use orchester_workspace::tasks::{
  attention::NeedItem,
  data::ProjectTasks,
  list::{PipelineLabel, ReadinessKind, TaskRow},
};

use super::{
  super::{list::age, phrase::Phrase, row::points_chip},
  display::{Display, Property},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    agent_mark::agent_mark,
    icon::{Icon, icon},
    message::message_text,
    status::{Status, priority_look},
    tooltip::Truncated,
  },
};

/// A chip's height (`Priority` `jeI96`) and a run chip's (`Run` `jfcqV`).
const CHIP_HEIGHT: f32 = 20.0;
/// The line of the key and the age (`Top` `V6LRJ`).
const TOP_HEIGHT: f32 = 16.0;
/// A selected card's check (`Check` `wAri6`).
const CHECK: f32 = 14.0;

/// What a card's run chip says.
#[derive(Clone, Debug, PartialEq)]
pub struct RunLine {
  pub glyph: Icon,
  pub words: Arc<str>,
  pub color: PaletteColor,
  /// The run needs input: the chip is on `warning_muted`.
  pub filled: bool,
  pub agent: Option<AgentProvider>,
}

/// What a card shows of its task.
#[derive(Clone, Debug, PartialEq)]
pub struct CardModel {
  pub project: ProjectId,
  pub row: TaskRow,
  pub run: Option<RunLine>,
  pub readiness: Arc<str>,
  pub readiness_color: PaletteColor,
  pub age: Arc<str>,
  pub module: Option<Arc<str>>,
  pub points: Option<Arc<str>>,
  /// Something of the task waits for the person.
  pub waits: bool,
  /// The task a Create just added.
  pub highlighted: bool,
  /// Its latest run, when it can be read: Run again runs after it.
  pub latest_run: Option<RunId>,
  /// The task as read, for a task that has run: Run again runs it only as it is now.
  pub revision: Option<TaskRevision>,
  /// Its pipeline's name, as a refusal of Run again names it.
  pub pipeline: Arc<str>,
}

/// What the cards are built from.
pub struct CardSource<'a> {
  pub project: ProjectId,
  pub tasks: &'a ProjectTasks,
  pub needs: &'a HashMap<TaskId, NeedItem>,
  pub highlight: Option<&'a str>,
  pub now: Timestamp,
}

impl CardModel {
  pub fn of(ctx: &Ctx, source: &CardSource<'_>, row: &TaskRow) -> Self {
    let need = source.needs.get(&row.id);
    let phrase = Phrase::of(ctx, row, need, source.tasks);
    Self {
      project: source.project,
      run: run_line(ctx, row),
      readiness: message_text(ctx, &row.detail),
      readiness_color: phrase.color,
      age: age(ctx, source.now, row.created_at),
      module: row.module.and_then(|id| {
        source
          .tasks
          .modules
          .iter()
          .find(|module| module.id() == id)
          .map(|module| Arc::from(module.name().as_str()))
      }),
      points: row.estimate.as_ref().map(|estimate| points_chip(ctx, estimate)),
      waits: need.is_some(),
      highlighted: source.highlight == Some(&*row.key),
      latest_run: source
        .tasks
        .phases
        .get(&row.id)
        .and_then(|latest| latest.run())
        .map(|run| run.id),
      revision: row
        .has_run
        .then(|| source.tasks.tasks.iter().find(|task| task.id() == row.id))
        .flatten()
        .map(Task::revision),
      pipeline: match &row.pipeline {
        PipelineLabel::Published { name, .. } => name.clone(),
        other => message_text(ctx, &other.message()),
      },
      row: row.clone(),
    }
  }
}

/// The run chip of `row`: none for a draft; its run's status, and the step it is at while it works, waits or stopped,
/// with who runs its agent.
fn run_line(ctx: &Ctx, row: &TaskRow) -> Option<RunLine> {
  let phase = match (&row.readiness, row.phase) {
    (ReadinessKind::Unreadable, _) | (_, None) => {
      return Some(RunLine {
        glyph: Status::Unreadable.icon(),
        words: ctx.t(Status::Unreadable.key()),
        color: Status::Unreadable.color(),
        filled: false,
        agent: None,
      });
    }
    (_, Some(TaskPhase::Draft)) => return None,
    (_, Some(phase)) => phase,
  };
  let status = match phase {
    TaskPhase::Succeeded if !row.nothing_to_merge => Status::ReadyToMerge,
    TaskPhase::Succeeded => Status::Succeeded,
    phase => Status::of_phase(phase),
  };
  // Work that waits to be merged passed every step: it reads as the step's success, as the list's phrase does.
  let color = match status {
    Status::ReadyToMerge => PaletteColor::Success,
    status => status.color(),
  };
  let stands = matches!(
    phase,
    TaskPhase::Running
      | TaskPhase::Waiting
      | TaskPhase::Stuck
      | TaskPhase::Paused
      | TaskPhase::StoppedAtLimit
      | TaskPhase::Interrupted
      | TaskPhase::Failed
  );
  let word = ctx.t(status.key());
  let words = match row.step.as_ref().filter(|_| stands) {
    Some(step) => {
      let step = ctx.t_args(
        "tasks.runStep",
        [
          ("number", format!("{:02}", step.number)),
          ("step", step.name.to_string()),
        ],
      );
      ctx.t_args("ui.status.withDetail", [("status", word), ("detail", step)])
    }
    None => word,
  };
  Some(RunLine {
    glyph: status.icon(),
    words,
    color,
    filled: status == Status::NeedsInput,
    agent: (phase != TaskPhase::Queued).then_some(row.agent).flatten(),
  })
}

/// How a card is drawn besides what it shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CardLook {
  pub selected: bool,
  /// Some card is selected: every card shows its check.
  pub selecting: bool,
  /// A drag lifted it away, or its move waits for the person's answer in another column: its place is an outline.
  pub lifted: bool,
  /// A question waits for the person's answer at it: faded in a `border_focus` stroke, where its move would put it
  /// or where it stays (`KON-16` `y7FQj9`).
  pub waiting: bool,
  pub focused: bool,
  /// The pointer is on it, or its menu is open: its age gives way to *More actions*.
  pub acting: bool,
}

/// The card's parts in a column: what it shows, with `more` (its *More actions*) at the top's end while acting.
pub fn content(
  ctx: &mut Ctx,
  card: &CardModel,
  (display, look): (&Display, CardLook),
  more: Option<Element>,
) -> Column {
  let key = &card.row.key;
  let blocked =
    (display.shows(Property::Blocked) && card.row.readiness == ReadinessKind::Blocked).then(|| blocked_flag(ctx, key));
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOP_HEIGHT))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(look.selecting.then(|| check(key, look.selected)))
    .with_children(display.shows(Property::Key).then(|| {
      Text::new(key)
        .id(format!("task-key-{key}"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
    }))
    .with_children(blocked)
    .child(Spacer::new().flex(1.0))
    .with_children(match more {
      Some(more) => Some(more),
      None => display.shows(Property::Age).then(|| {
        Text::new(&card.age)
          .id(format!("task-age-{key}"))
          .variant(ExtraTypography::MonoNote.role())
          .nowrap()
          .into()
      }),
    });
  let title = Row::new().width(Dimension::Pct(100.0)).child(
    Truncated::new(
      format!("task-title-text-{key}"),
      card.row.title.clone(),
      ExtraTypography::Strong.role(),
    )
    .render(ctx),
  );
  let run = card
    .run
    .as_ref()
    .filter(|_| display.shows(Property::Run))
    .map(|run| run_chip(ctx, key, run, display.shows(Property::Agent)));
  let readiness = display.shows(Property::Readiness).then(|| {
    Text::new(&card.readiness)
      .id(format!("task-state-{key}"))
      .variant(ExtraTypography::Note.role())
      .color(card.readiness_color.clone())
      .width(Dimension::Pct(100.0))
  });
  let chips = chips(ctx, card, display);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(ExtraSpacing::Inline.role())
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .spacing(ExtraSpacing::Pair.role())
    .child(top)
    .child(title)
    .with_children(run)
    .with_children(readiness)
    .with_children(chips)
}

/// The run's status in its tone, on `warning_muted` while it needs input, and who runs its agent.
fn run_chip(ctx: &Ctx, key: &str, run: &RunLine, agent: bool) -> Row {
  let chip = Row::new()
    .id(format!("task-run-{key}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(CHIP_HEIGHT))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(run.glyph, IconSize::Sm, run.color.clone()))
    .child(
      Text::new(&run.words)
        .id(format!("task-run-{key}-label"))
        .variant(TypographyStyle::Label)
        .color(run.color.clone())
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(
      run
        .agent
        .filter(|_| agent)
        .map(|provider| agent_mark(ctx, provider, IconSize::Md)),
    );
  if run.filled {
    chip
      .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
      .background(PaletteColor::WarningMuted)
      .rounded(RadiusSize::Sm)
  } else {
    chip
  }
}

/// The priority, points, module and tags the display shows; `None` when it shows none of them.
fn chips(ctx: &Ctx, card: &CardModel, display: &Display) -> Option<Row> {
  let key = &card.row.key;
  let mut chips: Vec<Element> = Vec::new();
  if display.shows(Property::Priority) {
    let (glyph, word, color) = priority_look(card.row.organization.priority());
    chips.push(
      chip(
        format!("task-priority-{key}"),
        Padding::horizontal(ExtraSpacing::Fine.role()),
      )
      .describe("a11y_name", ctx.t(word))
      .child(icon(glyph, IconSize::Sm, color))
      .into(),
    );
  }
  if let Some(points) = card.points.as_ref().filter(|_| display.shows(Property::Points)) {
    chips.push(word_chip(format!("task-points-{key}"), (points, None), true).into());
  }
  if let Some(module) = card.module.as_ref().filter(|_| display.shows(Property::Module)) {
    chips.push(word_chip(format!("task-module-{key}"), (module, Some(Icon::Target)), false).into());
  }
  if display.shows(Property::Tags) {
    chips.extend(
      card
        .row
        .tags
        .iter()
        .enumerate()
        .map(|(index, tag)| word_chip(format!("task-tag-{key}-{index}"), (tag, None), false).into()),
    );
  }
  (!chips.is_empty()).then(|| {
    Row::new()
      .id(format!("task-chips-{key}"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .clip()
      .with_children(chips)
  })
}

/// *Blocked* after the key: an 11 ban and the word in 11/500 `text_secondary`, 3 apart (`Blocked` `a0u3wX`).
fn blocked_flag(ctx: &Ctx, key: &str) -> Row {
  Row::new()
    .id(format!("task-blocked-{key}"))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Fine.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Ban, IconSize::Xs, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t(Status::Blocked.key()))
        .id(format!("task-blocked-{key}-label"))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
}
fn chip(id: String, padding: Padding) -> Row {
  Row::new()
    .id(id)
    .height(Dimension::Px(CHIP_HEIGHT))
    .flex_shrink(0.0)
    .padding(padding)
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
}

/// A chip of words, after a `text_muted` glyph if it has one: points in Geist Mono, a module or a tag in 11.5,
/// `text_secondary`.
fn word_chip(id: String, (words, glyph): (&str, Option<Icon>), mono: bool) -> Row {
  let style = if mono {
    ExtraTypography::MonoNote.role()
  } else {
    ExtraTypography::Note.role()
  };
  let label = format!("{id}-label");
  chip(id, Padding::horizontal(ExtraSpacing::Snug.role()))
    .with_children(glyph.map(|glyph| icon(glyph, IconSize::Xs, PaletteColor::TextMuted)))
    .child(
      Text::new(words)
        .id(label)
        .variant(style)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
}

/// A selected card's check, or an empty box while others are selected (`Check` `wAri6`, `yTHLE`).
fn check(key: &str, selected: bool) -> Row {
  let side = Row::new()
    .id(format!("task-check-{key}"))
    .width(Dimension::Px(CHECK))
    .height(Dimension::Px(CHECK))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .describe("state", if selected { "checked" } else { "unchecked" });
  if selected {
    side
      .background(PaletteColor::Accent)
      .child(icon(Icon::Check, IconSize::Xs, PaletteColor::TextInverse))
  } else {
    side
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
  }
}
