//! What a drop or a command asks in place, over the columns, before anything changes (`12 Moves and confirmations`
//! `aUNO8`, `13 Menus, bulk and header` `CdMab`): queuing a draft that starts at once (*Queue KON-27? It starts now: 1
//! agent slot is free.* with Keep as draft and Queue), running a task again, marking work that waits to be merged as
//! done, closing several tasks whose runs have not ended, and deleting several. A card `surface_raised` in a stroke of
//! its weight, radius 6, 12 over and under its parts, 14 at its sides: a 15 mark, the question in 13/600 and what it
//! does in 12.5 `text_secondary`; then *Enter confirms · Esc keeps it* and its two buttons at the end. Enter and Esc
//! answer it too.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::close::Closing;

use super::{
  actions::{Ask, BoardState, Change, ChangeKind},
  bulk::Bulk,
};
use crate::{
  screens::task_close::{Closer, Subject},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    kbd::key_cap,
  },
};

/// The words of a question: its mark and tone, title, what it does, and its two buttons.
struct Words {
  glyph: Icon,
  tone: PaletteColor,
  title: Arc<str>,
  detail: Arc<str>,
  keep: Arc<str>,
  confirm: Arc<str>,
  danger: bool,
}

/// The question `asked` puts, with its answers.
pub fn ask_card(ctx: &Ctx, (state, closer): (&BoardState, &Closer), asked: &Ask) -> Element {
  let words = words(ctx, asked);
  let pending = state.busy() || state.bulk.state().get().is_pending() || closer.busy();
  let keys = Row::new()
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(key_cap("tasks-board-ask-enter", &ctx.t("tasks.board.keys.enter")))
    .child(note(&ctx.t("tasks.board.ask.confirms")))
    .child(key_cap("tasks-board-ask-esc", &ctx.t("tasks.board.keys.esc")))
    .child(note(&ctx.t("tasks.board.ask.keeps")));
  let keep = ActionButton::new("tasks-board-ask-keep", ButtonKind::Ghost, words.keep).in_form();
  let keep = if pending {
    keep.disabled()
  } else {
    let asked = state.asked.clone();
    keep.on_click(move || asked.set(None))
  };
  let confirm = ActionButton::new(
    "tasks-board-ask-confirm",
    if words.danger { ButtonKind::Danger } else { ButtonKind::Secondary },
    words.confirm,
  )
  .in_form();
  let confirm = if pending {
    confirm.loading()
  } else {
    let (state, closer, asked) = (state.clone(), closer.clone(), asked.clone());
    confirm.on_click(move || answer(&state, &closer, &asked))
  };
  Column::new()
    .id("tasks-board-ask")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, words.tone.clone()))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Start)
        .child(icon(words.glyph, IconSize::Nav, words.tone))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .spacing(SpacingSize::Xs)
            .child(
              Text::new(&words.title)
                .id("tasks-board-ask-title")
                .variant(ExtraTypography::FormSection.role()),
            )
            .child(
              Text::new(&words.detail)
                .id("tasks-board-ask-text")
                .variant(ExtraTypography::Lead.role())
                .width(Dimension::Pct(100.0)),
            ),
        ),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(keys)
        .child(Spacer::new().flex(1.0))
        .child(keep)
        .child(confirm),
    )
    .into()
}

/// The person confirmed `asked`.
pub fn answer(state: &BoardState, closer: &Closer, asked: &Ask) {
  state.asked.set(None);
  match asked.clone() {
    Ask::StartsNow { task, .. } => state.change(vec![change(&task, ChangeKind::Queue)]),
    Ask::RunAgain { task, after, pipeline } => state.change(vec![change(&task, ChangeKind::RunAgain { after, pipeline })]),
    Ask::Unmerged(task) => closer.close(task, Closing::Done),
    Ask::BulkClose { closing, tasks, runs } => state.bulk.run(Bulk::Close(closing, tasks, runs)),
    Ask::BulkDelete(tasks) => state.bulk.run(Bulk::Delete(tasks)),
  }
}

fn change(task: &Subject, kind: ChangeKind) -> Change {
  Change {
    task: task.id,
    key: task.key.clone(),
    kind,
  }
}

fn words(ctx: &Ctx, asked: &Ask) -> Words {
  let neutral = |title, detail, (keep, confirm): (&str, &str)| Words {
    glyph: Icon::CircleHelp,
    tone: PaletteColor::BorderStrong,
    title,
    detail,
    keep: ctx.t(keep),
    confirm: ctx.t(confirm),
    danger: false,
  };
  match asked {
    Ask::StartsNow { task, free, pipeline } => neutral(
      ctx.t_args(
        if *free == 1 { "tasks.board.ask.startsNow.one" } else { "tasks.board.ask.startsNow.other" },
        [("task", task.key.to_string()), ("count", free.to_string())],
      ),
      ctx.t_args("tasks.board.ask.startsNow.detail", [("pipeline", &**pipeline)]),
      ("tasks.board.ask.startsNow.keep", "tasks.board.ask.startsNow.confirm"),
    ),
    Ask::RunAgain { task, pipeline, .. } => neutral(
      ctx.t_args("tasks.board.ask.runAgain.title", [("task", &*task.key)]),
      ctx.t_args("tasks.board.ask.runAgain.detail", [("task", &*task.key), ("pipeline", &**pipeline)]),
      ("tasks.board.ask.runAgain.keep", "tasks.board.ask.runAgain.confirm"),
    ),
    Ask::Unmerged(task) => Words {
      glyph: Icon::GitMerge,
      tone: PaletteColor::Warning,
      title: ctx.t_args("tasks.board.ask.unmerged.title", [("task", &*task.key)]),
      detail: ctx.t_args("tasks.board.ask.unmerged.detail", [("task", &*task.key)]),
      keep: ctx.t("tasks.board.ask.unmerged.keep"),
      confirm: ctx.t("tasks.board.ask.unmerged.confirm"),
      danger: false,
    },
    Ask::BulkClose { closing, tasks, runs } => bulk_close(ctx, *closing, tasks, runs),
    Ask::BulkDelete(tasks) => Words {
      glyph: Icon::Trash2,
      tone: PaletteColor::Danger,
      title: ctx.t_args("tasks.board.ask.delete.title", [("count", tasks.len().to_string())]),
      detail: ctx.t_args("tasks.board.ask.delete.detail", [("tasks", listed(ctx, tasks.iter()))]),
      keep: ctx.t("tasks.board.ask.delete.keep"),
      confirm: ctx.t_args("tasks.board.ask.delete.confirm", [("count", tasks.len().to_string())]),
      danger: true,
    },
  }
}

/// *Stop 2 runs and mark 3 tasks as done?* (`uQ875`).
fn bulk_close(ctx: &Ctx, closing: Closing, tasks: &[Subject], runs: &[(Subject, orchester_core::id::RunId)]) -> Words {
  let (title, detail, confirm) = match closing {
    Closing::Done => (
      "tasks.board.ask.bulkDone.title",
      "tasks.board.ask.bulkDone.detail",
      "tasks.board.ask.bulkDone.confirm",
    ),
    Closing::Canceled => (
      "tasks.board.ask.bulkCanceled.title",
      "tasks.board.ask.bulkCanceled.detail",
      "tasks.board.ask.bulkCanceled.confirm",
    ),
  };
  Words {
    glyph: Icon::CircleStop,
    tone: PaletteColor::Warning,
    title: ctx.t_args(
      title,
      [("runs", runs.len().to_string()), ("count", tasks.len().to_string())],
    ),
    detail: ctx.t_args(
      detail,
      [
        ("running", listed(ctx, runs.iter().map(|(task, _)| task))),
        ("tasks", listed(ctx, tasks.iter())),
      ],
    ),
    keep: ctx.t("tasks.board.ask.bulkKeep"),
    confirm: ctx.t(confirm),
    danger: false,
  }
}

/// `tasks` by their keys: *KON-9, KON-16 and KON-21*.
pub fn listed<'a>(ctx: &Ctx, tasks: impl Iterator<Item = &'a Subject>) -> String {
  let keys: Vec<&str> = tasks.map(|task| &*task.key).collect();
  match keys.split_last() {
    None => String::new(),
    Some((last, [])) => (*last).to_owned(),
    Some((last, first)) => ctx
      .t_args(
        "tasks.board.ask.keysAnd",
        [("first", first.join(&ctx.t("tasks.listSeparator"))), ("last", (*last).to_owned())],
      )
      .to_string(),
  }
}

fn note(words: &str) -> Text {
  Text::new(words).variant(ExtraTypography::Note.role()).nowrap()
}
