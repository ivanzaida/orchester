//! The largest-first retained-folder table (GXXao), including measuring, empty and failed-removal rows.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text, VirtualizedList},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::RunId,
  run::{PauseReason, RunStatus},
  time::Timestamp,
};
use orchester_workspace::{
  background, display,
  messages::Message,
  storage::{KeptKind, KeptRow},
};

use super::{StoragePage, TabProps, bytes, error, note, read::Reading, retry, total};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    skeleton,
    state::EmptyState,
    status::{Status, StatusChip},
    table::{ColumnWidth, TableRow, table_header},
    tooltip::{Tooltip, Truncated},
  },
};
/// Size columns and row height in GXXao (Jsbd1); the action column takes room for two compact actions.
const SIZE: ColumnWidth = ColumnWidth::Fixed(70.0);
const BUILD: ColumnWidth = ColumnWidth::Fixed(82.0);
const LAST: ColumnWidth = ColumnWidth::Fixed(82.0);
const ACTIONS: ColumnWidth = ColumnWidth::Fixed(240.0);
const ROW_HEIGHT: f32 = 56.0;
const MAX_HEIGHT: f32 = 560.0;

pub fn list(ctx: &mut Ctx, page: &StoragePage, props: &TabProps, reading: Option<&Reading>) -> Element {
  let title = ctx.t("settings.project.runs.storage.kept");
  let Some(reading) = reading else {
    let bones = skeleton::list(
      ctx,
      "storage-rows-reading",
      &[(200.0, 280.0), (160.0, 240.0), (180.0, 260.0)],
    );
    return super::Group::new("storage-kept", title)
      .end(note(ctx, "settings.project.runs.storage.reading"))
      .row(skeleton::Skeleton::new("storage-loading", ctx.t("settings.project.runs.storage.reading")).show(ctx, bones))
      .into();
  };
  let inventory = match &reading.0 {
    Ok(read) => read,
    Err(e) => {
      return super::Group::new("storage-kept", title)
        .row(error(ctx, "storage-inventory-error", e))
        .end(retry(ctx, "storage-inventory-retry", &page.refresh))
        .into();
    }
  };
  let group = group(ctx, inventory);
  if inventory.rows.is_empty() {
    return group.row(empty(ctx, props)).into();
  }
  group.row(table(ctx, page, inventory)).into()
}

fn group(ctx: &Ctx, inventory: &super::read::Inventory) -> super::Group {
  let title = ctx.t("settings.project.runs.storage.kept");
  let count = inventory.rows.len();
  let count_words = if inventory.finished {
    ctx.t_args(
      "settings.project.runs.storage.count",
      [
        ("count", count.to_string()),
        ("size", bytes(ctx, total(&inventory.rows)).to_string()),
      ],
    )
  } else {
    ctx.t_args(
      "settings.project.runs.storage.measuring",
      [("count", inventory.measured.to_string()), ("total", count.to_string())],
    )
  };
  let headings = [
    "settings.project.runs.storage.columns.item",
    "settings.project.runs.storage.columns.size",
    "settings.project.runs.storage.columns.build",
    "settings.project.runs.storage.columns.last",
  ]
  .map(|k| ctx.t(k));
  let header = table_header(
    "storage-table-header",
    &[
      (ColumnWidth::Fill, Some(&headings[0])),
      (SIZE, Some(&headings[1])),
      (BUILD, Some(&headings[2])),
      (LAST, Some(&headings[3])),
      (ACTIONS, None),
    ],
    false,
  );
  super::Group::new("storage-kept", title)
    .end(Text::new(&count_words).variant(ExtraTypography::Note.role()))
    .row(header)
    .after(crate::ui::unavailable::reason_line(
      "storage-build-unavailable-reason",
      &ctx.t("settings.project.runs.storage.buildUnavailable"),
    ))
    .after(crate::ui::unavailable::reason_line(
      "storage-workspace-unavailable-reason",
      &ctx.t("settings.project.runs.storage.workspaceRemoveUnavailable"),
    ))
}

fn empty(ctx: &mut Ctx, props: &TabProps) -> Element {
  let new_task = ActionButton::new(
    "storage-new-task",
    ButtonKind::Secondary,
    ctx.t("settings.project.runs.storage.newTask"),
  )
  .opens(crate::navigation::Navigation::of(ctx).to(crate::routes::Route::Project(
    props.project.open.project.id(),
    crate::routes::ProjectPage::NewTask,
  )));
  EmptyState::new(
    "storage-empty",
    Icon::HardDrive,
    ctx.t("settings.project.runs.storage.empty.title"),
    ctx.t("settings.project.runs.storage.empty.hint"),
  )
  .unframed()
  .action(new_task)
  .into()
}

fn table(ctx: &mut Ctx, page: &StoragePage, inventory: &super::read::Inventory) -> Element {
  let count = inventory.rows.len();
  let prepare = page.prepare.clone();
  let busy = page.busy() || !inventory.finished;
  let measuring = !inventory.finished;
  let failures = page.failures.get().0;
  VirtualizedList::new_shared(ctx, Arc::new(inventory.rows.clone()))
    .list_key("storage-folders")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px((ROW_HEIGHT * count as f32).min(MAX_HEIGHT)))
    .mount_keyed::<Folder, _, _, _>(
      |row| row.path.display().to_string(),
      move |row| Props {
        row: row.clone(),
        busy,
        measuring,
        failed: row
          .run
          .and_then(|id| failures.iter().find(|(run, _)| *run == id).map(|(_, e)| e.clone()))
          .or_else(|| {
            row.removal.as_ref().map(|left| {
              orchester_workspace::tasks::integration::IntegrationProblem::NotRemoved {
                path: left.path.clone(),
                detail: left.reason.clone().unwrap_or_default(),
              }
              .message(", ")
            })
          }),
        ask: {
          let prepare = prepare.clone();
          Arc::new(move |run| prepare.run(vec![run]))
        },
      },
    )
}

#[derive(Clone)]
struct Props {
  row: KeptRow,
  busy: bool,
  measuring: bool,
  failed: Option<Message>,
  ask: Arc<dyn Fn(RunId) + Send + Sync>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.row == other.row && self.busy == other.busy && self.measuring == other.measuring && self.failed == other.failed
  }
}
impl DevtoolsInspectable for Props {}
struct Folder {
  open: FutureAction<std::path::PathBuf, (), ()>,
}
impl Component for Folder {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.future_action(|path: std::path::PathBuf| async move {
        background::run(move || {
          orchester_exec::external::open_path(&path).map_err(|e| {
            tracing::warn!(%e,"cannot open storage folder");
          })
        })
        .await
        .map_err(|_| ())?
      }),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let row = &props.row;
    let id = row.path.display().to_string();
    let identity = identity(ctx, row, &id);
    let age = row.last_used.map_or_else(
      || ctx.t("settings.project.runs.storage.unknown"),
      |at| message_text(ctx, &display::ago(Timestamp::now(), at)),
    );
    let actions = self.actions(ctx, &props);
    let table = TableRow::new(format!("storage-row-{id}"))
      .height(ROW_HEIGHT)
      .cell(ColumnWidth::Fill, identity)
      .cell(
        SIZE,
        Text::new(&if row.bytes.is_none() && props.measuring {
          ctx.t("settings.project.runs.storage.measuringValue")
        } else {
          bytes(ctx, row.bytes)
        })
        .variant(ExtraTypography::MonoNote.role()),
      )
      .cell(
        BUILD,
        Tooltip::new(
          format!("storage-build-{id}"),
          ctx.t("settings.project.runs.storage.buildUnavailable"),
        )
        .wrap(ctx, note(ctx, "settings.project.runs.storage.unknown")),
      )
      .cell(LAST, Text::new(&age).variant(ExtraTypography::Note.role()))
      .cell(ACTIONS, actions);
    let mut content = Column::new().width(Dimension::Pct(100.0)).child(table);
    if let Some(failed) = &props.failed {
      content = content
        .background(PaletteColor::DangerMuted)
        .child(error(ctx, &format!("storage-remove-error-{id}"), failed))
        .child(
          Text::new(&id)
            .variant(ExtraTypography::MonoSmall.role())
            .width(Dimension::Pct(100.0)),
        )
        .child(note(ctx, "settings.project.runs.storage.processUnavailable"));
    }
    if self.open.state().get().error.is_some() {
      content = content.child(note(ctx, "settings.project.runs.storage.folderOpenFailed"));
    }
    content
  }
}
impl Folder {
  fn actions(&self, ctx: &Ctx, props: &Props) -> Element {
    let row = &props.row;
    let id = row.path.display().to_string();
    if row.in_use {
      return note(ctx, "settings.project.runs.storage.inUse").into();
    }
    let mut actions = Row::new().spacing(ExtraSpacing::Snug.role());
    if props.failed.is_some() {
      let (open, path) = (self.open.clone(), row.path.clone());
      actions = actions.child(
        ActionButton::new(
          format!("storage-open-{id}"),
          ButtonKind::Ghost,
          ctx.t("settings.project.runs.storage.openFolder"),
        )
        .compact()
        .on_click(move || open.run(path.clone())),
      );
    } else {
      actions = actions.child(
        ActionButton::new(
          format!("storage-clear-{id}"),
          ButtonKind::Ghost,
          ctx.t("settings.project.runs.storage.clearBuild"),
        )
        .compact()
        .unavailable(ctx.t("settings.project.runs.storage.buildUnavailable")),
      );
    }
    let key = if props.failed.is_some() {
      "common.tryAgain"
    } else {
      "settings.project.runs.storage.remove"
    };
    let mut remove = ActionButton::new(format!("storage-remove-{id}"), ButtonKind::DangerGhost, ctx.t(key)).compact();
    if let Some(run) = row.removable_run() {
      if props.busy {
        remove = remove.disabled();
      } else {
        let ask = props.ask.clone();
        remove = remove.on_click(move || ask(run));
      }
    } else {
      remove = remove.unavailable(ctx.t(if matches!(row.kind, KeptKind::Workspace(_)) {
        "settings.project.runs.storage.workspaceRemoveUnavailable"
      } else {
        "settings.project.runs.storage.runRemoveUnavailable"
      }));
    }
    actions.child(remove).into()
  }
}

fn identity(ctx: &mut Ctx, row: &KeptRow, id: &str) -> Element {
  let title = match row.kind {
    KeptKind::Workspace(number) => ctx.t_args(
      "settings.project.runs.storage.workspace",
      [("number", number.get().to_string())],
    ),
    KeptKind::Worktree(_) => row.task.as_ref().map_or_else(
      || ctx.t("settings.project.runs.storage.worktree"),
      |(key, title, _)| {
        ctx.t_args(
          "settings.project.runs.storage.task",
          [("key", key.as_str()), ("title", title.as_str())],
        )
      },
    ),
  };
  let top = Row::new()
    .spacing(SpacingSize::Sm)
    .child(Truncated::new(format!("storage-title-{id}"), title, ExtraTypography::NoteStrong.role()).render(ctx))
    .with_children(row.status.as_ref().map(|s| {
      StatusChip::new(format!("storage-state-{id}"), status(s))
        .compact()
        .render(ctx)
    }));
  let path = row.path.display().to_string();
  let hint = row.task.as_ref().map_or_else(
    || Arc::from(path.as_str()),
    |(_, _, number)| {
      ctx.t_args(
        "settings.project.runs.storage.path",
        [("number", number.to_string()), ("path", path.clone())],
      )
    },
  );
  Row::new()
    .spacing(SpacingSize::Sm)
    .child(icon(
      if matches!(row.kind, KeptKind::Workspace(_)) {
        Icon::Layers2
      } else {
        Icon::GitBranch
      },
      IconSize::Md,
      PaletteColor::TextMuted,
    ))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(top)
        .child(Truncated::new(format!("storage-path-{id}"), hint, ExtraTypography::MonoSmall.role()).render(ctx)),
    )
    .into()
}
fn status(status: &RunStatus) -> Status {
  match status {
    RunStatus::Pending => Status::Queued,
    RunStatus::Running => Status::Running,
    RunStatus::Paused(PauseReason::LowDisk | PauseReason::Requested) => Status::Paused,
    RunStatus::Paused(_) => Status::StoppedAtLimit,
    RunStatus::WaitingForPerson(_) => Status::NeedsInput,
    RunStatus::Interrupted(_) => Status::Interrupted,
    RunStatus::Succeeded => Status::Completed,
    RunStatus::Failed(_) => Status::Failed,
    RunStatus::Cancelled => Status::Canceled,
  }
}
