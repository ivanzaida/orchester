//! What *Create a git repository here* says (`OdlXb`): the head, the folder's properties, the note, the choices of
//! the first commit, the progress, the end and why it failed. A count changes the wording, so each count-bearing
//! message has a key for one and one for more.

use std::{path::Path, sync::Arc};

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_workspace::repository::init::{
  FirstCommit, InitError, InitStep, Initialized, LocalRepository, Survey, SurveyError,
};

use super::{
  super::{
    Head,
    words::{Property, Value, folder_property, git_value, property},
  },
  Shown,
};
use crate::ui::icon::Icon;

/// The head of a block in `shown`; `failed` when the last attempt in the folder failed.
pub(super) fn head(ctx: &Ctx, shown: &Shown, failed: bool) -> Head {
  let (status, title) = match shown {
    Shown::Missing => ("setup.git.status.notFound", "setup.git.heading.notFound"),
    Shown::NotRepository => (
      "setup.github.status.notRepository",
      "setup.github.heading.notRepository",
    ),
    Shown::NoCommits { .. } if failed => ("setup.git.status.noCommits", "setup.git.heading.commitFailed"),
    Shown::NoCommits { .. } => ("setup.git.status.noCommits", "setup.git.heading.noCommits"),
  };
  Head {
    glyph: Icon::CircleX,
    mark: PaletteColor::Danger,
    color: PaletteColor::Danger,
    status,
    title: ctx.t(title),
    // git is not optional.
    optional: false,
  }
}

/// The name a test or a tool reads the state by.
pub(super) const fn state_name(shown: &Shown) -> &'static str {
  match shown {
    Shown::Missing => "git_not_found",
    Shown::NotRepository => "not_repository",
    Shown::NoCommits { .. } => "no_commits",
  }
}

/// The folder, what git sees there, and what a first commit would hold (`vzK69`).
pub(super) fn properties(
  ctx: &Ctx,
  (shown, folder): (&Shown, &Path),
  surveyed: Option<&Result<Survey, SurveyError>>,
) -> Vec<Property> {
  let danger = Value::Text(PaletteColor::Danger);
  let git = match shown {
    Shown::Missing => ctx.t("setup.git.value.notFound"),
    Shown::NotRepository => ctx.t("setup.github.value.noGitRepository"),
    Shown::NoCommits { branch } => git_value(ctx, &LocalRepository::NoCommits { branch: branch.clone() })
      .unwrap_or_else(|| ctx.t("setup.git.value.noCommitsDetached")),
  };
  let mut rows = vec![folder_property(ctx, folder), property(ctx, "git", git, danger)];
  if !matches!(shown, Shown::Missing) {
    let (value, color) = files_value(ctx, surveyed);
    rows.push(Property {
      label: ctx.t("setup.git.property.files"),
      value,
      look: Value::Text(color),
    });
  }
  rows
}

/// What a first commit would add (`zWwl2`): counting, how many files and whether a `.gitignore` keeps some
/// out, or that they could not be counted.
fn files_value(ctx: &Ctx, surveyed: Option<&Result<Survey, SurveyError>>) -> (Arc<str>, PaletteColor) {
  let survey = match surveyed {
    None => return (ctx.t("setup.git.value.counting"), PaletteColor::TextMuted),
    Some(Err(_)) => return (ctx.t("setup.git.value.filesUnknown"), PaletteColor::TextMuted),
    Some(Ok(survey)) => survey,
  };
  let value = match (survey.files, survey.gitignore) {
    (0, _) => ctx.t("setup.git.value.noFiles"),
    (1, true) => ctx.t("setup.git.value.fileIgnored"),
    (1, false) => ctx.t("setup.git.value.file"),
    (count, true) => ctx.t_args("setup.git.value.filesIgnored", [("count", count.to_string())]),
    (count, false) => ctx.t_args("setup.git.value.files", [("count", count.to_string())]),
  };
  (value, PaletteColor::TextPrimary)
}

/// What the block says under its properties (`wzrTC`, `C4H7IK`, `b5Cd2c`).
pub(super) fn note(ctx: &Ctx, shown: &Shown, failed: bool) -> Arc<str> {
  ctx.t(match shown {
    Shown::Missing => "setup.git.note.notFound",
    Shown::NotRepository => "setup.git.note.notRepository",
    Shown::NoCommits { .. } if failed => "setup.git.note.commitFailed",
    Shown::NoCommits { .. } => "setup.git.note.noCommits",
  })
}

/// The branch `git init` will make (`Rpgmg`, `ffbMI`).
pub(super) fn branch_line(ctx: &Ctx, surveyed: Option<&Result<Survey, SurveyError>>) -> Arc<str> {
  match surveyed {
    Some(Ok(survey)) if survey.branch.is_configured() => {
      ctx.t_args("setup.git.create.branchConfigured", [("branch", survey.branch.name())])
    }
    Some(Ok(survey)) => ctx.t_args("setup.git.create.branchDefault", [("branch", survey.branch.name())]),
    _ => ctx.t("setup.git.create.branchChecking"),
  }
}

/// What committing the folder's files adds (`gsvsq` in `m98LAi`).
pub(super) fn files_detail(ctx: &Ctx, surveyed: Option<&Result<Survey, SurveyError>>) -> Arc<str> {
  match surveyed {
    Some(Ok(Survey {
      files: 1,
      gitignore: true,
      ..
    })) => ctx.t("setup.git.create.fileDetailIgnored"),
    Some(Ok(Survey {
      files: 1,
      gitignore: false,
      ..
    })) => ctx.t("setup.git.create.fileDetail"),
    Some(Ok(Survey {
      files, gitignore: true, ..
    }))
      if *files > 0 =>
    {
      ctx.t_args("setup.git.create.filesDetailIgnored", [("count", files.to_string())])
    }
    Some(Ok(Survey { files, .. })) if *files > 0 => {
      ctx.t_args("setup.git.create.filesDetail", [("count", files.to_string())])
    }
    _ => ctx.t("setup.git.create.filesDetailUnknown"),
  }
}

/// What an empty first commit means for the folder's files (`wOeJ8` in `pPDOB`, `gsvsq` in `K1IiVO`).
pub(super) fn empty_detail(ctx: &Ctx, surveyed: Option<&Result<Survey, SurveyError>>) -> Arc<str> {
  match surveyed
    .and_then(|surveyed| surveyed.as_ref().ok())
    .map(|survey| survey.files)
  {
    Some(0) => ctx.t("setup.git.create.emptyDetailEmptyFolder"),
    Some(1) => ctx.t("setup.git.create.emptyDetailOne"),
    Some(count) => ctx.t_args("setup.git.create.emptyDetail", [("count", count.to_string())]),
    None => ctx.t("setup.git.create.emptyDetailUnknown"),
  }
}

/// What git is doing now (`k3Ok3`, `OOs1a` in `AWogn`).
pub(super) fn committing(ctx: &Ctx, first: FirstCommit, surveyed: Option<&Result<Survey, SurveyError>>) -> Arc<str> {
  let files = surveyed
    .and_then(|surveyed| surveyed.as_ref().ok())
    .map(|survey| survey.files);
  match (first, files) {
    (FirstCommit::Empty, _) => ctx.t("setup.git.progress.committingEmpty"),
    (FirstCommit::Files, Some(1)) => ctx.t("setup.git.progress.committingFile"),
    (FirstCommit::Files, Some(count)) => {
      ctx.t_args("setup.git.progress.committingFiles", [("count", count.to_string())])
    }
    (FirstCommit::Files, None) => ctx.t("setup.git.progress.committingUnknown"),
  }
}

/// `git init` under way, on the branch the survey named.
pub(super) fn initializing(ctx: &Ctx, surveyed: Option<&Result<Survey, SurveyError>>) -> Arc<str> {
  match surveyed {
    Some(Ok(survey)) => ctx.t_args("setup.git.progress.initializing", [("branch", survey.branch.name())]),
    _ => ctx.t("setup.git.progress.initializingUnknown"),
  }
}

/// The repository made here, with its first commit (`pY9fQ` in `Tz5KP`).
pub(super) fn done(ctx: &Ctx, made: &Initialized) -> Arc<str> {
  match made.files {
    Some(0) => ctx.t("setup.git.done.empty"),
    Some(1) => ctx.t("setup.git.done.file"),
    Some(count) => ctx.t_args("setup.git.done.files", [("count", count.to_string())]),
    None => ctx.t("setup.git.done.unknown"),
  }
}

/// Why the repository or its first commit was not made (`nHh0j`, `mOlU7`, `B4oVhS`), and what git wrote, when
/// it wrote something.
pub(super) fn failure<'a>(ctx: &Ctx, error: &'a InitError, folder: &Path) -> (Arc<str>, Option<&'a str>) {
  let said = |key: &str, text: &'a String| (ctx.t(key), Some(text.as_str()).filter(|text| !text.is_empty()));
  match error {
    InitError::GitMissing => (ctx.t("setup.git.error.gitMissing"), None),
    InitError::NotWritable => (
      ctx.t_args(
        "setup.git.error.notWritable",
        [("folder", folder.display().to_string())],
      ),
      None,
    ),
    InitError::AlreadyRepository => (ctx.t("setup.git.error.alreadyRepository"), None),
    InitError::NotRepository => (ctx.t("setup.git.error.notRepository"), None),
    InitError::HasCommits => (ctx.t("setup.git.error.hasCommits"), None),
    InitError::Staged => (ctx.t("setup.git.error.staged"), None),
    InitError::InitFailed(text) => said("setup.git.error.initFailed", text),
    InitError::AddFailed(text) => said("setup.git.error.addFailed", text),
    InitError::CommitFailed(text) | InitError::NoIdentity(text) => said("setup.git.error.commitFailed", text),
    InitError::Stopped(step) => (
      ctx.t(match step {
        InitStep::Init => "setup.git.error.initStopped",
        InitStep::Add => "setup.git.error.addStopped",
        InitStep::Commit => "setup.git.error.commitStopped",
      }),
      None,
    ),
  }
}
