//! What a drop or a command asks in place before anything changes, in a popover at the card or over the selection bar
//! (`12 Moves and confirmations` `aUNO8`, `13 Menus, bulk and header` `CdMab`, drawn by `question`): queuing a draft
//! that starts at once (*Queue KON-27? It starts now: 1 agent slot is free.* with Keep as draft and Queue), what a task
//! whose run ended does on Todo (ORC-144: *Don't move*, *Move to Todo*, which only sets its status, and *Run again*),
//! marking work that waits to be merged as done, and for several selected tasks the same questions, closing those whose
//! runs have not ended, and deleting them. Enter and Esc answer it too: Enter as the last button, Esc as the first.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Column,
};
use orchester_core::task::organization::WorkflowStatus;
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

/// The words of a question: its mark and tone, title, what it does, its buttons, and what Enter and Esc do.
struct Words {
  glyph: Icon,
  tone: PaletteColor,
  title: Arc<str>,
  detail: Arc<str>,
  keep: Arc<str>,
  /// *Move to Todo*, between keep and confirm, for tasks whose runs ended; `None` for a question of two answers.
  move_only: Option<Arc<str>>,
  confirm: Arc<str>,
  danger: bool,
  /// What Enter does and what Esc keeps, in the hint under the words.
  keys: (Arc<str>, Arc<str>),
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
  let three = words.move_only.is_some();
  let between = words.move_only.map(|label| {
    let button = ActionButton::new("tasks-board-ask-move", ButtonKind::Secondary, label).in_form();
    let button = if pending {
      button.disabled()
    } else {
      let (state, asked) = (state.clone(), asked.clone());
      button.on_click(move || move_only(&state, &asked))
    };
    button.into()
  });
  let confirm = ActionButton::new(
    "tasks-board-ask-confirm",
    match (words.danger, three) {
      (true, _) => ButtonKind::Danger,
      (false, true) => ButtonKind::Primary,
      (false, false) => ButtonKind::Secondary,
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
  let (confirms, keeps) = words.keys;
  question(
    ctx,
    Parts {
      id: "tasks-board-ask",
      glyph: words.glyph,
      tone: words.tone,
      title: words.title,
      detail: words.detail,
      confirms,
      keeps,
      more: Vec::new(),
      keep: keep.into(),
      between,
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
    Ask::BulkRunAgain { again, changes, .. } => state.change(changes.into_iter().chain(again).collect()),
    Ask::BulkUnmerged { tasks, .. } => state.bulk.run(Bulk::Prepare(Closing::Done, tasks)),
    Ask::BulkClose { closing, tasks, runs } => state.bulk.run(Bulk::Close(closing, tasks, runs)),
    Ask::BulkDelete(tasks) => state.bulk.run(Bulk::Delete(tasks)),
  }
}

/// The person chose *Move to Todo* for the tasks of `asked` whose runs ended: only their status changes
/// ([`orchester_workspace::tasks::moves::MOVE_ONLY`]), and nothing starts.
fn move_only(state: &BoardState, asked: &Ask) {
  state.asked.set(None);
  let to_todo = |task: &Subject| change(task, ChangeKind::Status(WorkflowStatus::Todo));
  match asked {
    Ask::RunAgain { task, .. } => state.change(vec![to_todo(task)]),
    Ask::BulkRunAgain { ended, changes, .. } => {
      state.change(changes.iter().cloned().chain(ended.iter().map(to_todo)).collect());
    }
    _ => {}
  }
}

fn change(task: &Subject, kind: ChangeKind) -> Change {
  Change {
    task: task.id,
    key: task.key.clone(),
    kind,
  }
}

/// A question with the neutral mark and two answers, `keep` and `confirm` its buttons' words.
fn neutral(ctx: &Ctx, (title, detail): (Arc<str>, Arc<str>), (keep, confirm): (&str, &str)) -> Words {
  Words {
    glyph: Icon::CircleHelp,
    tone: PaletteColor::BorderStrong,
    title,
    detail,
    keep: ctx.t(keep),
    move_only: None,
    confirm: ctx.t(confirm),
    danger: false,
    keys: plain_keys(ctx),
  }
}

/// *Run KON-1 again?* for the tasks `ended` whose runs ended, moved to Todo: *Don't move*, *Move to Todo* and *Run
/// again*, with *Enter runs again · Esc doesn't move it*. A card in Todo already (Run again from its menu) has nowhere
/// to move: *Keep in Todo* and *Run again*.
fn run_again(ctx: &Ctx, ended: &[Subject]) -> Words {
  if let [task] = ended
    && task.status == WorkflowStatus::Todo
  {
    return Words {
      keys: (
        ctx.t("tasks.board.ask.runAgain.confirms"),
        ctx.t("tasks.board.ask.keeps"),
      ),
      ..neutral(
        ctx,
        (
          ctx.t_args("tasks.board.ask.runAgain.title", [("task", &*task.key)]),
          ctx.t("tasks.board.ask.runAgain.inTodo"),
        ),
        (
          "tasks.board.ask.runAgain.keepInTodo",
          "tasks.board.ask.runAgain.confirm",
        ),
      )
    };
  }
  let (title, detail, keeps) = match ended {
    [task] => (
      ctx.t_args("tasks.board.ask.runAgain.title", [("task", &*task.key)]),
      ctx.t("tasks.board.ask.runAgain.detail"),
      "tasks.board.ask.runAgain.keeps",
    ),
    _ => (
      ctx.t_args(
        "tasks.board.ask.bulkRunAgain.title",
        [("tasks", listed(ctx, ended.iter()))],
      ),
      ctx.t("tasks.board.ask.bulkRunAgain.detail"),
      "tasks.board.ask.bulkRunAgain.keeps",
    ),
  };
  Words {
    move_only: Some(ctx.t("tasks.board.ask.runAgain.move")),
    keys: (ctx.t("tasks.board.ask.runAgain.confirms"), ctx.t(keeps)),
    ..neutral(
      ctx,
      (title, detail),
      ("tasks.board.ask.runAgain.keep", "tasks.board.ask.runAgain.confirm"),
    )
  }
}

fn words(ctx: &Ctx, asked: &Ask) -> Words {
  let neutral = |title, detail, buttons: (&str, &str)| neutral(ctx, (title, detail), buttons);
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
    Ask::RunAgain { task, .. } => run_again(ctx, std::slice::from_ref(task)),
    Ask::BulkRunAgain { ended, .. } => run_again(ctx, ended),
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
      move_only: None,
      confirm: ctx.t_args("tasks.board.ask.delete.confirm", [("count", tasks.len().to_string())]),
      danger: true,
      keys: plain_keys(ctx),
    },
  }
}

/// *Enter confirms · Esc keeps it*, under a question of two answers.
fn plain_keys(ctx: &Ctx) -> (Arc<str>, Arc<str>) {
  (ctx.t("tasks.board.ask.confirms"), ctx.t("tasks.board.ask.keeps"))
}

/// *Mark KON-14 as done without merging?* (`12 Moves and confirmations` `aUNO8`).
fn unmerged(ctx: &Ctx, task: &Subject) -> Words {
  Words {
    glyph: Icon::GitMerge,
    tone: PaletteColor::Warning,
    title: ctx.t_args("tasks.board.ask.unmerged.title", [("task", &*task.key)]),
    detail: ctx.t_args("tasks.board.ask.unmerged.detail", [("task", &*task.key)]),
    keep: ctx.t("tasks.board.ask.unmerged.keep"),
    move_only: None,
    confirm: ctx.t("tasks.board.ask.unmerged.confirm"),
    danger: false,
    keys: plain_keys(ctx),
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
    move_only: None,
    confirm: ctx.t(confirm),
    danger: false,
    keys: plain_keys(ctx),
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
