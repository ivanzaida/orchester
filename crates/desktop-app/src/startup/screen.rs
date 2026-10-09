//! The startup screens in the window: the content panel with the screen in its middle, between the
//! title bar (the app menu and the window controls, no tabs) and a status bar with only the version.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::Column,
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_store::location::LockHolder;
use orchester_workspace::display;

use super::{
  Failure, Problem, StartupProblem,
  front::{self, Front},
  parts,
};
use crate::{
  chrome,
  shell::status_bar,
  theme::{ExtraColor, StartupSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::CodeBox,
    detail_row::{DetailValue, detail_row},
    icon::Icon,
  },
};

/// The width of a startup screen (`Startup · Orchester is already open` `g0Kj1`).
const SCREEN_WIDTH: f32 = 600.0;

/// The window of a session that did not start: `problem` in the content panel.
pub fn window(ctx: &mut Ctx, problem: Problem) -> Element {
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .child(ctx.mount_keyed::<StartupScreen>("startup", problem));
  let window = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceBase)
    .child(body)
    .child(status_bar::version_only(ctx));
  chrome::bare(ctx, window.into())
}

/// One startup screen in the content panel (`Content · Startup` `RqhE7`).
pub struct StartupScreen {
  /// *Switch to that window* got no answer: the screen says so until it is pressed again.
  no_answer: Signal<bool>,
}

impl Component for StartupScreen {
  type Props = Problem;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      no_answer: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Problem { problem, log } = ctx.props::<Problem>().clone();
    let screen = match &problem {
      StartupProblem::AlreadyOpen { folder, holder } => self.already_open(ctx, folder, *holder),
      StartupProblem::Failed(failure) => {
        let texts = Texts::of(ctx, failure);
        failure_screen(ctx, &texts, log.as_deref())
      }
    };
    Column::new()
      .id("screen-startup")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(StartupSpacing::Panel.role())
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .background(PaletteColor::SurfacePanel)
      .rounded(RadiusSize::Lg)
      .border_inside(BorderSize::Sm, ExtraColor::PanelBorder.role())
      .clip()
      .child(screen)
  }
}

impl StartupScreen {
  /// Another app has the data folder open (`N0Lpc`), or it did not come to the front (`uiSod`).
  fn already_open(&self, ctx: &mut Ctx, folder: &Path, holder: Option<LockHolder>) -> Element {
    let no_answer = self.no_answer.get();
    let other = holder.map_or_else(
      || ctx.t("startup.alreadyOpen.unknownWindow"),
      |holder| {
        ctx.t_args(
          "startup.alreadyOpen.since",
          [
            ("time", display::clock(holder.started_at)),
            ("pid", holder.pid.to_string()),
          ],
        )
      },
    );
    let pid = holder.map(|holder| holder.pid.to_string()).unwrap_or_default();
    let heading = if no_answer {
      parts::heading(
        &ctx.t("startup.noAnswer.title"),
        Some(&ctx.t_args("startup.noAnswer.reason", [("pid", pid.as_str())])),
        &ctx.t("startup.noAnswer.description"),
      )
    } else {
      parts::heading(
        &ctx.t("startup.alreadyOpen.title"),
        None,
        &ctx.t("startup.alreadyOpen.description"),
      )
    };
    let facts = Column::new()
      .width(Dimension::Pct(100.0))
      .child(detail_row(
        "startup-data-folder",
        &ctx.t("startup.dataFolder"),
        &folder.display().to_string(),
        DetailValue::Mono,
      ))
      .child(detail_row(
        "startup-other-window",
        &ctx.t("startup.alreadyOpen.otherWindow"),
        &other,
        DetailValue::Text,
      ));
    let step = no_answer.then(|| {
      parts::step(
        &ctx.t("startup.noAnswer.step"),
        &ctx.t_args("startup.noAnswer.stepBody", [("pid", pid.as_str())]),
      )
    });
    let (switch, footnote) = self.switch(ctx, holder);
    screen_column(if no_answer {
      "startup-no-answer"
    } else {
      "startup-already-open"
    })
    .child(parts::tile(Icon::AppWindow, false))
    .child(heading)
    .child(facts)
    .with_children(step)
    .child(parts::actions(switch.into_iter().chain([close_button(ctx)]).collect()))
    .with_children((!no_answer).then(|| parts::footnote(&footnote, false)))
    .into()
  }

  /// *Switch to that window*, when it can be offered, and the line under the actions that says what it
  /// does or why it is not offered.
  fn switch(&self, ctx: &mut Ctx, holder: Option<LockHolder>) -> (Option<Element>, Arc<str>) {
    let Some(holder) = holder else {
      return (None, ctx.t("startup.alreadyOpen.unknownProcess"));
    };
    if !front::SUPPORTED {
      return (None, ctx.t("startup.alreadyOpen.unsupported"));
    }
    let window = ctx.window();
    let no_answer = self.no_answer.clone();
    let button = ActionButton::new(
      "startup-switch",
      ButtonKind::Primary,
      ctx.t("startup.alreadyOpen.switch"),
    )
    .icon(Icon::AppWindow)
    .on_click(move || match front::bring_to_front(holder.pid) {
      Front::Brought => window.request_close(),
      Front::NoAnswer => no_answer.set(true),
    });
    (Some(button.into()), ctx.t("startup.alreadyOpen.footnote"))
  }
}

fn close_button(ctx: &mut Ctx) -> Element {
  let window = ctx.window();
  ActionButton::new(
    "startup-close",
    ButtonKind::Secondary,
    ctx.t("startup.alreadyOpen.close"),
  )
  .on_click(move || window.request_close())
  .into()
}

/// What a failure screen says, and the file and folder it shows.
struct Texts {
  words: Words,
  /// The path's label and the path, when one is known.
  path: Option<(Arc<str>, String)>,
  /// The label of the button that shows the folder, and the folder.
  folder: Option<(Arc<str>, PathBuf)>,
}

/// The words of a failure screen, and its element id.
struct Words {
  id: &'static str,
  title: Arc<str>,
  reason: Arc<str>,
  description: Arc<str>,
  step: Arc<str>,
}

impl Texts {
  fn of(ctx: &Ctx, failure: &Failure) -> Self {
    match failure {
      Failure::NewerData {
        database,
        found,
        supported,
      } => Self::about_database(
        ctx,
        database,
        Words {
          id: "startup-newer-data",
          title: ctx.t("startup.newerData.title"),
          reason: ctx.t("startup.newerData.reason"),
          description: versions(ctx, "startup.newerData.description", (*found, *supported)),
          step: ctx.t("startup.newerData.step"),
        },
      ),
      Failure::UpdateFailed {
        database,
        references,
        found,
        supported,
      } => Self::about_database(
        ctx,
        database,
        Words {
          id: "startup-update-failed",
          title: ctx.t("startup.updateFailed.title"),
          reason: ctx.t_args("startup.updateFailed.reason", [("count", references.to_string())]),
          description: versions(ctx, "startup.updateFailed.description", (*found, *supported)),
          step: ctx.t("startup.updateFailed.step"),
        },
      ),
      Failure::Unreadable { database, reason } => Self::about_database(
        ctx,
        database,
        Words {
          id: "startup-unreadable",
          title: ctx.t("startup.unreadable.title"),
          reason: ctx.t_args("startup.unreadable.reason", [("reason", reason.as_str())]),
          description: ctx.t("startup.unreadable.description"),
          step: ctx.t("startup.unreadable.step"),
        },
      ),
      Failure::NotWritable {
        folder,
        reason,
        beside_program,
      } => Self::not_writable(ctx, folder, reason, *beside_program),
      Failure::ProgramFolderUnknown => Self {
        words: Words {
          id: "startup-program-folder",
          title: ctx.t("startup.programFolder.title"),
          reason: ctx.t("startup.programFolder.reason"),
          description: ctx.t("startup.programFolder.description"),
          step: ctx.t("startup.programFolder.step"),
        },
        path: None,
        folder: None,
      },
    }
  }

  /// A failure about the database file: `words`, the database's path and its folder.
  fn about_database(ctx: &Ctx, database: &Path, words: Words) -> Self {
    Self {
      words,
      path: Some((ctx.t("startup.database"), database.display().to_string())),
      folder: database
        .parent()
        .map(|folder| (ctx.t("startup.showDataFolder"), folder.to_path_buf())),
    }
  }

  fn not_writable(ctx: &Ctx, folder: &Path, reason: &str, beside_program: bool) -> Self {
    let (description, step) = if beside_program {
      (
        "startup.notWritable.besideProgram",
        "startup.notWritable.stepBesideProgram",
      )
    } else {
      ("startup.notWritable.elsewhere", "startup.notWritable.stepElsewhere")
    };
    Self {
      words: Words {
        id: "startup-not-writable",
        title: ctx.t("startup.notWritable.title"),
        reason: Arc::from(reason),
        description: ctx.t(description),
        step: ctx.t(step),
      },
      path: Some((ctx.t("startup.dataFolder"), folder.display().to_string())),
      folder: Some((ctx.t("startup.showFolder"), folder.to_path_buf())),
    }
  }
}

/// `key` with the data version the database has and the one this build reads.
fn versions(ctx: &Ctx, key: &str, (found, supported): (u32, u32)) -> Arc<str> {
  ctx.t_args(
    key,
    [("found", found.to_string()), ("supported", supported.to_string())],
  )
}

/// A failure screen (`kMM56` and the failures of `ut8NI`): what failed, the file or folder, what to do,
/// and the log.
fn failure_screen(ctx: &mut Ctx, failure: &Texts, log: Option<&Path>) -> Element {
  let open_log = log.map(|log| {
    let log = log.to_path_buf();
    ActionButton::new("startup-open-log", ButtonKind::Secondary, ctx.t("startup.openLog"))
      .icon(Icon::FileText)
      .on_click(move || parts::open(log.clone()))
      .into()
  });
  let show_folder = failure.folder.as_ref().map(|(label, folder)| {
    let folder = folder.clone();
    ActionButton::new("startup-show-folder", ButtonKind::Secondary, label.clone())
      .icon(Icon::FolderOpen)
      .on_click(move || parts::open(folder.clone()))
      .into()
  });
  let path = failure
    .path
    .as_ref()
    .map(|(label, path)| CodeBox::new("startup-path", label.clone(), path.as_str()).render(ctx));
  let footnote = log.map(|log| ctx.t_args("startup.log", [("path", log.display().to_string())]));
  let words = &failure.words;
  screen_column(words.id)
    .child(parts::tile(Icon::CircleAlert, true))
    .child(parts::heading(&words.title, Some(&words.reason), &words.description))
    .with_children(path)
    .child(parts::step(&ctx.t("startup.keepWorking"), &words.step))
    .child(parts::actions(open_log.into_iter().chain(show_folder).collect()))
    .with_children(footnote.map(|footnote| parts::footnote(&footnote, true)))
    .into()
}

/// The column of one startup screen, 600 wide, its parts 20 apart.
fn screen_column(id: &'static str) -> Column {
  Column::new()
    .id(id)
    .width(Dimension::Px(SCREEN_WIDTH))
    .spacing(StartupSpacing::Block.role())
}
