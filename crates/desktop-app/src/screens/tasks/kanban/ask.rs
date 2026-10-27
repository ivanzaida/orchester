//! What a drop or a command asks in place before anything changes, in a popover at the card or over the selection bar
//! (`12 Moves and confirmations` `aUNO8`, `13 Menus, bulk and header` `CdMab`, drawn by `question`): queuing a draft
//! that starts at once (*Queue KON-27? It starts now: 1 agent slot is free.* with Keep as draft and Queue), running a
//! task again, marking work that waits to be merged as done, and for several selected tasks the same questions,
//! closing those whose runs have not ended, and deleting them. Enter and Esc answer it too.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Column,
};
use orchester_workspace::tasks::close::Closing;

use super::{
  actions::{Ask, BoardState, Change, ChangeKind},
  bulk::Bulk,
  question::{Parts, question},
};
use crate::{
  screens::task_close::{Closer, Subject},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
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

/// Whether a change is under way: a question's answers wait for it, by button or by key.
pub fn pending(state: &BoardState, closer: &Closer) -> bool {
  state.busy() || state.bulk.state().get().is_pending() || closer.busy()
}

/// The question `asked` puts, with its answers.
pub fn ask_panel(ctx: &Ctx, (state, closer): (&BoardState, &Closer), asked: &Ask) -> Column {
  let words = words(ctx, asked);
  let pending = pending(state, closer);
  let keep = ActionButton::new("tasks-board-ask-keep", ButtonKind::Ghost, words.keep).in_form();
  let keep = if pending {
    keep.disabled()
  } else {
    let asked = state.asked.clone();
    keep.on_click(move || asked.set(None))
  };
  let confirm = ActionButton::new(
    "tasks-board-ask-confirm",
    if words.danger {
      ButtonKind::Danger
    } else {
      ButtonKind::Secondary
    },
    words.confirm,
  )
  .in_form();
  let confirm = if pending {
    confirm.loading()
  } else {
    let (state, closer, asked) = (state.clone(), closer.clone(), asked.clone());
    confirm.on_click(move || answer(&state, &closer, &asked))
  };
  question(
    ctx,
    Parts {
      id: "tasks-board-ask",
      glyph: words.glyph,
      tone: words.tone,
      title: words.title,
      detail: words.detail,
      keeps: ctx.t("tasks.board.ask.keeps"),
      more: Vec::new(),
      keep: keep.into(),
      confirm: confirm.into(),
    },
  )
}

/// The person confirmed `asked`.
pub fn answer(state: &BoardState, closer: &Closer, asked: &Ask) {
  state.asked.set(None);
  match asked.clone() {
    Ask::StartsNow { task, .. } => state.change(vec![change(&task, ChangeKind::Queue)]),
    Ask::RunAgain {
      task,
      after,
      revision,
      pipeline,
    } => state.change(vec![change(
      &task,
      ChangeKind::RunAgain {
        after,
        revision,
        pipeline,
      },
    )]),
    Ask::Unmerged(task) => closer.close(task, Closing::Done),
    Ask::BulkQueue { changes, .. } => state.change(changes),
    Ask::BulkUnmerged { tasks, .. } => state.bulk.run(Bulk::Prepare(Closing::Done, tasks)),
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
        if *free == 1 {
          "tasks.board.ask.startsNow.one"
        } else {
          "tasks.board.ask.startsNow.other"
        },
        [("task", task.key.to_string()), ("count", free.to_string())],
      ),
      ctx.t_args("tasks.board.ask.startsNow.detail", [("pipeline", &**pipeline)]),
      ("tasks.board.ask.startsNow.keep", "tasks.board.ask.startsNow.confirm"),
    ),
    Ask::RunAgain { task, pipeline, .. } => neutral(
      ctx.t_args("tasks.board.ask.runAgain.title", [("task", &*task.key)]),
      ctx.t_args(
        "tasks.board.ask.runAgain.detail",
        [("task", &*task.key), ("pipeline", &**pipeline)],
      ),
      ("tasks.board.ask.runAgain.keep", "tasks.board.ask.runAgain.confirm"),
    ),
    Ask::Unmerged(task) => unmerged(ctx, task),
    Ask::BulkQueue { queued, free, .. } => neutral(
      match queued.as_slice() {
        [task] => ctx.t_args(
          if *free == 1 {
            "tasks.board.ask.startsNow.one"
          } else {
            "tasks.board.ask.startsNow.other"
          },
          [("task", task.key.to_string()), ("count", free.to_string())],
        ),
        _ => ctx.t_args(
          if *free == 1 {
            "tasks.board.ask.bulkQueue.one"
          } else {
            "tasks.board.ask.bulkQueue.other"
          },
          [("tasks", listed(ctx, queued.iter())), ("count", free.to_string())],
        ),
      },
      ctx.t("tasks.board.ask.bulkQueue.detail"),
      ("tasks.board.ask.bulkQueue.keep", "tasks.board.ask.startsNow.confirm"),
    ),
    Ask::BulkUnmerged { tasks, unmerged: held } => match tasks.as_slice() {
      [task] => unmerged(ctx, task),
      _ => Words {
        title: ctx.t_args(
          "tasks.board.ask.bulkUnmerged.title",
          [("count", tasks.len().to_string())],
        ),
        detail: ctx.t_args(
          "tasks.board.ask.bulkUnmerged.detail",
          [("unmerged", listed(ctx, held.iter()))],
        ),
        keep: ctx.t("tasks.board.ask.bulkKeep"),
        ..unmerged(ctx, &tasks[0])
      },
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

/// *Mark KON-14 as done without merging?* (`12 Moves and confirmations` `aUNO8`).
fn unmerged(ctx: &Ctx, task: &Subject) -> Words {
  Words {
    glyph: Icon::GitMerge,
    tone: PaletteColor::Warning,
    title: ctx.t_args("tasks.board.ask.unmerged.title", [("task", &*task.key)]),
    detail: ctx.t_args("tasks.board.ask.unmerged.detail", [("task", &*task.key)]),
    keep: ctx.t("tasks.board.ask.unmerged.keep"),
    confirm: ctx.t("tasks.board.ask.unmerged.confirm"),
    danger: false,
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
        [
          ("first", first.join(&ctx.t("tasks.listSeparator"))),
          ("last", (*last).to_owned()),
        ],
      )
      .to_string(),
  }
}
