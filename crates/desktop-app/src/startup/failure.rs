//! The failure screens (`kMM56` and the failures of `ut8NI`): what failed, the file or folder with Copy,
//! what to do, *Open log* and *Show data folder*, and the log's path.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{app::ctx::Ctx, node::Element};

use super::{Failure, parts, screen::screen_column};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  code_box::CodeBox,
  icon::Icon,
};

/// The screen of `failure`; `log` is the app's log file, when it has one.
pub fn screen(ctx: &mut Ctx, failure: &Failure, log: Option<&Path>) -> Element {
  let texts = Texts::of(ctx, failure);
  render(ctx, &texts, log)
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
fn render(ctx: &mut Ctx, failure: &Texts, log: Option<&Path>) -> Element {
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
